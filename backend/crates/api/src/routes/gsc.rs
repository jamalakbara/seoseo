use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Redirect,
};
use domain::gsc::{
    self, Callback, ConnectUrl, ConnectionView, InspectInput, InspectionRow, PerformanceInput,
    PerformanceView, Site, SiteInput,
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(gsc_status))
        .routes(routes!(gsc_connect))
        .routes(routes!(gsc_sites))
        .routes(routes!(gsc_set_site))
        .routes(routes!(gsc_performance))
        .routes(routes!(gsc_inspect))
        .routes(routes!(gsc_disconnect))
        .routes(routes!(gsc_callback))
        .routes(routes!(gsc_export))
}

#[utoipa::path(get, path="/projects/{project_id}/gsc", tag="search-console",
    params(("project_id" = Uuid, Path)),
    responses((status=OK, body=ConnectionView)), security(("session"=[]),("bearer"=[])))]
async fn gsc_status(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ConnectionView>> {
    Ok(Json(
        gsc::status(&state.pool, &actor, project_id, &state.cfg).await?,
    ))
}

#[utoipa::path(post, path="/projects/{project_id}/gsc/connect", tag="search-console",
    params(("project_id" = Uuid, Path)),
    responses((status=OK, body=ConnectUrl)), security(("session"=[]),("bearer"=[])))]
async fn gsc_connect(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<ConnectUrl>> {
    Ok(Json(
        gsc::connect(&state.pool, &actor, project_id, &state.cfg).await?,
    ))
}

#[utoipa::path(get, path="/projects/{project_id}/gsc/sites", tag="search-console",
    params(("project_id" = Uuid, Path)),
    responses((status=OK, body=Vec<Site>)), security(("session"=[]),("bearer"=[])))]
async fn gsc_sites(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Site>>> {
    Ok(Json(
        gsc::sites(&state.pool, &actor, project_id, &state.cfg).await?,
    ))
}

#[utoipa::path(put, path="/projects/{project_id}/gsc/site", tag="search-console",
    params(("project_id" = Uuid, Path)), request_body=SiteInput,
    responses((status=OK, body=ConnectionView)), security(("session"=[]),("bearer"=[])))]
async fn gsc_set_site(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<SiteInput>,
) -> ApiResult<Json<ConnectionView>> {
    Ok(Json(
        gsc::set_site(&state.pool, &actor, project_id, &state.cfg, input).await?,
    ))
}

#[utoipa::path(post, path="/projects/{project_id}/gsc/performance", tag="search-console",
    params(("project_id" = Uuid, Path)), request_body=PerformanceInput,
    responses((status=OK, body=PerformanceView)), security(("session"=[]),("bearer"=[])))]
async fn gsc_performance(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<PerformanceInput>,
) -> ApiResult<Json<PerformanceView>> {
    Ok(Json(
        gsc::performance(&state.pool, &actor, project_id, &state.cfg, input).await?,
    ))
}

#[utoipa::path(post, path="/projects/{project_id}/gsc/inspect", tag="search-console",
    params(("project_id" = Uuid, Path)), request_body=InspectInput,
    responses((status=OK, body=Vec<InspectionRow>)), security(("session"=[]),("bearer"=[])))]
async fn gsc_inspect(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<InspectInput>,
) -> ApiResult<Json<Vec<InspectionRow>>> {
    Ok(Json(
        gsc::inspect(&state.pool, &actor, project_id, &state.cfg, input).await?,
    ))
}

#[utoipa::path(delete, path="/projects/{project_id}/gsc", tag="search-console",
    params(("project_id" = Uuid, Path)), responses((status=NO_CONTENT)), security(("session"=[]),("bearer"=[])))]
async fn gsc_disconnect(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    gsc::disconnect(&state.pool, &actor, project_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path="/gsc/oauth/callback", tag="search-console",
    params(Callback), responses((status=SEE_OTHER)), security(("session"=[])))]
async fn gsc_callback(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Query(input): Query<Callback>,
) -> ApiResult<Redirect> {
    let project = gsc::callback(&state.pool, &actor, &state.cfg, input).await?;
    Ok(Redirect::to(&format!("/p/{project}/gsc")))
}

#[utoipa::path(post, path="/projects/{project_id}/gsc/performance.csv", tag="search-console",
    params(("project_id" = Uuid, Path)), request_body=PerformanceInput,
    responses((status=OK, body=String, content_type="text/csv")), security(("session"=[]),("bearer"=[])))]
async fn gsc_export(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<PerformanceInput>,
) -> ApiResult<([(header::HeaderName, &'static str); 2], String)> {
    let data = gsc::performance(&state.pool, &actor, project_id, &state.cfg, input).await?;
    let mut headings = data.dimensions;
    headings.extend(["clicks", "impressions", "ctr", "position"].map(str::to_string));
    let mut csv = headings
        .iter()
        .map(|s| super::audits::csv_cell(s))
        .collect::<Vec<_>>()
        .join(",")
        + "\r\n";
    for row in data.rows {
        let mut cells = row.keys;
        cells.extend([row.clicks, row.impressions, row.ctr, row.position].map(|n| n.to_string()));
        csv.push_str(
            &cells
                .iter()
                .map(|s| super::audits::csv_cell(s))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push_str("\r\n");
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"search-console.csv\"",
            ),
        ],
        csv,
    ))
}
