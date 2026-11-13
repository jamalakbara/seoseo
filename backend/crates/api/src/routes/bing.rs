use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use domain::bing::*;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(bing_status))
        .routes(routes!(bing_connect))
        .routes(routes!(bing_sites))
        .routes(routes!(bing_set_site))
        .routes(routes!(bing_report))
        .routes(routes!(bing_disconnect))
}

#[utoipa::path(get,path="/projects/{project_id}/bing",tag="bing-webmaster",params(("project_id"=Uuid,Path)),responses((status=OK,body=BingConnectionView)),security(("session"=[]),("bearer"=[])))]
async fn bing_status(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<BingConnectionView>> {
    Ok(Json(status(&s.pool, &a, pid, &s.cfg).await?))
}

#[utoipa::path(put,path="/projects/{project_id}/bing",tag="bing-webmaster",params(("project_id"=Uuid,Path)),request_body=BingConnectInput,responses((status=OK,body=BingConnectionView)),security(("session"=[]),("bearer"=[])))]
async fn bing_connect(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<BingConnectInput>,
) -> ApiResult<Json<BingConnectionView>> {
    Ok(Json(connect(&s.pool, &a, pid, &s.cfg, input).await?))
}

#[utoipa::path(get,path="/projects/{project_id}/bing/sites",tag="bing-webmaster",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<BingSiteView>)),security(("session"=[]),("bearer"=[])))]
async fn bing_sites(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<BingSiteView>>> {
    Ok(Json(sites(&s.pool, &a, pid, &s.cfg).await?))
}

#[utoipa::path(put,path="/projects/{project_id}/bing/site",tag="bing-webmaster",params(("project_id"=Uuid,Path)),request_body=BingSiteInput,responses((status=OK,body=BingConnectionView)),security(("session"=[]),("bearer"=[])))]
async fn bing_set_site(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<BingSiteInput>,
) -> ApiResult<Json<BingConnectionView>> {
    Ok(Json(set_site(&s.pool, &a, pid, &s.cfg, input).await?))
}

#[utoipa::path(post,path="/projects/{project_id}/bing/reports/{kind}",tag="bing-webmaster",params(("project_id"=Uuid,Path),("kind"=BingReportKind,Path)),request_body=BingReportInput,responses((status=OK,body=BingReportView)),security(("session"=[]),("bearer"=[])))]
async fn bing_report(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, kind)): Path<(Uuid, BingReportKind)>,
    Json(input): Json<BingReportInput>,
) -> ApiResult<Json<BingReportView>> {
    Ok(Json(report(&s.pool, &a, pid, &s.cfg, kind, input).await?))
}

#[utoipa::path(delete,path="/projects/{project_id}/bing",tag="bing-webmaster",params(("project_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn bing_disconnect(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<StatusCode> {
    disconnect(&s.pool, &a, pid).await?;
    Ok(StatusCode::NO_CONTENT)
}
