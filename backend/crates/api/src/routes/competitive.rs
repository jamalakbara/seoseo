use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use domain::competitive::{DomainOverview, DomainQuery};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_domain_overview))
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
