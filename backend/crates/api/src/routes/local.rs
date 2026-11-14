use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use db::local::LocalTracker;
use domain::local::{
    BusinessCategory, BusinessSearchInput, BusinessSearchResult, CreateLocalTrackerInput,
    LocalGridPreview,
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_categories))
        .routes(routes!(search_businesses))
        .routes(routes!(list_trackers, create_tracker))
        .routes(routes!(grid_preview))
}

#[utoipa::path(get, path = "/local/categories", tag = "local-seo",
    responses((status = OK, body = Vec<BusinessCategory>)), security(("session" = []), ("bearer" = [])))]
async fn list_categories(Auth(_actor): Auth) -> Json<&'static [BusinessCategory]> {
    Json(domain::local::categories())
}

/// Explicit, cached OpenStreetMap business lookup. This is discovery, not a local rank result.
#[utoipa::path(post, path = "/projects/{project_id}/local/businesses/search", tag = "local-seo",
    params(("project_id" = Uuid, Path)), request_body = BusinessSearchInput,
    responses((status = OK, body = BusinessSearchResult), (status = UNPROCESSABLE_ENTITY, body = Problem),
              (status = CONFLICT, body = Problem)), security(("session" = []), ("bearer" = [])))]
async fn search_businesses(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<BusinessSearchInput>,
) -> ApiResult<Json<BusinessSearchResult>> {
    Ok(Json(
        domain::local::search_businesses(&state.pool, &actor, project_id, &state.cfg, input)
            .await?,
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/local-trackers", tag = "local-seo",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<LocalTracker>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_trackers(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<LocalTracker>>> {
    Ok(Json(
        domain::local::list_trackers(&state.pool, &actor, project_id).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/local-trackers", tag = "local-seo",
    params(("project_id" = Uuid, Path)), request_body = CreateLocalTrackerInput,
    responses((status = CREATED, body = LocalTracker), (status = UNPROCESSABLE_ENTITY, body = Problem),
              (status = CONFLICT, body = Problem)), security(("session" = []), ("bearer" = [])))]
async fn create_tracker(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateLocalTrackerInput>,
) -> ApiResult<(StatusCode, Json<LocalTracker>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::local::create_tracker(&state.pool, &actor, project_id, input).await?),
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/local-trackers/{tracker_id}/grid", tag = "local-seo",
    params(("project_id" = Uuid, Path), ("tracker_id" = Uuid, Path)),
    responses((status = OK, body = LocalGridPreview), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn grid_preview(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, tracker_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<LocalGridPreview>> {
    Ok(Json(
        domain::local::grid_preview(&state.pool, &actor, project_id, tracker_id).await?,
    ))
}
