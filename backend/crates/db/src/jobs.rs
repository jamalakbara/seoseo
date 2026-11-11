//! Durable job queue on Postgres. Workers claim with `FOR UPDATE SKIP LOCKED` and hold a lease.

use crate::internal;
use crate::models::Job;
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

macro_rules! job_columns {
    () => {
        "id, kind, organization_id, project_id, payload, status, attempts, max_attempts, \
                       last_error, progress, run_at, created_at, finished_at"
    };
}

pub async fn enqueue(
    pool: &PgPool,
    id: Uuid,
    kind: &str,
    org_id: Option<Uuid>,
    project_id: Option<Uuid>,
    payload: serde_json::Value,
    max_attempts: i32,
) -> AppResult<Job> {
    sqlx::query_as(concat!(
        "INSERT INTO jobs (id, kind, organization_id, project_id, payload, max_attempts)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(id)
    .bind(kind)
    .bind(org_id)
    .bind(project_id)
    .bind(payload)
    .bind(max_attempts)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

/// Claim the next ready job and lease it to `worker` for `lease_secs`.
pub async fn claim(pool: &PgPool, worker: &str, lease_secs: i64) -> AppResult<Option<Job>> {
    sqlx::query_as(concat!(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, locked_by = $1,
                locked_until = now() + make_interval(secs => $2), updated_at = now()
         WHERE id = (
            SELECT id FROM jobs WHERE status = 'queued' AND run_at <= now()
            ORDER BY run_at FOR UPDATE SKIP LOCKED LIMIT 1
         ) RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(worker)
    .bind(lease_secs as f64)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// Extend the lease of a running job (heartbeat) and optionally store progress.
pub async fn heartbeat(
    pool: &PgPool,
    id: Uuid,
    worker: &str,
    lease_secs: i64,
    progress: Option<serde_json::Value>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE jobs SET locked_until = now() + make_interval(secs => $3),
                progress = COALESCE($4, progress), updated_at = now()
         WHERE id = $1 AND locked_by = $2 AND status = 'running'",
    )
    .bind(id)
    .bind(worker)
    .bind(lease_secs as f64)
    .bind(progress)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn complete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'completed', locked_by = NULL, locked_until = NULL,
                finished_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Record a failure: requeue with exponential backoff, or mark failed after max attempts.
pub async fn fail(pool: &PgPool, id: Uuid, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE jobs SET
            status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
            run_at = now() + make_interval(secs => power(2, attempts)::float8 * 15),
            finished_at = CASE WHEN attempts >= max_attempts THEN now() ELSE NULL END,
            last_error = left($2, 2000), locked_by = NULL, locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Watchdog: jobs whose lease expired (worker crashed) are requeued or failed with a reason.
pub async fn reap_expired(pool: &PgPool) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE jobs SET
            status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END,
            finished_at = CASE WHEN attempts >= max_attempts THEN now() ELSE NULL END,
            last_error = 'worker stopped responding (lease expired)',
            locked_by = NULL, locked_until = NULL, updated_at = now()
         WHERE status = 'running' AND locked_until < now()",
    )
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(r.rows_affected())
}

/// Fetch a job visible to an organization.
pub async fn get_for_org(pool: &PgPool, org_id: Uuid, id: Uuid) -> AppResult<Job> {
    sqlx::query_as(concat!(
        "SELECT ",
        job_columns!(),
        " FROM jobs WHERE id = $1 AND organization_id = $2"
    ))
    .bind(id)
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("job"))
}

pub async fn list_for_org(pool: &PgPool, org_id: Uuid, limit: i64) -> AppResult<Vec<Job>> {
    list_filtered(pool, org_id, None, limit).await
}

/// Jobs of an organization, newest first, optionally only one status.
pub async fn list_filtered(
    pool: &PgPool,
    org_id: Uuid,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<Job>> {
    sqlx::query_as(concat!(
        "SELECT ",
        job_columns!(),
        " FROM jobs WHERE organization_id = $1 AND ($2::text IS NULL OR status = $2)
          ORDER BY created_at DESC LIMIT $3"
    ))
    .bind(org_id)
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// `(status, count)` of an organization's jobs created in the last `days` days.
pub async fn status_counts(
    pool: &PgPool,
    org_id: Uuid,
    days: i32,
) -> AppResult<Vec<(String, i64)>> {
    sqlx::query_as(
        "SELECT status, count(*) FROM jobs
         WHERE organization_id = $1 AND created_at > now() - make_interval(days => $2)
         GROUP BY status",
    )
    .bind(org_id)
    .bind(days)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// The queued or running job of a kind, if any (instance-wide; used for singleton jobs).
pub async fn active_of_kind(pool: &PgPool, kind: &str) -> AppResult<Option<Job>> {
    sqlx::query_as(concat!(
        "SELECT ",
        job_columns!(),
        " FROM jobs WHERE kind = $1 AND status IN ('queued', 'running')
          ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(kind)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}
