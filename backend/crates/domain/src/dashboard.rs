//! Dashboard reads existing project evidence; provider refreshes use durable, deduplicated jobs.
use crate::{Actor, ActorVia, Role, gsc, projects};
use chrono::{Duration, Utc};
use common::{AppError, AppResult, Config};
use db::audits::Audit;
use db::dashboard::{self as repo, Counts, RankMover};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct ChecklistStep {
    pub id: String,
    pub label: String,
    pub href: String,
    pub complete: bool,
    pub skipped: bool,
}
#[derive(Serialize, ToSchema)]
pub struct DashboardOverview {
    pub checklist: Vec<ChecklistStep>,
    pub link_activity: Vec<db::integrations::LinkObservation>,
    pub latest_audit: Option<Audit>,
    pub completed_audit: Option<Audit>,
    pub counts: Counts,
    pub rank_movers: Vec<RankMover>,
    pub gsc_connected: bool,
    pub keywords: KeywordDashboard,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct KeywordSnapshot {
    pub site_url: String,
    pub start_date: String,
    pub end_date: String,
    pub recent_start_date: String,
    pub baseline_start_date: String,
    pub baseline_end_date: String,
    pub top: Vec<gsc::PerformanceRow>,
    pub new_queries: Vec<gsc::PerformanceRow>,
    pub striking_distance: Vec<gsc::PerformanceRow>,
    pub truncated: bool,
    pub note: String,
}

#[derive(Serialize, ToSchema)]
pub struct KeywordDashboard {
    pub site_url: Option<String>,
    pub snapshot: Option<KeywordSnapshot>,
    pub fetched_at: Option<chrono::DateTime<Utc>>,
    pub next_attempt_at: Option<chrono::DateTime<Utc>>,
    pub job_id: Option<Uuid>,
    pub refreshing: bool,
    pub error: Option<String>,
}
pub async fn keywords(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<KeywordDashboard> {
    projects::get(pool, actor, pid).await?;
    let source = repo::source(pool, actor.org_id, pid).await?;
    let row = repo::snapshot(pool, actor.org_id, pid).await?;
    match row {
        Some(r) => Ok(KeywordDashboard {
            site_url: source.site_url,
            snapshot: r
                .data
                .map(serde_json::from_value)
                .transpose()
                .map_err(AppError::internal)?,
            fetched_at: r.fetched_at,
            next_attempt_at: Some(r.next_attempt_at),
            job_id: r.job_id,
            refreshing: r.refreshing,
            error: r.error,
        }),
        None => Ok(KeywordDashboard {
            site_url: source.site_url,
            snapshot: None,
            fetched_at: None,
            next_attempt_at: None,
            job_id: None,
            refreshing: false,
            error: None,
        }),
    }
}
pub async fn refresh(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<KeywordDashboard> {
    projects::get(pool, actor, pid).await?;
    repo::enqueue_refresh(pool, actor.org_id, pid, actor.user_id).await?;
    keywords(pool, actor, pid).await
}
pub async fn overview(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<DashboardOverview> {
    let p = projects::get(pool, actor, pid).await?;
    let domain = p.domain.as_deref().unwrap_or("");
    let latest_audit =
        db::audits::latest_for_domain(pool, actor.org_id, pid, domain, false).await?;
    let completed_audit =
        db::audits::latest_for_domain(pool, actor.org_id, pid, domain, true).await?;
    let counts = repo::counts(pool, actor.org_id, pid).await?;
    let keywords = keywords(pool, actor, pid).await?;
    let gsc_connected = keywords.site_url.is_some();
    let skipped = repo::skipped(pool, actor.org_id, pid, actor.user_id).await?;
    let checklist = [
        (
            "audit",
            "Audit your website",
            "audit",
            completed_audit.is_some(),
        ),
        (
            "context",
            "Describe your business and goal",
            "onboarding",
            counts.memory_ready,
        ),
        ("gsc", "Connect Search Console", "gsc", gsc_connected),
        (
            "keywords",
            "Save target keywords",
            "keywords",
            counts.saved_keywords > 0,
        ),
        ("rank", "Set up rank tracking", "rank", counts.trackers > 0),
        (
            "reports",
            "Save your first report",
            "reports",
            counts.reports > 0,
        ),
    ]
    .into_iter()
    .map(|(id, label, path, complete)| ChecklistStep {
        id: id.into(),
        label: label.into(),
        href: format!("/p/{pid}/{path}"),
        complete,
        skipped: skipped.iter().any(|s| s == id),
    })
    .collect();
    let rank_movers = repo::movers(pool, actor.org_id, pid).await?;
    Ok(DashboardOverview {
        checklist,
        link_activity: db::integrations::links(pool, actor.org_id, pid).await?,
        latest_audit,
        completed_audit,
        counts,
        rank_movers,
        gsc_connected,
        keywords,
    })
}
#[derive(Deserialize, ToSchema)]
pub struct ChecklistInput {
    pub step: String,
    pub skipped: bool,
}
pub async fn checklist(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    input: ChecklistInput,
) -> AppResult<()> {
    projects::get(pool, actor, pid).await?;
    if !["audit", "context", "gsc", "keywords", "rank", "reports"].contains(&input.step.as_str()) {
        return Err(AppError::Validation("unknown checklist step".into()));
    }
    repo::skip(
        pool,
        actor.org_id,
        pid,
        actor.user_id,
        &input.step,
        input.skipped,
    )
    .await
}

fn query(
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
    dimensions: &[&str],
) -> gsc::PerformanceInput {
    gsc::PerformanceInput {
        start_date: start.to_string(),
        end_date: end.to_string(),
        dimensions: dimensions.iter().map(|s| s.to_string()).collect(),
        filters: vec![],
        search_type: Some("web".into()),
        row_limit: Some(1000),
        start_row: Some(0),
    }
}
fn is_striking(row: &gsc::PerformanceRow) -> bool {
    (5.0..=20.0).contains(&row.position)
}

async fn collect(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    site: &str,
) -> AppResult<KeywordSnapshot> {
    // Exclude an extra UTC day so the end date is safely complete in Google's Pacific timezone.
    let end = Utc::now().date_naive() - Duration::days(4);
    let start = end - Duration::days(27);
    let recent = end - Duration::days(6);
    let baseline_end = recent - Duration::days(1);
    let baseline_start = baseline_end - Duration::days(27);
    let mut top = gsc::performance(pool, actor, pid, cfg, query(start, end, &["query"])).await?;
    let mut pairs =
        gsc::performance(pool, actor, pid, cfg, query(start, end, &["query", "page"])).await?;
    let mut candidates =
        gsc::performance(pool, actor, pid, cfg, query(recent, end, &["query"])).await?;
    let baseline = gsc::performance(
        pool,
        actor,
        pid,
        cfg,
        query(baseline_start, baseline_end, &["query"]),
    )
    .await?;
    if [
        &top.site_url,
        &pairs.site_url,
        &candidates.site_url,
        &baseline.site_url,
    ]
    .iter()
    .any(|s| s.as_str() != site)
    {
        return Err(AppError::Conflict(
            "Search Console property changed during refresh".into(),
        ));
    }
    let truncated = [&top, &pairs, &candidates, &baseline]
        .iter()
        .any(|v| v.next_start_row.is_some());
    let old: std::collections::HashSet<&str> = baseline
        .rows
        .iter()
        .filter_map(|r| r.keys.first().map(String::as_str))
        .collect();
    candidates
        .rows
        .sort_by(|a, b| b.impressions.total_cmp(&a.impressions));
    let mut new_queries = vec![];
    for row in candidates
        .rows
        .into_iter()
        .filter(|r| {
            r.impressions >= 10.0 && r.keys.first().is_some_and(|q| !old.contains(q.as_str()))
        })
        .take(25)
    {
        // A truncated baseline cannot prove absence. Query the candidate directly before labeling it new.
        if baseline.next_start_row.is_some() {
            let mut q = query(baseline_start, baseline_end, &["query"]);
            q.filters.push(gsc::PerformanceFilter {
                dimension: "query".into(),
                operator: "equals".into(),
                expression: row.keys[0].clone(),
            });
            let checked = gsc::performance(pool, actor, pid, cfg, q).await?;
            if checked.site_url != site {
                return Err(AppError::Conflict(
                    "Search Console property changed during refresh".into(),
                ));
            }
            if !checked.rows.is_empty() {
                continue;
            }
        }
        new_queries.push(row);
    }
    top.rows.sort_by(|a, b| b.clicks.total_cmp(&a.clicks));
    pairs.rows.retain(is_striking);
    pairs
        .rows
        .sort_by(|a, b| b.impressions.total_cmp(&a.impressions));
    Ok(KeywordSnapshot{site_url:site.into(),start_date:start.to_string(),end_date:end.to_string(),recent_start_date:recent.to_string(),baseline_start_date:baseline_start.to_string(),baseline_end_date:baseline_end.to_string(),top:top.rows,new_queries,striking_distance:pairs.rows,truncated,note:"Source: Google Search Console, web search. Average positions, not live ranks. Up to 1,000 top rows per period/view; at most 25 new candidates checked. New means at least 10 impressions in 7 completed days and no returned observations in the preceding 28 days. Google may omit anonymized queries; this is not exhaustive. Dates use Pacific Time with a conservative four-day UTC lag.".into()})
}

pub fn register(registry: jobs::Registry, cfg: Config) -> jobs::Registry {
    let cfg = std::sync::Arc::new(cfg);
    registry.register("dashboard_refresh", move |ctx: jobs::JobCtx| {
        let cfg = cfg.clone();
        async move {
            let org = ctx
                .job
                .organization_id
                .ok_or_else(|| anyhow::anyhow!("missing organization"))?;
            let pid = ctx
                .job
                .project_id
                .ok_or_else(|| anyhow::anyhow!("missing project"))?;
            let user: Uuid = serde_json::from_value(ctx.job.payload["user_id"].clone())?;
            let identity = ctx.job.payload["identity"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing source identity"))?;
            let result = async {
                // Resolve membership again: queued work does not retain a removed member's authority.
                let role = db::orgs::role(&ctx.pool, org, user)
                    .await?
                    .ok_or(AppError::Unauthorized)?;
                let actor = Actor {
                    org_id: org,
                    user_id: user,
                    role: Role::parse(&role).ok_or(AppError::Unauthorized)?,
                    via: ActorVia::ApiKey,
                };
                let source = repo::source(&ctx.pool, org, pid).await?;
                if source.identity != identity {
                    return Err(AppError::Conflict(
                        "dashboard source changed; open the dashboard again".into(),
                    ));
                }
                let site = source
                    .site_url
                    .ok_or_else(|| AppError::Conflict("Search Console disconnected".into()))?;
                ctx.progress(
                    serde_json::json!({"message":"Refreshing Search Console keyword views"}),
                )
                .await?;
                let data = collect(&ctx.pool, &actor, pid, &cfg, &site).await?;
                serde_json::to_value(data).map_err(AppError::internal)
            }
            .await;
            let failed = result.is_err();
            let persisted = result.map_err(|e| match e {
                AppError::Internal(_) => "Dashboard refresh failed; check the server logs".into(),
                other => other.to_string(),
            });
            repo::finish_refresh(&ctx.pool, org, pid, ctx.job.id, identity, persisted).await?;
            if failed {
                anyhow::bail!(
                    "Dashboard refresh failed; saved data retained, next attempt in 15 minutes"
                );
            }
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn striking_distance_includes_boundaries_and_excludes_unknown() {
        for (position, expected) in [
            (4.9, false),
            (5.0, true),
            (20.0, true),
            (20.1, false),
            (f64::NAN, false),
        ] {
            let r = gsc::PerformanceRow {
                keys: vec![],
                clicks: 0.0,
                impressions: 0.0,
                ctr: 0.0,
                position,
            };
            assert_eq!(is_striking(&r), expected);
        }
    }
}
