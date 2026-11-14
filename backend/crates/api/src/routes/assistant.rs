use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use db::assistant::{AssistantChat, AssistantTurn};
use domain::assistant as svc;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(assistant_models))
        .routes(routes!(list_assistant_chats, create_assistant_chat))
        .routes(routes!(get_assistant_chat, delete_assistant_chat))
        .routes(routes!(send_assistant_message))
}

#[utoipa::path(get,path="/projects/{project_id}/assistant/models",tag="assistant",params(("project_id"=Uuid,Path)),responses((status=OK,body=svc::AssistantModels)),security(("session"=[]),("bearer"=[])))]
async fn assistant_models(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<svc::AssistantModels>> {
    domain::projects::get(&s.pool, &a, pid).await?;
    Ok(Json(svc::models(&s.cfg)))
}
#[utoipa::path(get,path="/projects/{project_id}/assistant/chats",tag="assistant",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<AssistantChat>)),security(("session"=[]),("bearer"=[])))]
async fn list_assistant_chats(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<AssistantChat>>> {
    Ok(Json(svc::list(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/assistant/chats",tag="assistant",params(("project_id"=Uuid,Path)),request_body=svc::CreateChatInput,responses((status=CREATED,body=AssistantChat)),security(("session"=[]),("bearer"=[])))]
async fn create_assistant_chat(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<svc::CreateChatInput>,
) -> ApiResult<(StatusCode, Json<AssistantChat>)> {
    Ok((
        StatusCode::CREATED,
        Json(svc::create(&s.pool, &a, pid, input).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/assistant/chats/{chat_id}",tag="assistant",params(("project_id"=Uuid,Path),("chat_id"=Uuid,Path)),responses((status=OK,body=svc::AssistantChatView)),security(("session"=[]),("bearer"=[])))]
async fn get_assistant_chat(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<svc::AssistantChatView>> {
    Ok(Json(svc::get(&s.pool, &a, pid, id).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/assistant/chats/{chat_id}",tag="assistant",params(("project_id"=Uuid,Path),("chat_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_assistant_chat(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    svc::delete(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/projects/{project_id}/assistant/chats/{chat_id}/messages",tag="assistant",params(("project_id"=Uuid,Path),("chat_id"=Uuid,Path)),request_body=svc::SendAssistantInput,responses((status=ACCEPTED,body=AssistantTurn)),security(("session"=[]),("bearer"=[])))]
async fn send_assistant_message(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<svc::SendAssistantInput>,
) -> ApiResult<(StatusCode, Json<AssistantTurn>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(svc::send(&s.pool, &a, pid, id, &s.cfg, input).await?),
    ))
}
