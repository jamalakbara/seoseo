use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use db::models::Project;
use domain::projects::{CreateProjectInput, UpdateProjectInput};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_projects, create_project))
        .routes(routes!(get_project, update_project, delete_project))
        .routes(routes!(archive_project))
        .routes(routes!(restore_project))
}

#[derive(Deserialize, IntoParams)]
struct ListQuery {
    /// List archived projects instead of active ones.
    #[serde(default)]
    archived: bool,
}

#[utoipa::path(get, path = "/projects", tag = "projects", params(ListQuery),
    responses((status = OK, body = Vec<Project>)), security(("session" = []), ("bearer" = [])))]
async fn list_projects(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(
        domain::projects::list(&state.pool, &actor, q.archived).await?,
    ))
}

#[utoipa::path(post, path = "/projects", tag = "projects", request_body = CreateProjectInput,
    responses((status = CREATED, body = Project), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn create_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(input): Json<CreateProjectInput>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::projects::create(&state.pool, &actor, input).await?),
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}", tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses((status = OK, body = Project), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        domain::projects::get(&state.pool, &actor, project_id).await?,
    ))
}

#[utoipa::path(patch, path = "/projects/{project_id}", tag = "projects",
    params(("project_id" = Uuid, Path)), request_body = UpdateProjectInput,
    responses((status = OK, body = Project), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<UpdateProjectInput>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        domain::projects::update(&state.pool, &actor, project_id, input).await?,
    ))
}

/// Permanently delete an archived project and all its data.
#[utoipa::path(delete, path = "/projects/{project_id}", tag = "projects",
    params(("project_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    domain::projects::delete(&state.pool, &actor, project_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/projects/{project_id}/archive", tag = "projects",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Project)),
    security(("session" = []), ("bearer" = [])))]
async fn archive_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        domain::projects::set_archived(&state.pool, &actor, project_id, true).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/restore", tag = "projects",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Project)),
    security(("session" = []), ("bearer" = [])))]
async fn restore_project(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        domain::projects::set_archived(&state.pool, &actor, project_id, false).await?,
    ))
}
