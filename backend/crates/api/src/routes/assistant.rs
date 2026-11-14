use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use db::assistant::{AssistantChat, AssistantTurn};
use domain::assistant as svc;
use futures::Stream;
use std::{convert::Infallible, time::Duration};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(assistant_models))
        .routes(routes!(list_assistant_chats, create_assistant_chat))
        .routes(routes!(get_assistant_chat, delete_assistant_chat))
        .routes(routes!(send_assistant_message))
        .routes(routes!(assistant_turn_events))
        .routes(routes!(approve_assistant_memory_note))
}

#[utoipa::path(post,path="/projects/{project_id}/assistant/chats/{chat_id}/turns/{turn_id}/memory",tag="assistant",params(("project_id"=Uuid,Path),("chat_id"=Uuid,Path),("turn_id"=Uuid,Path)),request_body=svc::ApproveMemoryNoteInput,responses((status=OK,body=domain::memory::ProjectContext)),security(("session"=[]),("bearer"=[])))]
async fn approve_assistant_memory_note(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, cid, tid)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<svc::ApproveMemoryNoteInput>,
) -> ApiResult<Json<domain::memory::ProjectContext>> {
    Ok(Json(
        svc::approve_memory_note(&s.pool, &a, pid, cid, tid, input).await?,
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/assistant/chats/{chat_id}/turns/{turn_id}/events",tag="assistant",params(("project_id"=Uuid,Path),("chat_id"=Uuid,Path),("turn_id"=Uuid,Path)),responses((status=OK,content_type="text/event-stream",body=String)),security(("session"=[]),("bearer"=[])))]
async fn assistant_turn_events(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, cid, tid)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<(
    [(&'static str, &'static str); 2],
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
)> {
    svc::turn(&s.pool, &a, pid, cid, tid).await?;
    let stream = async_stream::stream! {
        let mut last=String::new();
        // Close before the HTTP timeout; EventSource reconnects without restarting inference.
        for _ in 0..180 {
            let turn=match svc::turn(&s.pool,&a,pid,cid,tid).await {
                Ok(t)=>t,Err(_)=>{yield Ok(Event::default().event("unavailable").data("Conversation unavailable"));break;}
            };
            let terminal=!matches!(turn.status.as_str(),"queued"|"running");
            let body=serde_json::to_string(&turn).unwrap_or_default();
            if body!=last {yield Ok(Event::default().event("turn").data(body.clone()));last=body;}
            if terminal {break;}
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    };
    Ok((
        [
            ("Cache-Control", "no-store, no-transform"),
            ("X-Accel-Buffering", "no"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
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
