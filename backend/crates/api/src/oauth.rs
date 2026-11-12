use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Form, Json, Router,
    extract::{Path, Query, RawQuery, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::oauth as service;
use serde::Deserialize;
use serde_json::json;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn public() -> Router<AppState> {
    Router::new()
        .route("/.well-known/oauth-authorization-server", get(metadata))
        .route(
            "/.well-known/oauth-protected-resource",
            get(resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(resource_metadata),
        )
        .route("/oauth/register", post(register))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .route("/oauth/revoke", post(revoke))
}
pub fn documented() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(oauth_preview))
        .routes(routes!(oauth_consent))
        .routes(routes!(oauth_grants))
        .routes(routes!(oauth_revoke_grant))
}
async fn metadata(State(s): State<AppState>) -> Json<serde_json::Value> {
    let issuer = service::issuer(&s.cfg);
    Json(
        json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/oauth/authorize"),"token_endpoint":format!("{issuer}/oauth/token"),"registration_endpoint":format!("{issuer}/oauth/register"),"revocation_endpoint":format!("{issuer}/oauth/revoke"),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"token_endpoint_auth_methods_supported":["none"],"code_challenge_methods_supported":["S256"],"scopes_supported":["mcp"]}),
    )
}
async fn resource_metadata(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(
        json!({"resource":service::resource(&s.cfg),"authorization_servers":[service::issuer(&s.cfg)],"scopes_supported":["mcp"],"bearer_methods_supported":["header"]}),
    )
}
fn result<T: serde::Serialize>(
    r: common::AppResult<T>,
    success: StatusCode,
    err: &str,
) -> Response {
    let mut response = match r {
        Ok(v) => (success, Json(v)).into_response(),
        Err(e) => {
            if let common::AppError::Internal(ref detail) = e {
                tracing::error!(error=?detail,"OAuth operation failed");
            }
            (StatusCode::BAD_REQUEST,Json(json!({"error":err,"error_description":"OAuth request rejected; check registration, consent, resource, proof and expiry"}))).into_response()
        }
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response
}
async fn register(State(s): State<AppState>, Json(q): Json<service::Registration>) -> Response {
    result(
        service::register(&s.pool, q).await,
        StatusCode::CREATED,
        "invalid_client_metadata",
    )
}
async fn authorize(
    State(s): State<AppState>,
    RawQuery(raw): RawQuery,
    Query(q): Query<service::Authorization>,
) -> Response {
    if let Err(e) = service::preview(&s.pool, &s.cfg, &q).await {
        return result::<()>(Err(e), StatusCode::OK, "invalid_request");
    }
    let raw = raw.unwrap_or_default();
    if raw.len() > 8192 {
        return result::<()>(
            Err(common::AppError::Unauthorized),
            StatusCode::OK,
            "invalid_request",
        );
    }
    Redirect::to(&format!("{}/oauth/consent?{raw}", service::issuer(&s.cfg))).into_response()
}
async fn token(State(s): State<AppState>, Form(q): Form<service::TokenInput>) -> Response {
    if !matches!(
        q.grant_type.as_str(),
        "authorization_code" | "refresh_token"
    ) {
        return result::<()>(
            Err(common::AppError::Unauthorized),
            StatusCode::OK,
            "unsupported_grant_type",
        );
    }

    result(
        service::exchange(&s.pool, &s.cfg, q).await,
        StatusCode::OK,
        "invalid_grant",
    )
}
#[derive(Deserialize)]
struct Revocation {
    token: String,
    client_id: String,
}
async fn revoke(State(s): State<AppState>, Form(q): Form<Revocation>) -> Response {
    result(
        db::oauth::revoke_token(&s.pool, &common::token::sha256_hex(&q.token), &q.client_id)
            .await
            .map(|_| json!({})),
        StatusCode::OK,
        "invalid_request",
    )
}
#[utoipa::path(post,path="/oauth/preview",tag="oauth",request_body=service::Authorization,responses((status=OK,body=service::ConsentView)),security(("session"=[])))]
async fn oauth_preview(
    State(s): State<AppState>,
    Auth(_a): Auth,
    Json(q): Json<service::Authorization>,
) -> ApiResult<Json<service::ConsentView>> {
    Ok(Json(service::preview(&s.pool, &s.cfg, &q).await?))
}
#[utoipa::path(post,path="/oauth/consent",tag="oauth",request_body=service::ConsentInput,responses((status=OK,body=service::ConsentResult)),security(("session"=[])))]
async fn oauth_consent(
    State(s): State<AppState>,
    Auth(a): Auth,
    Json(q): Json<service::ConsentInput>,
) -> ApiResult<Json<service::ConsentResult>> {
    Ok(Json(service::consent(&s.pool, &a, &s.cfg, q).await?))
}
#[utoipa::path(get,path="/oauth/grants",tag="oauth",responses((status=OK,body=Vec<db::oauth::GrantView>)),security(("session"=[])))]
async fn oauth_grants(
    State(s): State<AppState>,
    Auth(a): Auth,
) -> ApiResult<Json<Vec<db::oauth::GrantView>>> {
    Ok(Json(db::oauth::grants(&s.pool, a.user_id, a.org_id).await?))
}
#[utoipa::path(delete,path="/oauth/grants/{id}",tag="oauth",params(("id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[])))]
async fn oauth_revoke_grant(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    db::oauth::revoke_grant(&s.pool, a.user_id, a.org_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
