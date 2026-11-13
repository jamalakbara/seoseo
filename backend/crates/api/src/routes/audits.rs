use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use crawler::rules::IssueType;
use db::audits::{Audit, IssueRow, PageRow};
use domain::audits::{IssueQuery, IssueSummary, PageQuery, PageView, Paged, StartAuditInput};
use futures::Stream;
use std::convert::Infallible;
use std::time::Duration;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_audits, start_audit))
        .routes(routes!(get_audit, delete_audit))
        .routes(routes!(cancel_audit))
        .routes(routes!(audit_events))
        .routes(routes!(audit_issue_summary))
        .routes(routes!(list_audit_issues))
        .routes(routes!(export_audit_issues))
        .routes(routes!(list_audit_pages))
        .routes(routes!(get_audit_page))
        .routes(routes!(list_issue_types))
        .routes(routes!(get_internal_link_graph))
        .routes(routes!(compare_site_audits))
        .routes(routes!(get_audit_schedule, set_audit_schedule))
}

#[utoipa::path(get,path="/projects/{project_id}/audits/{audit_id}/comparison",tag="audits",
    params(("project_id"=Uuid,Path),("audit_id"=Uuid,Path),domain::audit_compare::CompareAuditQuery),
    responses((status=OK,body=domain::audit_compare::AuditComparison),(status=NOT_FOUND,body=Problem),(status=CONFLICT,body=Problem),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn compare_site_audits(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((pid, aid)): Path<(Uuid, Uuid)>,
    Query(q): Query<domain::audit_compare::CompareAuditQuery>,
) -> ApiResult<Json<domain::audit_compare::AuditComparison>> {
    Ok(Json(
        domain::audit_compare::compare(&state.pool, &actor, pid, aid, q).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/audit-schedule",tag="audits",params(("project_id"=Uuid,Path)),
    responses((status=OK,body=Option<db::audit_schedule::AuditSchedule>)),security(("session"=[]),("bearer"=[])))]
async fn get_audit_schedule(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Option<db::audit_schedule::AuditSchedule>>> {
    Ok(Json(
        domain::audit_schedule::get(&state.pool, &actor, pid).await?,
    ))
}

#[utoipa::path(put,path="/projects/{project_id}/audit-schedule",tag="audits",params(("project_id"=Uuid,Path)),request_body=domain::audit_schedule::SetAuditScheduleInput,
    responses((status=OK,body=db::audit_schedule::AuditSchedule),(status=CONFLICT,body=Problem),(status=FORBIDDEN,body=Problem),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn set_audit_schedule(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<domain::audit_schedule::SetAuditScheduleInput>,
) -> ApiResult<Json<db::audit_schedule::AuditSchedule>> {
    Ok(Json(
        domain::audit_schedule::set(
            &state.pool,
            &actor,
            pid,
            policy(&state),
            state
                .cfg
                .renderer_url
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty()),
            input,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/internal-links", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path), domain::internal_links::InternalLinkQuery),
    responses((status = OK, body = domain::internal_links::InternalLinkGraph), (status = NOT_FOUND, body = Problem),
              (status = CONFLICT, body = Problem), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_internal_link_graph(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<domain::internal_links::InternalLinkQuery>,
) -> ApiResult<Json<domain::internal_links::InternalLinkGraph>> {
    Ok(Json(
        domain::internal_links::graph(&state.pool, &actor, project_id, audit_id, query).await?,
    ))
}

fn policy(state: &AppState) -> crawler::UrlPolicy {
    crawler::UrlPolicy {
        allow_private: state.cfg.crawler_allow_private,
    }
}

/// The issue catalog: ids, severities, explanations and fixes.
#[utoipa::path(get, path = "/audit-issue-types", tag = "audits",
    responses((status = OK, body = Vec<IssueType>)))]
async fn list_issue_types() -> Json<&'static [IssueType]> {
    Json(domain::audits::issue_types())
}

#[utoipa::path(get, path = "/projects/{project_id}/audits", tag = "audits",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<Audit>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_audits(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Audit>>> {
    Ok(Json(
        domain::audits::list(&state.pool, &actor, project_id).await?,
    ))
}

/// Start a crawl of the project's website. Runs in the background; follow progress via
/// `GET …/audits/{audit_id}` or the `…/events` stream.
#[utoipa::path(post, path = "/projects/{project_id}/audits", tag = "audits",
    params(("project_id" = Uuid, Path)), request_body = StartAuditInput,
    responses((status = ACCEPTED, body = Audit), (status = CONFLICT, body = Problem), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn start_audit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<StartAuditInput>,
) -> ApiResult<(StatusCode, Json<Audit>)> {
    let audit = domain::audits::start(
        &state.pool,
        &actor,
        project_id,
        policy(&state),
        state
            .cfg
            .renderer_url
            .as_ref()
            .is_some_and(|s| !s.is_empty()),
        input,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(audit)))
}

#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = OK, body = Audit), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_audit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Audit>> {
    Ok(Json(
        domain::audits::get(&state.pool, &actor, project_id, audit_id).await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/audits/{audit_id}", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_audit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::audits::delete(&state.pool, &actor, project_id, audit_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/projects/{project_id}/audits/{audit_id}/cancel", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = OK, body = Audit), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn cancel_audit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Audit>> {
    Ok(Json(
        domain::audits::cancel(&state.pool, &actor, project_id, audit_id).await?,
    ))
}

/// Server-sent events: an `audit` event with the audit JSON every second while it runs,
/// then one final event and the stream closes.
#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/events", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = OK, content_type = "text/event-stream", body = String)),
    security(("session" = []), ("bearer" = [])))]
async fn audit_events(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Authorize once up front; the stream then reads by project-scoped id.
    domain::audits::get(&state.pool, &actor, project_id, audit_id).await?;
    let pool = state.pool.clone();
    let stream = async_stream::stream! {
        let mut last = String::new();
        // Cap the stream at ~3h; clients reconnect if they need more.
        for _ in 0..10_800 {
            let audit = match db::audits::get(&pool, project_id, audit_id).await {
                Ok(a) => a,
                Err(_) => break,
            };
            let done = !matches!(audit.status.as_str(), "queued" | "running");
            let body = serde_json::to_string(&audit).unwrap_or_default();
            if body != last {
                yield Ok(Event::default().event("audit").data(body.clone()));
                last = body;
            }
            if done {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Issue types found, with the number of affected pages.
#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/issues/summary", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = OK, body = Vec<IssueSummary>)), security(("session" = []), ("bearer" = [])))]
async fn audit_issue_summary(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<IssueSummary>>> {
    Ok(Json(
        domain::audits::issue_summary(&state.pool, &actor, project_id, audit_id).await?,
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/issues", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path), IssueQuery),
    responses((status = OK, body = Paged<IssueRow>)), security(("session" = []), ("bearer" = [])))]
async fn list_audit_issues(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<IssueQuery>,
) -> ApiResult<Json<Paged<IssueRow>>> {
    Ok(Json(
        domain::audits::issues(&state.pool, &actor, project_id, audit_id, q).await?,
    ))
}

/// All issues as CSV (url, issue, severity, title, details).
#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/issues.csv", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path)),
    responses((status = OK, content_type = "text/csv", body = String)),
    security(("session" = []), ("bearer" = [])))]
async fn export_audit_issues(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let mut offset = 0;
    let mut csv = String::from("url,issue,severity,title,details\n");
    loop {
        let page = domain::audits::issues(
            &state.pool,
            &actor,
            project_id,
            audit_id,
            IssueQuery {
                issue_type: None,
                limit: Some(1000),
                offset: Some(offset),
            },
        )
        .await?;
        for i in &page.items {
            let title = crawler::rules::issue_type(&i.issue_type).map_or("", |t| t.title);
            csv.push_str(
                &[
                    csv_cell(i.page_url.as_deref().unwrap_or("")),
                    csv_cell(&i.issue_type),
                    csv_cell(&i.severity),
                    csv_cell(title),
                    csv_cell(&i.details.to_string()),
                ]
                .join(","),
            );
            csv.push('\n');
        }
        offset += page.items.len() as i64;
        if page.items.is_empty() || offset >= page.total {
            break;
        }
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"audit-{audit_id}-issues.csv\""),
            ),
        ],
        csv,
    )
        .into_response())
}

/// Quote a CSV cell and neutralize spreadsheet formula injection.
pub(crate) fn csv_cell(v: &str) -> String {
    let v = if v.starts_with(['\t', '\r', '\n']) || v.trim_start().starts_with(['=', '+', '-', '@'])
    {
        format!("'{v}")
    } else {
        v.to_string()
    };
    format!("\"{}\"", v.replace('"', "\"\""))
}

#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/pages", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path), PageQuery),
    responses((status = OK, body = Paged<PageRow>)), security(("session" = []), ("bearer" = [])))]
async fn list_audit_pages(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<PageQuery>,
) -> ApiResult<Json<Paged<PageRow>>> {
    Ok(Json(
        domain::audits::pages(&state.pool, &actor, project_id, audit_id, q).await?,
    ))
}

/// One crawled page with its issues, outgoing links and inlinks.
#[utoipa::path(get, path = "/projects/{project_id}/audits/{audit_id}/pages/{page_id}", tag = "audits",
    params(("project_id" = Uuid, Path), ("audit_id" = Uuid, Path), ("page_id" = Uuid, Path)),
    responses((status = OK, body = PageView), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_audit_page(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, audit_id, page_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<PageView>> {
    Ok(Json(
        domain::audits::page(&state.pool, &actor, project_id, audit_id, page_id).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::csv_cell;

    #[test]
    fn csv_cells_are_quoted_and_safe() {
        assert_eq!(csv_cell(r#"a "b""#), r#""a ""b""""#);
        assert_eq!(csv_cell("=HYPERLINK(1)"), r#""'=HYPERLINK(1)""#);
        assert_eq!(csv_cell("\t=1+1"), "\"'\t=1+1\"");
        assert_eq!(csv_cell("  +1"), "\"'  +1\"");
    }
}
