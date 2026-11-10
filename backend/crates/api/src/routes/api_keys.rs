use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use db::models::ApiKey;
use domain::api_keys::{CreateApiKeyInput, CreatedApiKey};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_api_keys, create_api_key))
        .routes(routes!(revoke_api_key))
}

#[utoipa::path(get, path = "/api-keys", tag = "api-keys",
    responses((status = OK, body = Vec<ApiKey>)), security(("session" = []), ("bearer" = [])))]
async fn list_api_keys(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<Vec<ApiKey>>> {
    Ok(Json(domain::api_keys::list(&state.pool, &actor).await?))
}

/// Create a key bound to the active workspace. The secret is returned once.
#[utoipa::path(post, path = "/api-keys", tag = "api-keys", request_body = CreateApiKeyInput,
    responses((status = CREATED, body = CreatedApiKey), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn create_api_key(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(input): Json<CreateApiKeyInput>,
) -> ApiResult<(StatusCode, Json<CreatedApiKey>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::api_keys::create(&state.pool, &actor, input).await?),
    ))
}

#[utoipa::path(delete, path = "/api-keys/{id}", tag = "api-keys",
    params(("id" = Uuid, Path)), responses((status = NO_CONTENT), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn revoke_api_key(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    domain::api_keys::revoke(&state.pool, &actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
