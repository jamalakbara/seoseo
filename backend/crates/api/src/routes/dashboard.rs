use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use domain::{dashboard as dash, onboarding as setup};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(dashboard_overview))
        .routes(routes!(dashboard_keywords))
        .routes(routes!(refresh_dashboard))
        .routes(routes!(save_checklist))
        .routes(routes!(research_website))
        .routes(routes!(get_website_setup, save_website_setup))
}

#[utoipa::path(get,path="/projects/{project_id}/dashboard/overview",tag="dashboard",params(("project_id"=Uuid,Path)),responses((status=OK,body=dash::DashboardOverview)),security(("session"=[]),("bearer"=[])))]
async fn dashboard_overview(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<dash::DashboardOverview>> {
    Ok(Json(dash::overview(&s.pool, &a, pid).await?))
}

#[utoipa::path(get,path="/projects/{project_id}/dashboard/keywords",tag="dashboard",params(("project_id"=Uuid,Path)),responses((status=OK,body=dash::KeywordDashboard)),security(("session"=[]),("bearer"=[])))]
async fn dashboard_keywords(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<dash::KeywordDashboard>> {
    Ok(Json(dash::keywords(&s.pool, &a, pid).await?))
}

#[utoipa::path(post,path="/projects/{project_id}/dashboard/refresh",tag="dashboard",params(("project_id"=Uuid,Path)),responses((status=OK,body=dash::KeywordDashboard)),security(("session"=[]),("bearer"=[])))]
async fn refresh_dashboard(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<dash::KeywordDashboard>> {
    Ok(Json(dash::refresh(&s.pool, &a, pid).await?))
}

#[utoipa::path(get,path="/projects/{project_id}/website-setup",tag="dashboard",params(("project_id"=Uuid,Path)),responses((status=OK,body=setup::WebsiteSetupView)),security(("session"=[]),("bearer"=[])))]
async fn get_website_setup(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<setup::WebsiteSetupView>> {
    Ok(Json(setup::get(&s.pool, &a, pid).await?))
}

#[utoipa::path(post,path="/projects/{project_id}/website-setup",tag="dashboard",params(("project_id"=Uuid,Path)),request_body=setup::WebsiteSetupInput,responses((status=OK,body=domain::memory::ProjectContext)),security(("session"=[]),("bearer"=[])))]
async fn save_website_setup(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<setup::WebsiteSetupInput>,
) -> ApiResult<Json<domain::memory::ProjectContext>> {
    Ok(Json(setup::save(&s.pool, &a, pid, input).await?))
}

#[utoipa::path(post,path="/projects/{project_id}/website-research",tag="dashboard",params(("project_id"=Uuid,Path)),request_body=setup::WebsiteResearchInput,responses((status=OK,body=db::audits::Audit)),security(("session"=[]),("bearer"=[])))]
async fn research_website(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<setup::WebsiteResearchInput>,
) -> ApiResult<Json<db::audits::Audit>> {
    Ok(Json(
        setup::research(
            &s.pool,
            &a,
            pid,
            crawler::UrlPolicy {
                allow_private: s.cfg.crawler_allow_private,
            },
            s.cfg.renderer_url.as_ref().is_some_and(|u| !u.is_empty()),
            input,
        )
        .await?,
    ))
}

#[utoipa::path(put,path="/projects/{project_id}/dashboard/checklist",tag="dashboard",params(("project_id"=Uuid,Path)),request_body=dash::ChecklistInput,responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn save_checklist(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<dash::ChecklistInput>,
) -> ApiResult<StatusCode> {
    dash::checklist(&s.pool, &a, pid, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
