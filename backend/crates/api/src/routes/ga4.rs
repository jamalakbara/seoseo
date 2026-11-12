use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Redirect,
};
use domain::ga4::*;
use serde::Deserialize;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
#[derive(Deserialize)]
struct Page {
    page_token: Option<String>,
}
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ga4_status))
        .routes(routes!(ga4_connect))
        .routes(routes!(ga4_properties))
        .routes(routes!(ga4_set_property))
        .routes(routes!(ga4_report))
        .routes(routes!(ga4_disconnect))
        .routes(routes!(ga4_callback))
}
#[utoipa::path(get,path="/projects/{project_id}/ga4",tag="analytics",params(("project_id"=Uuid,Path)),responses((status=OK,body=Ga4ConnectionView)),security(("session"=[]),("bearer"=[])))]
async fn ga4_status(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Ga4ConnectionView>> {
    Ok(Json(status(&s.pool, &a, pid, &s.cfg).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/ga4/connect",tag="analytics",params(("project_id"=Uuid,Path)),responses((status=OK,body=Ga4ConnectUrl)),security(("session"=[]),("bearer"=[])))]
async fn ga4_connect(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Ga4ConnectUrl>> {
    Ok(Json(connect(&s.pool, &a, pid, &s.cfg).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/ga4/properties",tag="analytics",params(("project_id"=Uuid,Path),("page_token"=Option<String>,Query)),responses((status=OK,body=Ga4Properties)),security(("session"=[]),("bearer"=[])))]
async fn ga4_properties(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Query(q): Query<Page>,
) -> ApiResult<Json<Ga4Properties>> {
    Ok(Json(
        properties(&s.pool, &a, pid, &s.cfg, q.page_token).await?,
    ))
}
#[utoipa::path(put,path="/projects/{project_id}/ga4/property",tag="analytics",params(("project_id"=Uuid,Path)),request_body=Ga4PropertyInput,responses((status=OK,body=Ga4ConnectionView)),security(("session"=[]),("bearer"=[])))]
async fn ga4_set_property(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<Ga4PropertyInput>,
) -> ApiResult<Json<Ga4ConnectionView>> {
    Ok(Json(set_property(&s.pool, &a, pid, &s.cfg, input).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/ga4/reports/{kind}",tag="analytics",params(("project_id"=Uuid,Path), ("kind"=Ga4ReportKind,Path)),request_body=Ga4ReportInput,responses((status=OK,body=Ga4ReportView)),security(("session"=[]),("bearer"=[])))]
async fn ga4_report(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, kind)): Path<(Uuid, Ga4ReportKind)>,
    Json(input): Json<Ga4ReportInput>,
) -> ApiResult<Json<Ga4ReportView>> {
    Ok(Json(report(&s.pool, &a, pid, &s.cfg, kind, input).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/ga4",tag="analytics",params(("project_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[])))]
async fn ga4_disconnect(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<StatusCode> {
    disconnect(&s.pool, &a, pid).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/ga4/oauth/callback",tag="analytics",params(Ga4Callback),responses((status=SEE_OTHER)),security(("session"=[])))]
async fn ga4_callback(
    State(s): State<AppState>,
    Auth(a): Auth,
    Query(q): Query<Ga4Callback>,
) -> ApiResult<Redirect> {
    let pid = callback(&s.pool, &a, &s.cfg, q).await?;
    Ok(Redirect::to(&format!(
        "{}/p/{pid}/integrations",
        s.cfg.web_base_url.trim_end_matches('/')
    )))
}
