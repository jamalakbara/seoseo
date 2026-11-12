use crate::{
    auth::Auth,
    error::{ApiResult, Problem},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use db::ai_visibility::{AiVisibilityRun, AiVisibilityTracker};
use domain::ai_visibility as ai;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ai_visibility_models))
        .routes(routes!(list_ai_trackers, create_ai_tracker))
        .routes(routes!(get_ai_tracker, delete_ai_tracker))
        .routes(routes!(start_ai_run))
        .routes(routes!(get_ai_run, delete_ai_run))
        .routes(routes!(save_ai_schedule))
        .routes(routes!(research_ai_prompts))
        .routes(routes!(ai_visibility_summary))
}

#[utoipa::path(patch,path="/projects/{project_id}/ai-trackers/{tracker_id}/schedule",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),request_body=ai::AiScheduleInput,responses((status=OK,body=AiVisibilityTracker)),security(("session"=[]),("bearer"=[])))]
async fn save_ai_schedule(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(input): Json<ai::AiScheduleInput>,
) -> ApiResult<Json<AiVisibilityTracker>> {
    Ok(Json(
        ai::schedule(&s.pool, &a, pid, tid, &s.cfg, input).await?,
    ))
}
#[utoipa::path(post,path="/projects/{project_id}/ai-prompt-research",tag="ai-visibility",params(("project_id"=Uuid,Path)),request_body=ai::AiPromptResearchInput,responses((status=OK,body=ai::AiPromptDraft)),security(("session"=[]),("bearer"=[])))]
async fn research_ai_prompts(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<ai::AiPromptResearchInput>,
) -> ApiResult<Json<ai::AiPromptDraft>> {
    Ok(Json(
        ai::prompt_research(&s.pool, &a, pid, &s.cfg, input).await?,
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/ai-trackers/{tracker_id}/summary",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),responses((status=OK,body=ai::AiVisibilitySummary)),security(("session"=[]),("bearer"=[])))]
async fn ai_visibility_summary(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ai::AiVisibilitySummary>> {
    Ok(Json(ai::summary(&s.pool, &a, pid, tid).await?))
}
#[utoipa::path(get,path="/ai-visibility/models",tag="ai-visibility",responses((status=OK,body=ai::AiModelAvailability)),security(("session"=[]),("bearer"=[])))]
async fn ai_visibility_models(
    State(s): State<AppState>,
    Auth(_actor): Auth,
) -> Json<ai::AiModelAvailability> {
    Json(ai::models(&s.cfg))
}
#[utoipa::path(get,path="/projects/{project_id}/ai-trackers",tag="ai-visibility",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<AiVisibilityTracker>)),security(("session"=[]),("bearer"=[])))]
async fn list_ai_trackers(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<AiVisibilityTracker>>> {
    Ok(Json(ai::list(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/ai-trackers",tag="ai-visibility",params(("project_id"=Uuid,Path)),request_body=ai::CreateAiTrackerInput,responses((status=CREATED,body=AiVisibilityTracker),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn create_ai_tracker(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<ai::CreateAiTrackerInput>,
) -> ApiResult<(StatusCode, Json<AiVisibilityTracker>)> {
    Ok((
        StatusCode::CREATED,
        Json(ai::create(&s.pool, &a, pid, &s.cfg, input).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/ai-trackers/{tracker_id}",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),responses((status=OK,body=ai::AiTrackerView)),security(("session"=[]),("bearer"=[])))]
async fn get_ai_tracker(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ai::AiTrackerView>> {
    Ok(Json(ai::get(&s.pool, &a, pid, tid).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/ai-trackers/{tracker_id}",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_ai_tracker(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    ai::delete(&s.pool, &a, pid, tid, None).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/projects/{project_id}/ai-trackers/{tracker_id}/runs",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path)),request_body=ai::StartAiRunInput,responses((status=ACCEPTED,body=AiVisibilityRun),(status=CONFLICT,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn start_ai_run(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(input): Json<ai::StartAiRunInput>,
) -> ApiResult<(StatusCode, Json<AiVisibilityRun>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(ai::start(&s.pool, &a, pid, tid, &s.cfg, input).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/ai-trackers/{tracker_id}/runs/{run_id}",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path),("run_id"=Uuid,Path)),responses((status=OK,body=ai::AiRunView)),security(("session"=[]),("bearer"=[])))]
async fn get_ai_run(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ai::AiRunView>> {
    Ok(Json(ai::run(&s.pool, &a, pid, tid, rid).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/ai-trackers/{tracker_id}/runs/{run_id}",tag="ai-visibility",params(("project_id"=Uuid,Path),("tracker_id"=Uuid,Path),("run_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_ai_run(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, tid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    ai::delete(&s.pool, &a, pid, tid, Some(rid)).await?;
    Ok(StatusCode::NO_CONTENT)
}
