//! `rank_check` job and the tracker scheduler.
//!
//! A run freezes its keyword list. Each keyword gets one fresh SERP fetch (rank checks are
//! never served from the research cache) and one `rank_results` row, so a retry after a
//! crash only fetches keywords that have no row yet. A provider block or rate limit fails
//! the run with the reason and keeps the results gathered so far.

use crate::JobCtx;
use crate::provider_ctx::ProviderCtx;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use common::{AppResult, new_id};
use db::rank::{self, NewResult, RankRun, RankTracker};
use db::serp::SnapshotItem;
use providers::Market;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

pub const KIND: &str = "rank_check";

#[derive(Deserialize)]
struct Payload {
    run_id: Uuid,
}

pub fn register(registry: crate::Registry, pctx: ProviderCtx) -> crate::Registry {
    registry.register(KIND, move |ctx: JobCtx| {
        let pctx = pctx.clone();
        async move { run(ctx, pctx).await }
    })
}

/// Create a run with a frozen keyword list and enqueue its job.
pub async fn enqueue_run(
    pool: &PgPool,
    org_id: Uuid,
    tracker: &RankTracker,
    trigger: &str,
    keyword_ids: &[Uuid],
) -> AppResult<RankRun> {
    let run = rank::insert_run(pool, new_id(), tracker.id, trigger, keyword_ids).await?;
    let job = db::jobs::enqueue(
        pool,
        new_id(),
        KIND,
        Some(org_id),
        Some(tracker.project_id),
        json!({ "run_id": run.id }),
        3,
    )
    .await?;
    rank::set_run_job(pool, run.id, job.id).await?;
    Ok(RankRun {
        job_id: Some(job.id),
        ..run
    })
}

async fn run(ctx: JobCtx, pctx: ProviderCtx) -> anyhow::Result<()> {
    let payload: Payload = serde_json::from_value(ctx.job.payload.clone())?;
    let pool = &ctx.pool;
    let (run, keyword_ids) = rank::get_run_any(pool, payload.run_id).await?;
    if matches!(run.status.as_str(), "completed" | "failed") {
        return Ok(());
    }
    match check(&ctx, &pctx, &run, &keyword_ids).await {
        Ok(()) => rank::finish_run(pool, run.id, None).await?,
        // Deterministic failures (no provider, blocked) are final: retrying would only
        // repeat requests the provider just refused.
        Err(e) => rank::finish_run(pool, run.id, Some(&e.to_string())).await?,
    }
    Ok(())
}

async fn check(
    ctx: &JobCtx,
    pctx: &ProviderCtx,
    run: &RankRun,
    keyword_ids: &[Uuid],
) -> AppResult<()> {
    let pool = &ctx.pool;
    let tracker = rank::get_tracker_any(pool, run.tracker_id).await?;
    let org_id = ctx
        .job
        .organization_id
        .ok_or_else(|| common::AppError::Conflict("job has no workspace".into()))?;
    let registry = pctx.registry(pool, org_id).await?;
    if !registry.has_serp() {
        return Err(common::AppError::Conflict(
            "No search results provider is set up. Start SearXNG (compose profile `serp`) or add a key in Settings → Data providers.".into(),
        ));
    }
    rank::start_run(pool, run.id).await?;
    let market = Market::new(&tracker.country, &tracker.language);
    let done: HashSet<Uuid> = rank::done_keyword_ids(pool, run.id)
        .await?
        .into_iter()
        .collect();
    let keywords = rank::keywords(pool, tracker.id, Some(keyword_ids)).await?;
    let total = keywords.len();
    for (n, k) in keywords.iter().enumerate() {
        if done.contains(&k.id) {
            continue;
        }
        ctx.progress(json!({ "done": n, "total": total })).await?;
        let (snap, items) =
            crate::serp::fetch_snapshot(pool, &registry, &k.keyword, &market).await?;
        let hit = find_position(&items, &tracker.domain);
        rank::insert_result(
            pool,
            &NewResult {
                run_id: run.id,
                keyword_id: k.id,
                position: hit.map(|i| i.position),
                url: hit.map(|i| i.url.as_str()),
                depth: items.len() as i32,
                provider: &snap.provider,
                exact: snap.exact,
                features: &serde_json::from_value::<Vec<String>>(snap.features.clone())
                    .unwrap_or_default(),
                snapshot_id: snap.id,
            },
        )
        .await?;
    }
    ctx.progress(json!({ "done": total, "total": total }))
        .await?;
    Ok(())
}

/// First result that belongs to `domain` (the registrable domain itself or a subdomain).
pub fn find_position<'a>(items: &'a [SnapshotItem], domain: &str) -> Option<&'a SnapshotItem> {
    items.iter().find(|i| {
        i.domain == domain
            || scoring::host_of(&i.url)
                .is_some_and(|h| h == domain || h.ends_with(&format!(".{domain}")))
    })
}

// ---------------------------------------------------------------- schedule

/// Next scheduled time strictly after `after`. `manual` has none.
/// daily: every day at `hour` UTC; weekly: on ISO `weekday`; monthly: on the 1st.
pub fn next_run(
    schedule: &str,
    hour: u32,
    weekday: u32,
    after: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let at = |d: NaiveDate| {
        d.and_hms_opt(hour.min(23), 0, 0)
            .map(|n| Utc.from_utc_datetime(&n))
    };
    let today = after.date_naive();
    match schedule {
        "daily" => (0..=1)
            .filter_map(|n| at(today + Duration::days(n)))
            .find(|t| *t > after),
        "weekly" => (0..=7)
            .map(|n| today + Duration::days(n))
            .filter(|d| d.weekday().number_from_monday() == weekday.clamp(1, 7))
            .filter_map(at)
            .find(|t| *t > after),
        "monthly" => {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)?;
            let (y, m) = if today.month() == 12 {
                (today.year() + 1, 1)
            } else {
                (today.year(), today.month() + 1)
            };
            [first, NaiveDate::from_ymd_opt(y, m, 1)?]
                .into_iter()
                .filter_map(at)
                .find(|t| *t > after)
        }
        _ => None,
    }
}

/// Start runs for trackers whose scheduled time has passed. Called periodically by every
/// worker; `claim_due` makes sure each tracker is picked up once.
pub async fn schedule_due(pool: &PgPool) -> AppResult<usize> {
    let due = rank::claim_due(pool, 20).await?;
    let mut started = 0;
    for t in due {
        let next = next_run(&t.schedule, t.hour_utc as u32, t.weekday as u32, Utc::now());
        rank::set_next_run(pool, t.id, next).await?;
        let Some((org_id, archived)) = rank::tracker_project(pool, t.project_id).await? else {
            continue;
        };
        // Skip quietly: archived project, nothing to check, or the previous run still going.
        if archived || rank::active_run(pool, t.id).await?.is_some() {
            continue;
        }
        let ids: Vec<Uuid> = rank::keywords(pool, t.id, None)
            .await?
            .into_iter()
            .map(|k| k.id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        enqueue_run(pool, org_id, &t, "schedule", &ids).await?;
        started += 1;
    }
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 30, 0).unwrap()
    }

    fn hour(y: i32, m: u32, d: u32, h: u32) -> Option<DateTime<Utc>> {
        Some(Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap())
    }

    #[test]
    fn schedules() {
        // 2026-11-11 is a Wednesday.
        assert_eq!(next_run("manual", 6, 1, t(2026, 11, 11, 3)), None);
        assert_eq!(
            next_run("daily", 6, 1, t(2026, 11, 11, 3)),
            hour(2026, 11, 11, 6)
        );
        assert_eq!(
            next_run("daily", 6, 1, t(2026, 11, 11, 6)),
            hour(2026, 11, 12, 6)
        );
        assert_eq!(
            next_run("weekly", 6, 3, t(2026, 11, 11, 3)),
            hour(2026, 11, 11, 6)
        );
        assert_eq!(
            next_run("weekly", 6, 3, t(2026, 11, 11, 9)),
            hour(2026, 11, 18, 6)
        );
        assert_eq!(
            next_run("weekly", 6, 1, t(2026, 11, 11, 9)),
            hour(2026, 11, 16, 6)
        );
        assert_eq!(
            next_run("monthly", 6, 1, t(2026, 11, 11, 9)),
            hour(2026, 12, 1, 6)
        );
        assert_eq!(
            next_run("monthly", 6, 1, t(2026, 12, 1, 3)),
            hour(2026, 12, 1, 6)
        );
        assert_eq!(
            next_run("monthly", 6, 1, t(2026, 12, 1, 9)),
            hour(2027, 1, 1, 6)
        );
    }

    #[test]
    fn matches_domain_and_subdomains_only() {
        let item = |pos, url: &str, domain: &str| SnapshotItem {
            position: pos,
            url: url.into(),
            domain: domain.into(),
            title: String::new(),
            snippet: None,
        };
        let items = vec![
            item(1, "https://notexample.com/", "notexample.com"),
            item(2, "https://example.com.evil.io/", "evil.io"),
            item(3, "https://blog.example.com/post", "example.com"),
            item(4, "https://example.com/", "example.com"),
        ];
        assert_eq!(find_position(&items, "example.com").unwrap().position, 3);
        assert!(find_position(&items, "missing.org").is_none());
        // A tracker on a subdomain matches only that host.
        let items = vec![
            item(1, "https://example.com/", "example.com"),
            item(2, "https://shop.example.com/x", "example.com"),
        ];
        assert_eq!(
            find_position(&items, "shop.example.com").unwrap().position,
            2
        );
    }
}
