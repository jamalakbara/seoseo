use crate::{Actor, Role};
use common::{AppError, AppResult};
use db::audit_schedule::{AuditSchedule, SaveAuditSchedule};
use jobs::site_audit::AuditConfig;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetAuditScheduleInput {
    /// 0 for first save; otherwise the revision returned by GET.
    pub expected_revision: i64,
    pub expected_domain: String,
    /// Required when enabling a schedule; frozen configuration is copied from this completed audit.
    pub baseline_audit_id: Option<Uuid>,
    pub schedule: String,
    pub hour_utc: i16,
    pub weekday: i16,
}

pub async fn get(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<Option<AuditSchedule>> {
    db::projects::get(pool, actor.org_id, pid).await?;
    db::audit_schedule::get(pool, actor.org_id, pid).await
}

pub async fn set(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    policy: crawler::UrlPolicy,
    renderer_available: bool,
    input: SetAuditScheduleInput,
) -> AppResult<AuditSchedule> {
    actor.require(Role::Admin)?;
    let project = db::projects::get(pool, actor.org_id, pid).await?;
    if !["manual", "daily", "weekly", "monthly"].contains(&input.schedule.as_str())
        || !(0..=23).contains(&input.hour_utc)
        || !(1..=7).contains(&input.weekday)
        || input.expected_revision < 0
    {
        return Err(AppError::Validation(
            "choose manual/daily/weekly/monthly, hour UTC 0-23, weekday 1-7 and a valid revision"
                .into(),
        ));
    }
    let existing = db::audit_schedule::get(pool, actor.org_id, pid).await?;
    let mut value = if input.schedule == "manual" {
        existing
            .ok_or_else(|| AppError::Validation("no audit schedule has been configured".into()))?
    } else {
        let domain = project
            .domain
            .as_deref()
            .ok_or_else(|| AppError::Validation("set the project website first".into()))?;
        if project.archived_at.is_some() || input.expected_domain != domain {
            return Err(AppError::Conflict(
                "project archived or website changed; reload before scheduling".into(),
            ));
        }
        let id = input
            .baseline_audit_id
            .ok_or_else(|| AppError::Validation("select a completed baseline audit".into()))?;
        let audit = crate::audits::get(pool, actor, pid, id).await?;
        if audit.status != "completed" {
            return Err(AppError::Conflict(
                "baseline audit must be completed".into(),
            ));
        }
        let url = url::Url::parse(&audit.start_url)
            .map_err(|_| AppError::Validation("invalid baseline start URL".into()))?;
        policy
            .check_url(&url)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let root = url::Url::parse(&format!("https://{domain}/")).map_err(AppError::internal)?;
        if !crawler::urls::Scope::new(&root).contains(&url) {
            return Err(AppError::Validation(
                "baseline must audit the current project website".into(),
            ));
        }
        let config: AuditConfig = if audit.config == serde_json::json!({}) {
            AuditConfig::default()
        } else {
            serde_json::from_value(audit.config)
                .map_err(|_| AppError::Validation("invalid baseline crawl configuration".into()))?
        };
        if !(1..=10000).contains(&config.max_pages)
            || !(1..=8).contains(&config.concurrency)
            || config.delay_ms > 10000
            || config.exclude_paths.len() > 20
            || config.exclude_paths.iter().any(|s| s.len() > 1000)
            || audit.start_url.len() > 4096
        {
            return Err(AppError::Validation(
                "baseline crawl configuration exceeds supported limits".into(),
            ));
        }
        if (config.render_js || config.lighthouse) && !renderer_available {
            return Err(AppError::Conflict(
                "configure the renderer before scheduling this audit".into(),
            ));
        }
        AuditSchedule {
            project_id: pid,
            domain: domain.into(),
            start_url: audit.start_url,
            config: serde_json::to_value(config).map_err(AppError::internal)?,
            baseline_audit_id: Some(id),
            schedule: input.schedule.clone(),
            hour_utc: input.hour_utc,
            weekday: input.weekday,
            next_run_at: None,
            last_run_at: None,
            last_audit_id: None,
            last_error: None,
            revision: 0,
            created_by: Some(actor.user_id),
            updated_at: chrono::Utc::now(),
        }
    };
    value.schedule = input.schedule;
    value.hour_utc = input.hour_utc;
    value.weekday = input.weekday;
    value.next_run_at = jobs::rank_check::next_run(
        &value.schedule,
        value.hour_utc as u32,
        value.weekday as u32,
        chrono::Utc::now(),
    );
    db::audit_schedule::save(
        pool,
        SaveAuditSchedule {
            org: actor.org_id,
            pid,
            user: actor.user_id,
            expected_revision: input.expected_revision,
            value,
        },
    )
    .await
}
