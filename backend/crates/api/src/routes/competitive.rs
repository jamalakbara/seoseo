use crate::{
    auth::Auth,
    error::{ApiResult, Problem},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use domain::competitive::{DomainOverview, DomainQuery};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_domain_overview))
        .routes(routes!(get_cannibalization_flags))
        .routes(routes!(detect_technologies))
        .routes(routes!(lookup_domain_age))
        .routes(routes!(import_competitive_volumes))
}

#[utoipa::path(post,path="/projects/{project_id}/technology",tag="competitive",params(("project_id"=Uuid,Path)),request_body=domain::technology::TechnologyInput,
    responses((status=OK,body=domain::technology::TechnologyResult),(status=NOT_FOUND,body=Problem),(status=CONFLICT,body=Problem),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn detect_technologies(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::technology::TechnologyInput>,
) -> ApiResult<Json<domain::technology::TechnologyResult>> {
    Ok(Json(
        domain::technology::inspect(
            &s.pool,
            &a,
            pid,
            crawler::UrlPolicy {
                allow_private: s.cfg.crawler_allow_private,
            },
            input,
        )
        .await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/cannibalization",tag="competitive",params(("project_id"=Uuid,Path),domain::cannibalization::CannibalizationQuery),
    responses((status=OK,body=domain::cannibalization::CannibalizationFlags),(status=NOT_FOUND,body=Problem),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn get_cannibalization_flags(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Query(q): Query<domain::cannibalization::CannibalizationQuery>,
) -> ApiResult<Json<domain::cannibalization::CannibalizationFlags>> {
    Ok(Json(
        domain::cannibalization::flags(&s.pool, &a, pid, q).await?,
    ))
}
#[utoipa::path(post,path="/projects/{project_id}/competitive-volumes",tag="competitive",params(("project_id"=Uuid,Path)),request_body=domain::competitive::CompetitiveVolumeInput,responses((status=OK,body=serde_json::Value)),security(("session"=[]),("bearer"=[])))]
async fn import_competitive_volumes(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::competitive::CompetitiveVolumeInput>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(
        domain::competitive::volumes(&s.pool, &a, pid, input).await?,
    ))
}

#[utoipa::path(post, path="/projects/{project_id}/domain-age", tag="competitive",
    params(("project_id"=Uuid,Path)),request_body=domain::domain_age::DomainAgeInput,
    responses((status=OK,body=domain::domain_age::DomainAge), (status=NOT_FOUND,body=Problem),
        (status=UNPROCESSABLE_ENTITY,body=Problem), (status=SERVICE_UNAVAILABLE,body=Problem)),
    security(("session"=[]),("bearer"=[])))]
async fn lookup_domain_age(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::domain_age::DomainAgeInput>,
) -> ApiResult<Json<domain::domain_age::DomainAge>> {
    Ok(Json(
        domain::domain_age::lookup(&s.pool, &a, pid, input).await?,
    ))
}

#[utoipa::path(get, path="/projects/{project_id}/domain-overview", tag="competitive",
    params(("project_id"=Uuid,Path),DomainQuery),
    responses((status=OK,body=DomainOverview)), security(("session"=[]),("bearer"=[])))]
async fn get_domain_overview(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Query(q): Query<DomainQuery>,
) -> ApiResult<Json<DomainOverview>> {
    Ok(Json(
        domain::competitive::overview(&s.pool, &a, pid, q).await?,
    ))
}
