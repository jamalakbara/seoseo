//! Rank tracking: trackers, keywords, runs and position reports.

use crate::actor::{Actor, Role};
use crate::validate;
use chrono::Utc;
use common::{AppError, AppResult, new_id};
use db::models::Project;
use db::rank::{
    self, NewTracker, RankPoint, RankRow, RankRun, RankTracker, RankTrendPoint, TrackerUpdate,
};
use jobs::ProviderCtx;
use jobs::rank_check::{enqueue_run, next_run};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub const MAX_TRACKERS_PER_PROJECT: i64 = 10;
pub const MAX_KEYWORDS_PER_TRACKER: i64 = 500;
const SCHEDULES: [&str; 4] = ["manual", "daily", "weekly", "monthly"];

async fn project(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Project> {
    db::projects::get(pool, actor.org_id, project_id).await
}

async fn tracker(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<(Project, RankTracker)> {
    let p = project(pool, actor, project_id).await?;
    let t = rank::get_tracker(pool, project_id, tracker_id).await?;
    Ok((p, t))
}

fn schedule(s: &str, hour: i16, weekday: i16) -> AppResult<()> {
    if !SCHEDULES.contains(&s) {
        return Err(AppError::Validation(
            "schedule must be manual, daily, weekly or monthly".into(),
        ));
    }
    if !(0..=23).contains(&hour) {
        return Err(AppError::Validation(
            "hour_utc must be between 0 and 23".into(),
        ));
    }
    if !(1..=7).contains(&weekday) {
        return Err(AppError::Validation(
            "weekday must be between 1 (Monday) and 7 (Sunday)".into(),
        ));
    }
    Ok(())
}

fn keywords(raw: &[String]) -> AppResult<Vec<String>> {
    let mut out = Vec::with_capacity(raw.len());
    for k in raw {
        let k = scoring::normalize_keyword(k);
        if k.is_empty() {
            continue;
        }
        if k.chars().count() > 120 || k.split_whitespace().count() > 12 {
            return Err(AppError::Validation(format!(
                "`{k}` is too long: a keyword can have at most 120 characters and 12 words"
            )));
        }
        if !out.contains(&k) {
            out.push(k);
        }
    }
    Ok(out)
}

fn ids(ids: &[Uuid]) -> AppResult<()> {
    if ids.is_empty() || ids.len() > 1000 {
        return Err(AppError::Validation("pass 1 to 1000 keyword ids".into()));
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct CreateTrackerInput {
    /// Defaults to the tracked domain.
    pub name: Option<String>,
    /// Domain to track; defaults to the project's website. Subdomains count as the domain.
    pub domain: Option<String>,
    /// Defaults to the project's market.
    pub country: Option<String>,
    pub language: Option<String>,
    /// manual (default) | daily | weekly | monthly
    pub schedule: Option<String>,
    /// Hour of day in UTC for scheduled runs (default 6).
    pub hour_utc: Option<i16>,
    /// ISO weekday for weekly runs, 1 = Monday (default 1).
    pub weekday: Option<i16>,
    /// Keywords to start with.
    pub keywords: Option<Vec<String>>,
}

pub async fn create(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: CreateTrackerInput,
) -> AppResult<RankTracker> {
    actor.require(Role::Member)?;
    let p = project(pool, actor, project_id).await?;
    let raw_domain = input
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .or(p.domain.as_deref())
        .ok_or_else(|| {
            AppError::Validation("pass a domain or set the project's website first".into())
        })?;
    let host = validate::domain(raw_domain)?;
    let domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let country = validate::country(input.country.as_deref().unwrap_or(&p.country_code))?;
    let language = validate::language(input.language.as_deref().unwrap_or(&p.language_code))?;
    let sched = input.schedule.as_deref().unwrap_or("manual");
    let hour = input.hour_utc.unwrap_or(6);
    let weekday = input.weekday.unwrap_or(1);
    schedule(sched, hour, weekday)?;
    let name = validate::non_empty("name", input.name.as_deref().unwrap_or(&domain), 120)?;
    let kws = keywords(&input.keywords.unwrap_or_default())?;
    if kws.len() as i64 > MAX_KEYWORDS_PER_TRACKER {
        return Err(AppError::Validation(format!(
            "a tracker can hold {MAX_KEYWORDS_PER_TRACKER} keywords"
        )));
    }
    if rank::count_trackers(pool, project_id).await? >= MAX_TRACKERS_PER_PROJECT {
        return Err(AppError::Conflict(format!(
            "a project can have {MAX_TRACKERS_PER_PROJECT} rank trackers; delete one first"
        )));
    }
    let id = new_id();
    rank::insert_tracker(
        pool,
        &NewTracker {
            id,
            project_id,
            name: &name,
            domain: &domain,
            country: &country,
            language: &language,
            schedule: sched,
            hour_utc: hour,
            weekday,
            next_run_at: next_run(sched, hour as u32, weekday as u32, Utc::now()),
            created_by: actor.user_id,
        },
    )
    .await?;
    rank::add_keywords(pool, id, &kws, &[]).await?;
    rank::get_tracker(pool, project_id, id).await
}

pub async fn list(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Vec<RankTracker>> {
    project(pool, actor, project_id).await?;
    rank::list_trackers(pool, project_id).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrackerView {
    pub tracker: RankTracker,
    /// Latest position per keyword, pinned keywords first.
    pub keywords: Vec<RankRow>,
    /// Recent runs, newest first.
    pub runs: Vec<RankRun>,
}

pub async fn get(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<TrackerView> {
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    Ok(TrackerView {
        keywords: rank::latest(pool, t.id).await?,
        runs: rank::list_runs(pool, t.id, 20).await?,
        tracker: t,
    })
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateTrackerInput {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub hour_utc: Option<i16>,
    pub weekday: Option<i16>,
}

pub async fn update(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    input: UpdateTrackerInput,
) -> AppResult<RankTracker> {
    actor.require(Role::Member)?;
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    let name = validate::non_empty("name", input.name.as_deref().unwrap_or(&t.name), 120)?;
    let sched = input.schedule.unwrap_or(t.schedule);
    let hour = input.hour_utc.unwrap_or(t.hour_utc);
    let weekday = input.weekday.unwrap_or(t.weekday);
    schedule(&sched, hour, weekday)?;
    rank::update_tracker(
        pool,
        project_id,
        tracker_id,
        &TrackerUpdate {
            name: &name,
            schedule: &sched,
            hour_utc: hour,
            weekday,
            next_run_at: next_run(&sched, hour as u32, weekday as u32, Utc::now()),
        },
    )
    .await
}

pub async fn delete(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    actor.require(Role::Admin)?;
    project(pool, actor, project_id).await?;
    rank::delete_tracker(pool, project_id, tracker_id).await
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddKeywordsInput {
    pub keywords: Vec<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChangeResult {
    /// Rows added, removed or updated.
    pub changed: u64,
    /// Keywords now on the tracker.
    pub keyword_count: i64,
}

pub async fn add_keywords(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    input: AddKeywordsInput,
) -> AppResult<ChangeResult> {
    actor.require(Role::Member)?;
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    let kws = keywords(&input.keywords)?;
    if kws.is_empty() {
        return Err(AppError::Validation("pass at least one keyword".into()));
    }
    let mut tags: Vec<String> = input
        .tags
        .unwrap_or_default()
        .iter()
        .map(|t| scoring::normalize_keyword(t))
        .filter(|t| !t.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    if tags.len() > 10 || tags.iter().any(|t| t.chars().count() > 30) {
        return Err(AppError::Validation(
            "at most 10 tags of up to 30 characters each".into(),
        ));
    }
    if rank::count_keywords(pool, t.id).await? + kws.len() as i64 > MAX_KEYWORDS_PER_TRACKER {
        return Err(AppError::Conflict(format!(
            "a tracker can hold {MAX_KEYWORDS_PER_TRACKER} keywords; remove some first"
        )));
    }
    Ok(ChangeResult {
        changed: rank::add_keywords(pool, t.id, &kws, &tags).await?,
        keyword_count: rank::count_keywords(pool, t.id).await?,
    })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct KeywordIdsInput {
    pub ids: Vec<Uuid>,
}

pub async fn remove_keywords(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    input: KeywordIdsInput,
) -> AppResult<ChangeResult> {
    actor.require(Role::Member)?;
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    ids(&input.ids)?;
    Ok(ChangeResult {
        changed: rank::remove_keywords(pool, t.id, &input.ids).await?,
        keyword_count: rank::count_keywords(pool, t.id).await?,
    })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PinKeywordsInput {
    pub ids: Vec<Uuid>,
    /// Pinned keywords are listed first. Default true.
    pub pinned: Option<bool>,
}

pub async fn pin_keywords(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    input: PinKeywordsInput,
) -> AppResult<ChangeResult> {
    actor.require(Role::Member)?;
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    ids(&input.ids)?;
    Ok(ChangeResult {
        changed: rank::pin_keywords(pool, t.id, &input.ids, input.pinned.unwrap_or(true)).await?,
        keyword_count: rank::count_keywords(pool, t.id).await?,
    })
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct StartRunInput {
    /// Check only these keywords; omit for all.
    pub keyword_ids: Option<Vec<Uuid>>,
}

pub async fn start_run(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    input: StartRunInput,
) -> AppResult<RankRun> {
    actor.require(Role::Member)?;
    let (p, t) = tracker(pool, actor, project_id, tracker_id).await?;
    if p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "restore the project before checking rankings".into(),
        ));
    }
    if rank::active_run(pool, t.id).await?.is_some() {
        return Err(AppError::Conflict(
            "a rank check is already running for this tracker; wait for it to finish".into(),
        ));
    }
    let subset = input.keyword_ids.filter(|v| !v.is_empty());
    let kw_ids: Vec<Uuid> = rank::keywords(pool, t.id, subset.as_deref())
        .await?
        .into_iter()
        .map(|k| k.id)
        .collect();
    if kw_ids.is_empty() {
        return Err(AppError::Validation(
            "add keywords to the tracker before running a check".into(),
        ));
    }
    enqueue_run(pool, actor.org_id, &t, "manual", &kw_ids).await
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct HistoryQuery {
    pub keyword_id: Uuid,
    /// Most recent checks to return (default 90, max 365).
    pub limit: Option<i64>,
}

pub async fn history(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
    q: HistoryQuery,
) -> AppResult<Vec<RankPoint>> {
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    rank::history(
        pool,
        t.id,
        q.keyword_id,
        q.limit.unwrap_or(90).clamp(1, 365),
    )
    .await
}

pub async fn trend(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<Vec<RankTrendPoint>> {
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    rank::trend(pool, t.id, 60).await
}

/// What a check costs in provider requests (there are no credits or fees in SeoSeo itself).
#[derive(Debug, Serialize, ToSchema)]
pub struct RunEstimate {
    pub keywords: i64,
    /// One search request per keyword.
    pub requests_per_run: i64,
    /// Scheduled runs in a 30-day month (0 for manual).
    pub runs_per_month: i64,
    pub requests_per_month: i64,
    /// Provider that would be used first; null when none is set up.
    pub provider: Option<String>,
    /// Rough duration of one run given the provider's pacing; null without a provider.
    pub est_seconds: Option<i64>,
    /// What to do before a run can work, if anything.
    pub note: Option<String>,
}

pub async fn estimate(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<RunEstimate> {
    let (_, t) = tracker(pool, actor, project_id, tracker_id).await?;
    let registry = pctx.registry(pool, actor.org_id).await?;
    let provider = registry.serp.first().map(|p| p.id().to_string());
    // Seconds between requests enforced by each adapter.
    let pace = match provider.as_deref() {
        Some("serp_serper") => Some(0.5),
        Some("serp_brave") => Some(1.2),
        Some("serp_searxng") => Some(2.5),
        Some("serp_chromium") => Some(10.0),
        Some(_) => Some(2.0),
        None => None,
    };
    let runs = match t.schedule.as_str() {
        "daily" => 30,
        "weekly" => 4,
        "monthly" => 1,
        _ => 0,
    };
    Ok(RunEstimate {
        keywords: t.keyword_count,
        requests_per_run: t.keyword_count,
        runs_per_month: runs,
        requests_per_month: t.keyword_count * runs,
        est_seconds: pace.map(|p| (t.keyword_count as f64 * p).ceil() as i64),
        note: provider.is_none().then(|| {
            "No search results provider is set up. Start SearXNG (compose profile `serp`) or add a key in Settings → Data providers.".to_string()
        }),
        provider,
    })
}
