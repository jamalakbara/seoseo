use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(start_local_scan, list_local_scans))
        .routes(routes!(import_local_scan))
        .routes(routes!(get_local_scan, delete_local_scan))
        .routes(routes!(capture_google_overview, list_google_overviews))
        .routes(routes!(import_google_overview))
        .routes(routes!(import_city_demand, list_city_demand))
}

#[utoipa::path(post,path="/projects/{project_id}/local-trackers/{tracker_id}/scans",tag="local-seo",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),request_body=domain::local_scans::StartLocalScanInput,responses((status=OK,body=db::local_scans::LocalScanRun)),security(("session"=[]),("bearer"=[])))]
async fn start_local_scan(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(input): Json<domain::local_scans::StartLocalScanInput>,
) -> ApiResult<Json<db::local_scans::LocalScanRun>> {
    Ok(Json(
        domain::local_scans::start(&s.pool, &a, pid, tid, &s.cfg, input).await?,
    ))
}

#[utoipa::path(post,path="/projects/{project_id}/local-trackers/{tracker_id}/scans/import",tag="local-seo",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),request_body=domain::local_scans::ImportLocalScanInput,responses((status=OK,body=db::local_scans::LocalScanRun)),security(("session"=[]),("bearer"=[])))]
async fn import_local_scan(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(input): Json<domain::local_scans::ImportLocalScanInput>,
) -> ApiResult<Json<db::local_scans::LocalScanRun>> {
    Ok(Json(
        domain::local_scans::import(&s.pool, &a, pid, tid, input).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/local-trackers/{tracker_id}/scans",tag="local-seo",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),responses((status=OK,body=Vec<db::local_scans::LocalScanRun>)),security(("session"=[]),("bearer"=[])))]
async fn list_local_scans(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<db::local_scans::LocalScanRun>>> {
    Ok(Json(
        domain::local_scans::list(&s.pool, &a, pid, tid).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/local-trackers/{tracker_id}/scans/{run_id}",tag="local-seo",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path),("run_id"=Uuid,Path)),responses((status=OK,body=domain::local_scans::LocalScanView)),security(("session"=[]),("bearer"=[])))]
async fn get_local_scan(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<domain::local_scans::LocalScanView>> {
    Ok(Json(
        domain::local_scans::get(&s.pool, &a, pid, tid, rid).await?,
    ))
}

#[utoipa::path(delete,path="/projects/{project_id}/local-trackers/{tracker_id}/scans/{run_id}",tag="local-seo",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path),("run_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_local_scan(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::local_scans::delete(&s.pool, &a, pid, tid, rid).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post,path="/projects/{project_id}/ai-overviews",tag="ai-visibility",params(("project_id"=Uuid,Path)),request_body=domain::observations::CaptureOverviewInput,responses((status=OK,body=domain::observations::OverviewCapture)),security(("session"=[]),("bearer"=[])))]
async fn capture_google_overview(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::observations::CaptureOverviewInput>,
) -> ApiResult<Json<domain::observations::OverviewCapture>> {
    Ok(Json(
        domain::observations::capture_overview(&s.pool, &a, pid, &s.cfg, input).await?,
    ))
}

#[utoipa::path(post,path="/projects/{project_id}/ai-overviews/import",tag="ai-visibility",params(("project_id"=Uuid,Path)),request_body=domain::observations::ImportOverviewInput,responses((status=OK,body=db::observations::ResearchObservation)),security(("session"=[]),("bearer"=[])))]
async fn import_google_overview(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::observations::ImportOverviewInput>,
) -> ApiResult<Json<db::observations::ResearchObservation>> {
    Ok(Json(
        domain::observations::import_overview(&s.pool, &a, pid, input).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/ai-overviews",tag="ai-visibility",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<db::observations::ResearchObservation>)),security(("session"=[]),("bearer"=[])))]
async fn list_google_overviews(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<db::observations::ResearchObservation>>> {
    Ok(Json(
        domain::observations::overview_history(&s.pool, &a, pid).await?,
    ))
}

#[utoipa::path(post,path="/projects/{project_id}/city-demand",tag="local-seo",params(("project_id"=Uuid,Path)),request_body=domain::observations::ImportCityDemandInput,responses((status=OK,body=db::observations::ResearchObservation)),security(("session"=[]),("bearer"=[])))]
async fn import_city_demand(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::observations::ImportCityDemandInput>,
) -> ApiResult<Json<db::observations::ResearchObservation>> {
    Ok(Json(
        domain::observations::import_city(&s.pool, &a, pid, input).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/city-demand",tag="local-seo",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<db::observations::ResearchObservation>)),security(("session"=[]),("bearer"=[])))]
async fn list_city_demand(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<db::observations::ResearchObservation>>> {
    Ok(Json(
        domain::observations::city_history(&s.pool, &a, pid).await?,
    ))
}
