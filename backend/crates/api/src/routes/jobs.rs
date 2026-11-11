use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use db::models::Job;
use domain::Role;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_jobs))
        .routes(routes!(job_summary))
        .routes(routes!(get_job))
        .routes(routes!(ping_worker))
}

#[derive(Deserialize, IntoParams)]
struct JobsQuery {
    /// queued | running | completed | failed | cancelled
    status: Option<String>,
    /// Default 50, max 200.
    limit: Option<i64>,
}

/// Recent jobs of the active workspace, newest first.
#[utoipa::path(get, path = "/jobs", tag = "jobs", params(JobsQuery),
    responses((status = OK, body = Vec<Job>)), security(("session" = []), ("bearer" = [])))]
async fn list_jobs(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Query(q): Query<JobsQuery>,
) -> ApiResult<Json<Vec<Job>>> {
    Ok(Json(
        db::jobs::list_filtered(
            &state.pool,
            actor.org_id,
            q.status.as_deref().filter(|s| !s.is_empty()),
            q.limit.unwrap_or(50).clamp(1, 200),
        )
        .await?,
    ))
}

#[derive(Serialize, ToSchema, Default)]
struct JobSummary {
    /// Counts over the last 7 days.
    queued: i64,
    running: i64,
    completed: i64,
    failed: i64,
    cancelled: i64,
}

/// Job counts by status for the last 7 days.
#[utoipa::path(get, path = "/jobs/summary", tag = "jobs",
    responses((status = OK, body = JobSummary)), security(("session" = []), ("bearer" = [])))]
async fn job_summary(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<JobSummary>> {
    let mut out = JobSummary::default();
    for (status, n) in db::jobs::status_counts(&state.pool, actor.org_id, 7).await? {
        match status.as_str() {
            "queued" => out.queued = n,
            "running" => out.running = n,
            "completed" => out.completed = n,
            "failed" => out.failed = n,
            "cancelled" => out.cancelled = n,
            _ => {}
        }
    }
    Ok(Json(out))
}

#[utoipa::path(get, path = "/jobs/{job_id}", tag = "jobs", params(("job_id" = Uuid, Path)),
    responses((status = OK, body = Job), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_job(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(job_id): Path<Uuid>,
) -> ApiResult<Json<Job>> {
    Ok(Json(
        db::jobs::get_for_org(&state.pool, actor.org_id, job_id).await?,
    ))
}

/// Enqueue a no-op job to check the worker is running.
#[utoipa::path(post, path = "/jobs/ping", tag = "jobs",
    responses((status = ACCEPTED, body = Job)), security(("session" = []), ("bearer" = [])))]
async fn ping_worker(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<(StatusCode, Json<Job>)> {
    actor.require(Role::Admin)?;
    let job = jobs::enqueue(
        &state.pool,
        "noop",
        Some(actor.org_id),
        None,
        serde_json::json!({}),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}
