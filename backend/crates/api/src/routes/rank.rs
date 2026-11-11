use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use db::rank::{RankPoint, RankRun, RankTracker, RankTrendPoint};
use domain::rank::{
    AddKeywordsInput, ChangeResult, CreateTrackerInput, HistoryQuery, KeywordIdsInput,
    PinKeywordsInput, RunEstimate, StartRunInput, TrackerView, UpdateTrackerInput,
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_rank_trackers, create_rank_tracker))
        .routes(routes!(
            get_rank_tracker,
            update_rank_tracker,
            delete_rank_tracker
        ))
        .routes(routes!(add_rank_keywords))
        .routes(routes!(remove_rank_keywords))
        .routes(routes!(pin_rank_keywords))
        .routes(routes!(start_rank_run))
        .routes(routes!(get_rank_history))
        .routes(routes!(get_rank_trend))
        .routes(routes!(estimate_rank_run))
}

#[utoipa::path(get, path = "/projects/{project_id}/rank-trackers", tag = "rank-tracking",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<RankTracker>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_rank_trackers(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<RankTracker>>> {
    Ok(Json(
        domain::rank::list(&state.pool, &actor, project_id).await?,
    ))
}

/// Track where a domain ranks for a set of keywords in one market.
#[utoipa::path(post, path = "/projects/{project_id}/rank-trackers", tag = "rank-tracking",
    params(("project_id" = Uuid, Path)), request_body = CreateTrackerInput,
    responses((status = CREATED, body = RankTracker), (status = CONFLICT, body = Problem),
              (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn create_rank_tracker(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateTrackerInput>,
) -> ApiResult<(StatusCode, Json<RankTracker>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::rank::create(&state.pool, &actor, project_id, input).await?),
    ))
}

/// The tracker with the latest position of every keyword (plus previous and best) and its
/// recent runs. A null `position` with a `checked_at` means "not in the first `depth`
/// results"; a null `checked_at` means "never checked".
#[utoipa::path(get, path = "/projects/{project_id}/rank-trackers/{tracker_id}", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)),
    responses((status = OK, body = TrackerView), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_rank_tracker(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<TrackerView>> {
    Ok(Json(
        domain::rank::get(&state.pool, &actor, project_id, tracker_id).await?,
    ))
}

#[utoipa::path(patch, path = "/projects/{project_id}/rank-trackers/{tracker_id}", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)), request_body = UpdateTrackerInput,
    responses((status = OK, body = RankTracker), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_rank_tracker(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateTrackerInput>,
) -> ApiResult<Json<RankTracker>> {
    Ok(Json(
        domain::rank::update(&state.pool, &actor, project_id, tracker_id, input).await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/rank-trackers/{tracker_id}", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_rank_tracker(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::rank::delete(&state.pool, &actor, project_id, tracker_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/projects/{project_id}/rank-trackers/{tracker_id}/keywords", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)), request_body = AddKeywordsInput,
    responses((status = OK, body = ChangeResult), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn add_rank_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AddKeywordsInput>,
) -> ApiResult<Json<ChangeResult>> {
    Ok(Json(
        domain::rank::add_keywords(&state.pool, &actor, project_id, tracker_id, input).await?,
    ))
}

/// Remove keywords and their position history.
#[utoipa::path(post, path = "/projects/{project_id}/rank-trackers/{tracker_id}/keywords/delete", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)), request_body = KeywordIdsInput,
    responses((status = OK, body = ChangeResult)), security(("session" = []), ("bearer" = [])))]
async fn remove_rank_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<KeywordIdsInput>,
) -> ApiResult<Json<ChangeResult>> {
    Ok(Json(
        domain::rank::remove_keywords(&state.pool, &actor, project_id, tracker_id, input).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/rank-trackers/{tracker_id}/keywords/pin", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)), request_body = PinKeywordsInput,
    responses((status = OK, body = ChangeResult)), security(("session" = []), ("bearer" = [])))]
async fn pin_rank_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<PinKeywordsInput>,
) -> ApiResult<Json<ChangeResult>> {
    Ok(Json(
        domain::rank::pin_keywords(&state.pool, &actor, project_id, tracker_id, input).await?,
    ))
}

/// Check positions now (all keywords or a subset). Runs in the background: poll the tracker
/// or `GET /jobs/{job_id}`. Every keyword costs one request to the search results provider.
#[utoipa::path(post, path = "/projects/{project_id}/rank-trackers/{tracker_id}/runs", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)), request_body = StartRunInput,
    responses((status = ACCEPTED, body = RankRun), (status = CONFLICT, body = Problem),
              (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn start_rank_run(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<StartRunInput>,
) -> ApiResult<(StatusCode, Json<RankRun>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(domain::rank::start_run(&state.pool, &actor, project_id, tracker_id, input).await?),
    ))
}

/// Position history of one keyword, newest first.
#[utoipa::path(get, path = "/projects/{project_id}/rank-trackers/{tracker_id}/history", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path), HistoryQuery),
    responses((status = OK, body = Vec<RankPoint>)), security(("session" = []), ("bearer" = [])))]
async fn get_rank_history(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<RankPoint>>> {
    Ok(Json(
        domain::rank::history(&state.pool, &actor, project_id, tracker_id, q).await?,
    ))
}

/// Per-run summary (ranking keywords, top 3, top 10, average position), oldest first.
#[utoipa::path(get, path = "/projects/{project_id}/rank-trackers/{tracker_id}/trend", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)),
    responses((status = OK, body = Vec<RankTrendPoint>)), security(("session" = []), ("bearer" = [])))]
async fn get_rank_trend(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<RankTrendPoint>>> {
    Ok(Json(
        domain::rank::trend(&state.pool, &actor, project_id, tracker_id).await?,
    ))
}

/// Request budget of a run: provider requests per run and per month. SeoSeo has no credits;
/// this only tells you what the configured provider will be asked to do.
#[utoipa::path(get, path = "/projects/{project_id}/rank-trackers/{tracker_id}/estimate", tag = "rank-tracking",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)),
    responses((status = OK, body = RunEstimate)), security(("session" = []), ("bearer" = [])))]
async fn estimate_rank_run(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<RunEstimate>> {
    Ok(Json(
        domain::rank::estimate(
            &state.pool,
            &actor,
            &state.providers,
            project_id,
            tracker_id,
        )
        .await?,
    ))
}
