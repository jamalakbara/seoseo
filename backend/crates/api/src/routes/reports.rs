use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use db::models::{Report, ReportListItem, ReportTemplate};
use domain::reports::{
    SaveReportInput, SaveReportTemplateInput, ShareReportResult, SharedReportView,
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_reports, create_report))
        .routes(routes!(get_report, update_report, delete_report))
        .routes(routes!(get_report_raw))
        .routes(routes!(share_report, unshare_report))
        .routes(routes!(list_report_templates, create_report_template))
        .routes(routes!(update_report_template, delete_report_template))
        .routes(routes!(get_shared_report))
        .routes(routes!(get_shared_report_raw))
}

fn sandboxed_html(html: String) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; sandbox; base-uri 'none'; form-action 'none'; frame-ancestors 'self'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    (headers, html).into_response()
}

#[utoipa::path(get, path = "/projects/{project_id}/reports", tag = "reports",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<ReportListItem>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_reports(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ReportListItem>>> {
    Ok(Json(
        domain::reports::list(&state.pool, &actor, project_id).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/reports", tag = "reports",
    params(("project_id" = Uuid, Path)), request_body = SaveReportInput,
    responses((status = CREATED, body = Report), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn create_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<SaveReportInput>,
) -> ApiResult<(StatusCode, Json<Report>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::reports::create(&state.pool, &actor, project_id, input).await?),
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/reports/{report_id}", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)),
    responses((status = OK, body = Report), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Report>> {
    Ok(Json(
        domain::reports::get(&state.pool, &actor, project_id, report_id).await?,
    ))
}

#[utoipa::path(put, path = "/projects/{project_id}/reports/{report_id}", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)), request_body = SaveReportInput,
    responses((status = OK, body = Report), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SaveReportInput>,
) -> ApiResult<Json<Report>> {
    Ok(Json(
        domain::reports::update(&state.pool, &actor, project_id, report_id, input).await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/reports/{report_id}", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::reports::delete(&state.pool, &actor, project_id, report_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/projects/{project_id}/reports/{report_id}/raw", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)),
    responses((status = OK, content_type = "text/html", body = String)),
    security(("session" = []), ("bearer" = [])))]
async fn get_report_raw(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let report = domain::reports::get(&state.pool, &actor, project_id, report_id).await?;
    Ok(sandboxed_html(report.html))
}

#[utoipa::path(post, path = "/projects/{project_id}/reports/{report_id}/share", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)),
    responses((status = OK, body = ShareReportResult)),
    security(("session" = []), ("bearer" = [])))]
async fn share_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ShareReportResult>> {
    Ok(Json(
        domain::reports::share(&state.pool, &actor, project_id, report_id).await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/reports/{report_id}/share", tag = "reports",
    params(("project_id" = Uuid, Path), ("report_id" = Uuid, Path)),
    responses((status = OK, body = Report)),
    security(("session" = []), ("bearer" = [])))]
async fn unshare_report(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Report>> {
    Ok(Json(
        domain::reports::unshare(&state.pool, &actor, project_id, report_id).await?,
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/report-templates", tag = "reports",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<ReportTemplate>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_report_templates(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ReportTemplate>>> {
    Ok(Json(
        domain::reports::list_templates(&state.pool, &actor, project_id).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/report-templates", tag = "reports",
    params(("project_id" = Uuid, Path)), request_body = SaveReportTemplateInput,
    responses((status = CREATED, body = ReportTemplate)),
    security(("session" = []), ("bearer" = [])))]
async fn create_report_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<SaveReportTemplateInput>,
) -> ApiResult<(StatusCode, Json<ReportTemplate>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::reports::create_template(&state.pool, &actor, project_id, input).await?),
    ))
}

#[utoipa::path(put, path = "/projects/{project_id}/report-templates/{template_id}", tag = "reports",
    params(("project_id" = Uuid, Path), ("template_id" = Uuid, Path)), request_body = SaveReportTemplateInput,
    responses((status = OK, body = ReportTemplate)),
    security(("session" = []), ("bearer" = [])))]
async fn update_report_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, template_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SaveReportTemplateInput>,
) -> ApiResult<Json<ReportTemplate>> {
    Ok(Json(
        domain::reports::update_template(&state.pool, &actor, project_id, template_id, input)
            .await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/report-templates/{template_id}", tag = "reports",
    params(("project_id" = Uuid, Path), ("template_id" = Uuid, Path)), responses((status = NO_CONTENT)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_report_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, template_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::reports::delete_template(&state.pool, &actor, project_id, template_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/shared-reports/{token}", tag = "reports",
    params(("token" = String, Path)), responses((status = OK, body = SharedReportView), (status = NOT_FOUND, body = Problem)))]
async fn get_shared_report(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Json<SharedReportView>> {
    let report = domain::reports::shared(&state.pool, &token).await?;
    Ok(Json(SharedReportView::from(&report)))
}

#[utoipa::path(get, path = "/shared-reports/{token}/raw", tag = "reports",
    params(("token" = String, Path)), responses((status = OK, content_type = "text/html", body = String)))]
async fn get_shared_report_raw(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Response> {
    let report = domain::reports::shared(&state.pool, &token).await?;
    Ok(sandboxed_html(report.html))
}
