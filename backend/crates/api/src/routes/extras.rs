use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use domain::{cpc, extras as e, public_tools as p, telemetry as t};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(run_public_tool))
        .routes(routes!(list_log_analyses, create_log_analysis))
        .routes(routes!(delete_log_analysis))
        .routes(routes!(get_volatility))
        .routes(routes!(
            get_crawler_credential,
            save_crawler_credential,
            delete_crawler_credential
        ))
        .routes(routes!(get_cpc_metrics))
        .routes(routes!(get_telemetry, save_telemetry))
        .routes(routes!(send_telemetry))
}
#[utoipa::path(post,path="/public/tools",tag="public-tools",request_body=p::PublicToolInput,responses((status=OK,body=serde_json::Value)))]
async fn run_public_tool(
    State(s): State<AppState>,
    Json(v): Json<p::PublicToolInput>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(p::run(&s.pool, v).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/log-analyses",params(("project_id"=Uuid,Path)),tag="extras",responses((status=OK,body=Vec<db::extras::LogAnalysis>)),security(("session"=[]),("bearer"=[])))]
async fn list_log_analyses(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<db::extras::LogAnalysis>>> {
    Ok(Json(e::logs(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/log-analyses",params(("project_id"=Uuid,Path)),tag="extras",request_body=e::LogInput,responses((status=OK,body=db::extras::LogAnalysis)),security(("session"=[]),("bearer"=[])))]
async fn create_log_analysis(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<e::LogInput>,
) -> ApiResult<Json<db::extras::LogAnalysis>> {
    Ok(Json(e::save_log(&s.pool, &a, pid, v).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/log-analyses/{analysis_id}",params(("project_id"=Uuid,Path),("analysis_id"=Uuid,Path)),tag="extras",responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_log_analysis(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    e::delete_log(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/projects/{project_id}/volatility",params(("project_id"=Uuid,Path)),tag="extras",responses((status=OK,body=e::Volatility)),security(("session"=[]),("bearer"=[])))]
async fn get_volatility(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<e::Volatility>> {
    Ok(Json(e::get_volatility(&s.pool, &a, pid).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/crawler-credential",params(("project_id"=Uuid,Path)),tag="extras",responses((status=OK,body=e::CredentialView)),security(("session"=[]),("bearer"=[])))]
async fn get_crawler_credential(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<e::CredentialView>> {
    Ok(Json(e::credential_status(&s.pool, &a, pid).await?))
}
#[utoipa::path(put,path="/projects/{project_id}/crawler-credential",params(("project_id"=Uuid,Path)),tag="extras",request_body=e::CrawlerCredentialInput,responses((status=OK,body=e::CredentialView)),security(("session"=[]),("bearer"=[])))]
async fn save_crawler_credential(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<e::CrawlerCredentialInput>,
) -> ApiResult<Json<e::CredentialView>> {
    Ok(Json(e::save_credential(&s.pool, &a, pid, &s.cfg, v).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/crawler-credential",params(("project_id"=Uuid,Path)),tag="extras",responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_crawler_credential(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<StatusCode> {
    e::delete_credential(&s.pool, &a, pid).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/projects/{project_id}/keyword-cpc",params(("project_id"=Uuid,Path)),tag="keywords",request_body=cpc::CpcInput,responses((status=OK,body=cpc::CpcView)),security(("session"=[]),("bearer"=[])))]
async fn get_cpc_metrics(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<cpc::CpcInput>,
) -> ApiResult<Json<cpc::CpcView>> {
    Ok(Json(cpc::metrics(&s.pool, &a, pid, &s.cfg, v).await?))
}
#[utoipa::path(get,path="/settings/telemetry",tag="system",responses((status=OK,body=t::TelemetryView)),security(("session"=[])))]
async fn get_telemetry(
    State(s): State<AppState>,
    Auth(a): Auth,
) -> ApiResult<Json<t::TelemetryView>> {
    Ok(Json(t::status(&s.pool, &a, &s.cfg).await?))
}
#[utoipa::path(put,path="/settings/telemetry",tag="system",request_body=t::TelemetryInput,responses((status=OK,body=t::TelemetryView)),security(("session"=[])))]
async fn save_telemetry(
    State(s): State<AppState>,
    Auth(a): Auth,
    Json(v): Json<t::TelemetryInput>,
) -> ApiResult<Json<t::TelemetryView>> {
    Ok(Json(t::save(&s.pool, &a, &s.cfg, v).await?))
}
#[utoipa::path(post,path="/settings/telemetry/send",tag="system",responses((status=OK,body=t::TelemetryView)),security(("session"=[])))]
async fn send_telemetry(
    State(s): State<AppState>,
    Auth(a): Auth,
) -> ApiResult<Json<t::TelemetryView>> {
    Ok(Json(t::send(&s.pool, &a, &s.cfg).await?))
}
