use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use domain::memory::{ProjectContext, UpdateProjectContextInput};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_project_context, update_project_context))
}

#[utoipa::path(get, path = "/projects/{project_id}/context", tag = "project-memory",
    params(("project_id" = Uuid, Path)),
    responses((status = OK, body = ProjectContext), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_project_context(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ProjectContext>> {
    Ok(Json(
        domain::memory::get(&state.pool, &actor, project_id).await?,
    ))
}

#[utoipa::path(put, path = "/projects/{project_id}/context", tag = "project-memory",
    params(("project_id" = Uuid, Path)), request_body = UpdateProjectContextInput,
    responses((status = OK, body = ProjectContext), (status = CONFLICT, body = Problem),
              (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_project_context(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<UpdateProjectContextInput>,
) -> ApiResult<Json<ProjectContext>> {
    Ok(Json(
        domain::memory::update(&state.pool, &actor, project_id, input).await?,
    ))
}
