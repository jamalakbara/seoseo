//! Rank tracking persistence: trackers, their keywords, runs and per-keyword results.
//! Read queries take the project id so callers stay tenant-scoped.

use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! tracker_columns {
    () => {
        "t.id, t.project_id, t.name, t.domain, t.country, t.language, t.schedule, t.hour_utc, \
         t.weekday, t.next_run_at, t.last_run_at, t.created_at, \
         (SELECT count(*) FROM rank_tracker_keywords k WHERE k.tracker_id = t.id) AS keyword_count"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RankTracker {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    /// Registrable domain whose positions are tracked.
    pub domain: String,
    pub country: String,
    pub language: String,
    /// manual | daily | weekly | monthly
    pub schedule: String,
    /// Hour of day (UTC) of scheduled runs.
    pub hour_utc: i16,
    /// ISO weekday (1 = Monday) of weekly runs.
    pub weekday: i16,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub keyword_count: i64,
}

pub struct NewTracker<'a> {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: &'a str,
    pub domain: &'a str,
    pub country: &'a str,
    pub language: &'a str,
    pub schedule: &'a str,
    pub hour_utc: i16,
    pub weekday: i16,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
}

pub async fn insert_tracker(pool: &PgPool, t: &NewTracker<'_>) -> AppResult<RankTracker> {
    sqlx::query(
        "INSERT INTO rank_trackers (id, project_id, name, domain, country, language, schedule,
                hour_utc, weekday, next_run_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(t.id)
    .bind(t.project_id)
    .bind(t.name)
    .bind(t.domain)
    .bind(t.country)
    .bind(t.language)
    .bind(t.schedule)
    .bind(t.hour_utc)
    .bind(t.weekday)
    .bind(t.next_run_at)
    .bind(t.created_by)
    .execute(pool)
    .await
    .map_err(internal)?;
    get_tracker(pool, t.project_id, t.id).await
}

pub async fn get_tracker(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<RankTracker> {
    sqlx::query_as(concat!(
        "SELECT ",
        tracker_columns!(),
        " FROM rank_trackers t WHERE t.id = $1 AND t.project_id = $2"
    ))
    .bind(id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("rank tracker"))
}

/// Unscoped read for the worker.
pub async fn get_tracker_any(pool: &PgPool, id: Uuid) -> AppResult<RankTracker> {
    sqlx::query_as(concat!(
        "SELECT ",
        tracker_columns!(),
        " FROM rank_trackers t WHERE t.id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("rank tracker"))
}

pub async fn list_trackers(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<RankTracker>> {
    sqlx::query_as(concat!(
        "SELECT ",
        tracker_columns!(),
        " FROM rank_trackers t WHERE t.project_id = $1 ORDER BY t.created_at"
    ))
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn count_trackers(pool: &PgPool, project_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM rank_trackers WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

pub struct TrackerUpdate<'a> {
    pub name: &'a str,
    pub schedule: &'a str,
    pub hour_utc: i16,
    pub weekday: i16,
    pub next_run_at: Option<DateTime<Utc>>,
}

pub async fn update_tracker(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
    u: &TrackerUpdate<'_>,
) -> AppResult<RankTracker> {
    let r = sqlx::query(
        "UPDATE rank_trackers SET name = $3, schedule = $4, hour_utc = $5, weekday = $6,
                next_run_at = $7, updated_at = now()
         WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project_id)
    .bind(u.name)
    .bind(u.schedule)
    .bind(u.hour_utc)
    .bind(u.weekday)
    .bind(u.next_run_at)
    .execute(pool)
    .await
    .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("rank tracker"));
    }
    get_tracker(pool, project_id, id).await
}

pub async fn delete_tracker(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query("DELETE FROM rank_trackers WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(project_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("rank tracker"));
    }
    Ok(())
}

// ---------------------------------------------------------------- keywords

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrackedKeyword {
    pub id: Uuid,
    pub keyword: String,
}

/// Add keywords; existing ones are left alone. Returns how many were new.
pub async fn add_keywords(
    pool: &PgPool,
    tracker_id: Uuid,
    keywords: &[String],
    tags: &[String],
) -> AppResult<u64> {
    if keywords.is_empty() {
        return Ok(0);
    }
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO rank_tracker_keywords (id, tracker_id, keyword, tags) ");
    qb.push_values(keywords, |mut b, k| {
        b.push_bind(common::new_id())
            .push_bind(tracker_id)
            .push_bind(k)
            .push_bind(tags);
    });
    qb.push(" ON CONFLICT (tracker_id, keyword) DO NOTHING");
    let r = qb.build().execute(pool).await.map_err(internal)?;
    Ok(r.rows_affected())
}

pub async fn count_keywords(pool: &PgPool, tracker_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM rank_tracker_keywords WHERE tracker_id = $1")
        .bind(tracker_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

pub async fn remove_keywords(pool: &PgPool, tracker_id: Uuid, ids: &[Uuid]) -> AppResult<u64> {
    let r = sqlx::query("DELETE FROM rank_tracker_keywords WHERE tracker_id = $1 AND id = ANY($2)")
        .bind(tracker_id)
        .bind(ids)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(r.rows_affected())
}

pub async fn pin_keywords(
    pool: &PgPool,
    tracker_id: Uuid,
    ids: &[Uuid],
    pinned: bool,
) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE rank_tracker_keywords SET pinned = $3 WHERE tracker_id = $1 AND id = ANY($2)",
    )
    .bind(tracker_id)
    .bind(ids)
    .bind(pinned)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(r.rows_affected())
}

/// Keywords of a tracker, optionally restricted to `ids`.
pub async fn keywords(
    pool: &PgPool,
    tracker_id: Uuid,
    ids: Option<&[Uuid]>,
) -> AppResult<Vec<TrackedKeyword>> {
    sqlx::query_as(
        "SELECT id, keyword FROM rank_tracker_keywords
         WHERE tracker_id = $1 AND ($2::uuid[] IS NULL OR id = ANY($2)) ORDER BY keyword",
    )
    .bind(tracker_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

// ---------------------------------------------------------------- runs

macro_rules! run_columns {
    () => {
        "id, tracker_id, job_id, status, trigger, keywords_total, keywords_done, error, \
         created_at, finished_at"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RankRun {
    pub id: Uuid,
    pub tracker_id: Uuid,
    pub job_id: Option<Uuid>,
    /// queued | running | completed | failed
    pub status: String,
    /// manual | schedule
    pub trigger: String,
    pub keywords_total: i32,
    pub keywords_done: i32,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub async fn insert_run(
    pool: &PgPool,
    id: Uuid,
    tracker_id: Uuid,
    trigger: &str,
    keyword_ids: &[Uuid],
) -> AppResult<RankRun> {
    sqlx::query_as(concat!(
        "INSERT INTO rank_runs (id, tracker_id, trigger, keyword_ids, keywords_total)
         VALUES ($1, $2, $3, $4, $5) RETURNING ",
        run_columns!()
    ))
    .bind(id)
    .bind(tracker_id)
    .bind(trigger)
    .bind(keyword_ids)
    .bind(keyword_ids.len() as i32)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn set_run_job(pool: &PgPool, id: Uuid, job_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE rank_runs SET job_id = $2 WHERE id = $1")
        .bind(id)
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn get_run_any(pool: &PgPool, id: Uuid) -> AppResult<(RankRun, Vec<Uuid>)> {
    let run: RankRun = sqlx::query_as(concat!(
        "SELECT ",
        run_columns!(),
        " FROM rank_runs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("rank run"))?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT keyword_ids FROM rank_runs WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(internal)?;
    Ok((run, ids))
}

pub async fn list_runs(pool: &PgPool, tracker_id: Uuid, limit: i64) -> AppResult<Vec<RankRun>> {
    sqlx::query_as(concat!(
        "SELECT ",
        run_columns!(),
        " FROM rank_runs WHERE tracker_id = $1 ORDER BY created_at DESC LIMIT $2"
    ))
    .bind(tracker_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn active_run(pool: &PgPool, tracker_id: Uuid) -> AppResult<Option<RankRun>> {
    sqlx::query_as(concat!(
        "SELECT ",
        run_columns!(),
        " FROM rank_runs WHERE tracker_id = $1 AND status IN ('queued', 'running')
          ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(tracker_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn start_run(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE rank_runs SET status = 'running' WHERE id = $1 AND status = 'queued'")
        .bind(id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn finish_run(pool: &PgPool, id: Uuid, error: Option<&str>) -> AppResult<()> {
    sqlx::query(
        "UPDATE rank_runs r SET status = CASE WHEN $2::text IS NULL THEN 'completed' ELSE 'failed' END,
                error = left($2, 1000), finished_at = now(),
                keywords_done = (SELECT count(*) FROM rank_results x WHERE x.run_id = r.id)
         WHERE r.id = $1 AND r.status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await
    .map_err(internal)?;
    sqlx::query(
        "UPDATE rank_trackers t SET last_run_at = now()
         FROM rank_runs r WHERE r.id = $1 AND t.id = r.tracker_id",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub struct NewResult<'a> {
    pub run_id: Uuid,
    pub keyword_id: Uuid,
    pub position: Option<i32>,
    pub url: Option<&'a str>,
    pub depth: i32,
    pub provider: &'a str,
    pub exact: bool,
    pub features: &'a [String],
    pub snapshot_id: Uuid,
}

pub async fn insert_result(pool: &PgPool, r: &NewResult<'_>) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO rank_results (run_id, keyword_id, position, url, depth, provider, exact, features, snapshot_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (run_id, keyword_id) DO NOTHING",
    )
    .bind(r.run_id)
    .bind(r.keyword_id)
    .bind(r.position)
    .bind(r.url)
    .bind(r.depth)
    .bind(r.provider)
    .bind(r.exact)
    .bind(r.features)
    .bind(r.snapshot_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    sqlx::query(
        "UPDATE rank_runs r SET keywords_done = (SELECT count(*) FROM rank_results x WHERE x.run_id = r.id)
         WHERE r.id = $1",
    )
    .bind(r.run_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Keyword ids of a run that already have a result (resume after a crash).
pub async fn done_keyword_ids(pool: &PgPool, run_id: Uuid) -> AppResult<Vec<Uuid>> {
    sqlx::query_scalar("SELECT keyword_id FROM rank_results WHERE run_id = $1")
        .bind(run_id)
        .fetch_all(pool)
        .await
        .map_err(internal)
}

// ---------------------------------------------------------------- scheduler

/// Claim trackers whose scheduled time has passed and move their `next_run_at` forward by
/// one period so two workers never start the same run. The caller recomputes the exact
/// next time afterwards.
pub async fn claim_due(pool: &PgPool, limit: i64) -> AppResult<Vec<RankTracker>> {
    sqlx::query_as(concat!(
        "WITH due AS (
            SELECT id FROM rank_trackers
            WHERE next_run_at <= now() AND schedule <> 'manual'
            ORDER BY next_run_at FOR UPDATE SKIP LOCKED LIMIT $1
         )
         UPDATE rank_trackers t SET next_run_at = now() + interval '1 hour'
         FROM due WHERE t.id = due.id RETURNING ",
        tracker_columns!()
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn set_next_run(pool: &PgPool, id: Uuid, at: Option<DateTime<Utc>>) -> AppResult<()> {
    sqlx::query("UPDATE rank_trackers SET next_run_at = $2 WHERE id = $1")
        .bind(id)
        .bind(at)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// `(organization, archived)` of a tracker's project, for the scheduler.
pub async fn tracker_project(pool: &PgPool, project_id: Uuid) -> AppResult<Option<(Uuid, bool)>> {
    sqlx::query_as("SELECT organization_id, archived_at IS NOT NULL FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}

// ---------------------------------------------------------------- reads

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RankRow {
    pub keyword_id: Uuid,
    pub keyword: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    /// Latest position; null = not in the first `depth` results, or never checked
    /// (see `checked_at`).
    pub position: Option<i32>,
    /// Position in the check before the latest one.
    pub previous_position: Option<i32>,
    /// Best position ever measured.
    pub best_position: Option<i32>,
    /// Ranking URL in the latest check.
    pub url: Option<String>,
    /// Results examined in the latest check.
    pub depth: Option<i32>,
    pub provider: Option<String>,
    /// True when the latest check used one search engine's real ranking.
    pub exact: Option<bool>,
    pub features: Option<Vec<String>>,
    /// Null when the keyword was never checked.
    pub checked_at: Option<DateTime<Utc>>,
    pub previous_checked_at: Option<DateTime<Utc>>,
}

/// Latest result per keyword with the previous and best positions. Pinned keywords first.
pub async fn latest(pool: &PgPool, tracker_id: Uuid) -> AppResult<Vec<RankRow>> {
    sqlx::query_as(
        "SELECT k.id AS keyword_id, k.keyword, k.pinned, k.tags,
                l.position, p.position AS previous_position,
                (SELECT min(position) FROM rank_results b WHERE b.keyword_id = k.id) AS best_position,
                l.url, l.depth, l.provider, l.exact, l.features, l.checked_at,
                p.checked_at AS previous_checked_at
         FROM rank_tracker_keywords k
         LEFT JOIN LATERAL (
            SELECT * FROM rank_results r WHERE r.keyword_id = k.id ORDER BY r.checked_at DESC LIMIT 1
         ) l ON true
         LEFT JOIN LATERAL (
            SELECT * FROM rank_results r WHERE r.keyword_id = k.id ORDER BY r.checked_at DESC
            OFFSET 1 LIMIT 1
         ) p ON true
         WHERE k.tracker_id = $1
         ORDER BY k.pinned DESC, l.position ASC NULLS LAST, k.keyword",
    )
    .bind(tracker_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RankPoint {
    pub checked_at: DateTime<Utc>,
    /// Null = not in the first `depth` results at that time.
    pub position: Option<i32>,
    pub url: Option<String>,
    pub depth: i32,
    pub provider: String,
}

pub async fn history(
    pool: &PgPool,
    tracker_id: Uuid,
    keyword_id: Uuid,
    limit: i64,
) -> AppResult<Vec<RankPoint>> {
    sqlx::query_as(
        "SELECT r.checked_at, r.position, r.url, r.depth, r.provider
         FROM rank_results r JOIN rank_tracker_keywords k ON k.id = r.keyword_id
         WHERE k.tracker_id = $1 AND k.id = $2 ORDER BY r.checked_at DESC LIMIT $3",
    )
    .bind(tracker_id)
    .bind(keyword_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RankTrendPoint {
    pub run_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub checked: i64,
    pub ranking: i64,
    pub top3: i64,
    pub top10: i64,
    /// Average position of the keywords that rank; null when none do.
    pub avg_position: Option<f64>,
}

/// Per-run summary of completed runs, oldest first.
pub async fn trend(pool: &PgPool, tracker_id: Uuid, limit: i64) -> AppResult<Vec<RankTrendPoint>> {
    sqlx::query_as(
        "SELECT * FROM (
            SELECT r.id AS run_id, r.created_at, count(x.*) AS checked,
                   count(x.position) AS ranking,
                   count(*) FILTER (WHERE x.position <= 3) AS top3,
                   count(*) FILTER (WHERE x.position <= 10) AS top10,
                   avg(x.position)::float8 AS avg_position
            FROM rank_runs r JOIN rank_results x ON x.run_id = r.id
            WHERE r.tracker_id = $1 AND r.status = 'completed'
            GROUP BY r.id ORDER BY r.created_at DESC LIMIT $2
         ) t ORDER BY created_at",
    )
    .bind(tracker_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}
