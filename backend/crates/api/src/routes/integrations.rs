use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
};
use domain::integrations::*;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(indexnow_setup_route))
        .routes(routes!(indexnow_verify_route))
        .routes(routes!(indexnow_submit_route))
        .routes(routes!(link_list_route))
        .routes(routes!(link_save_route))
        .routes(routes!(website_synthesize_route))
}
#[utoipa::path(post,path="/projects/{project_id}/indexnow/setup",tag="integrations",params(("project_id"=Uuid,Path)), responses((status=OK,body=db::integrations::IndexNowSettings)),security(("session"=[]),("bearer"=[])))]
async fn indexnow_setup_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<db::integrations::IndexNowSettings>> {
    Ok(Json(indexnow_setup(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/indexnow/verify",tag="integrations",params(("project_id"=Uuid,Path)), responses((status=OK,body=IndexNowResult)),security(("session"=[]),("bearer"=[])))]
async fn indexnow_verify_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<IndexNowResult>> {
    Ok(Json(indexnow_verify(&s.pool, &a, pid, &s.cfg).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/indexnow/submit",tag="integrations",params(("project_id"=Uuid,Path)), request_body=IndexNowInput,responses((status=OK,body=Vec<IndexNowResult>)),security(("session"=[]),("bearer"=[])))]
async fn indexnow_submit_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<IndexNowInput>,
) -> ApiResult<Json<Vec<IndexNowResult>>> {
    Ok(Json(
        indexnow_submit(&s.pool, &a, pid, &s.cfg, input).await?,
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/links",tag="integrations",params(("project_id"=Uuid,Path)), responses((status=OK,body=Vec<db::integrations::LinkObservation>)),security(("session"=[]),("bearer"=[])))]
async fn link_list_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<db::integrations::LinkObservation>>> {
    Ok(Json(links(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/links",tag="integrations",params(("project_id"=Uuid,Path)), request_body=LinkInput,responses((status=OK,body=db::integrations::LinkObservation)),security(("session"=[]),("bearer"=[])))]
async fn link_save_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<LinkInput>,
) -> ApiResult<Json<db::integrations::LinkObservation>> {
    Ok(Json(save_link(&s.pool, &a, pid, input).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/website-synthesis",tag="integrations",params(("project_id"=Uuid,Path)), responses((status=OK,body=WebsiteSynthesis)),security(("session"=[]),("bearer"=[])))]
async fn website_synthesize_route(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<WebsiteSynthesis>> {
    Ok(Json(synthesize(&s.pool, &a, pid, &s.cfg).await?))
}
