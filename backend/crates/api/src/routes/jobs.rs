use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use db::models::Job;
use domain::Role;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_jobs))
        .routes(routes!(get_job))
        .routes(routes!(ping_worker))
}

/// Recent jobs of the active workspace.
#[utoipa::path(get, path = "/jobs", tag = "jobs",
    responses((status = OK, body = Vec<Job>)), security(("session" = []), ("bearer" = [])))]
async fn list_jobs(State(state): State<AppState>, Auth(actor): Auth) -> ApiResult<Json<Vec<Job>>> {
    Ok(Json(
        db::jobs::list_for_org(&state.pool, actor.org_id, 50).await?,
    ))
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
