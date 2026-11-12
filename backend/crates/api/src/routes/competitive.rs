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
        .routes(routes!(lookup_domain_age))
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
