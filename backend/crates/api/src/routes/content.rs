use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use domain::content as svc;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(generate_content_brief))
        .routes(routes!(score_content))
}
#[utoipa::path(post,path="/projects/{project_id}/content/brief",tag="assistant",params(("project_id"=Uuid,Path)),request_body=svc::ContentBriefInput,responses((status=ACCEPTED,body=domain::assistant::AssistantChatView)),security(("session"=[]),("bearer"=[])))]
async fn generate_content_brief(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<svc::ContentBriefInput>,
) -> ApiResult<(StatusCode, Json<domain::assistant::AssistantChatView>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(svc::brief(&s.pool, &a, pid, &s.cfg, input).await?),
    ))
}
#[utoipa::path(post,path="/projects/{project_id}/content/score",tag="assistant",params(("project_id"=Uuid,Path)),request_body=svc::OptimizeContentInput,responses((status=OK,body=svc::ContentScore)),security(("session"=[]),("bearer"=[])))]
async fn score_content(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<svc::OptimizeContentInput>,
) -> ApiResult<Json<svc::ContentScore>> {
    Ok(Json(svc::optimize(&s.pool, &a, pid, input).await?))
}
