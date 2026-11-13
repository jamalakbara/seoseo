use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AuditSchedule {
    pub project_id: Uuid,
    pub domain: String,
    pub start_url: String,
    pub config: serde_json::Value,
    pub baseline_audit_id: Option<Uuid>,
    pub schedule: String,
    pub hour_utc: i16,
    pub weekday: i16,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_audit_id: Option<Uuid>,
    pub last_error: Option<String>,
    pub revision: i64,
    pub created_by: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

pub async fn get(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Option<AuditSchedule>> {
    sqlx::query_as("SELECT s.* FROM audit_schedules s JOIN projects p ON p.id=s.project_id WHERE p.organization_id=$1 AND p.id=$2")
        .bind(org).bind(pid).fetch_optional(pool).await.map_err(internal)
}

pub struct SaveAuditSchedule {
    pub org: Uuid,
    pub pid: Uuid,
    pub user: Uuid,
    pub expected_revision: i64,
    pub value: AuditSchedule,
}

pub async fn save(pool: &PgPool, input: SaveAuditSchedule) -> AppResult<AuditSchedule> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p: (Option<String>,bool) = sqlx::query_as("SELECT domain::text,archived_at IS NOT NULL FROM projects WHERE id=$1 AND organization_id=$2 FOR UPDATE")
        .bind(input.pid).bind(input.org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("project"))?;
    let s = input.value;
    if s.schedule != "manual" && (p.1 || p.0.as_deref() != Some(&s.domain)) {
        return Err(AppError::Conflict(
            "project archived or website changed; reload before scheduling".into(),
        ));
    }
    let revision: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM audit_schedules WHERE project_id=$1")
            .bind(input.pid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
    if revision.unwrap_or(0) != input.expected_revision {
        return Err(AppError::Conflict(
            "audit schedule changed; reload before saving".into(),
        ));
    }
    if let Some(id) = s.baseline_audit_id {
        sqlx::query_scalar::<_,Uuid>("SELECT id FROM audits WHERE id=$1 AND project_id=$2 AND status='completed' FOR KEY SHARE")
            .bind(id).bind(input.pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("completed baseline audit"))?;
    }
    let value = sqlx::query_as("INSERT INTO audit_schedules(project_id,domain,start_url,config,baseline_audit_id,schedule,hour_utc,weekday,next_run_at,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        ON CONFLICT(project_id) DO UPDATE SET domain=excluded.domain,start_url=excluded.start_url,config=excluded.config,baseline_audit_id=excluded.baseline_audit_id,schedule=excluded.schedule,hour_utc=excluded.hour_utc,weekday=excluded.weekday,next_run_at=excluded.next_run_at,created_by=excluded.created_by,last_error=NULL,revision=audit_schedules.revision+1,updated_at=clock_timestamp() RETURNING *")
        .bind(input.pid).bind(&s.domain).bind(&s.start_url).bind(&s.config).bind(s.baseline_audit_id).bind(&s.schedule).bind(s.hour_utc).bind(s.weekday).bind(s.next_run_at).bind(input.user)
        .fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(value)
}

pub async fn due(pool: &PgPool) -> AppResult<Vec<AuditSchedule>> {
    sqlx::query_as("SELECT * FROM audit_schedules WHERE next_run_at<=now() AND schedule<>'manual' ORDER BY next_run_at,project_id LIMIT 20")
        .fetch_all(pool).await.map_err(internal)
}

/// Serialize with manual audit starts and project edits. Claim, next time, audit and
/// job commit together; other workers cannot enqueue the same scheduled slot.
pub async fn enqueue_due(
    pool: &PgPool,
    previous: &AuditSchedule,
    next: DateTime<Utc>,
    operator_error: Option<String>,
) -> AppResult<Option<Uuid>> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p: Option<(Uuid,Option<String>,bool)> = sqlx::query_as("SELECT organization_id,domain::text,archived_at IS NOT NULL FROM projects WHERE id=$1 FOR UPDATE SKIP LOCKED")
        .bind(previous.project_id).fetch_optional(&mut *tx).await.map_err(internal)?;
    let Some((org, domain, archived)) = p else {
        return Ok(None);
    };
    let current: Option<AuditSchedule> = sqlx::query_as("SELECT * FROM audit_schedules WHERE project_id=$1 AND revision=$2 AND next_run_at=$3 AND next_run_at<=now() AND schedule<>'manual' FOR UPDATE")
        .bind(previous.project_id).bind(previous.revision).bind(previous.next_run_at).fetch_optional(&mut *tx).await.map_err(internal)?;
    let Some(s) = current else {
        return Ok(None);
    };
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audits WHERE project_id=$1")
        .bind(s.project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    let error = if archived {
        Some("Project archived; re-enable the schedule explicitly after restoring".into())
    } else if domain.as_deref() != Some(&s.domain) {
        Some("Website changed; review and save a new baseline schedule".into())
    } else if count >= 100 {
        Some("100 stored audits reached; delete unneeded audits and re-enable the schedule".into())
    } else {
        operator_error
    };
    if let Some(error) = error {
        sqlx::query("UPDATE audit_schedules SET schedule='manual',next_run_at=NULL,last_error=$2,revision=revision+1,updated_at=clock_timestamp() WHERE project_id=$1")
            .bind(s.project_id).bind(error).execute(&mut *tx).await.map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        return Ok(None);
    }
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM audits WHERE project_id=$1 AND status IN ('queued','running'))")
        .bind(s.project_id).fetch_one(&mut *tx).await.map_err(internal)?;
    if active {
        sqlx::query("UPDATE audit_schedules SET next_run_at=$2,last_error='Skipped this occurrence: an audit is already active',revision=revision+1,updated_at=clock_timestamp() WHERE project_id=$1")
            .bind(s.project_id).bind(next).execute(&mut *tx).await.map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        return Ok(None);
    }
    let aid = common::new_id();
    let jid = common::new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'site_audit',$2,$3,$4,3)")
        .bind(jid).bind(org).bind(s.project_id).bind(serde_json::json!({"audit_id":aid,"scheduled":true,"expected_domain":s.domain})).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO audits(id,project_id,started_by,start_url,config,job_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(aid).bind(s.project_id).bind(s.created_by).bind(s.start_url).bind(s.config).bind(jid).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("UPDATE audit_schedules SET next_run_at=$2,last_run_at=clock_timestamp(),last_audit_id=$3,last_error=NULL,revision=revision+1,updated_at=clock_timestamp() WHERE project_id=$1")
        .bind(s.project_id).bind(next).bind(aid).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(Some(aid))
}
