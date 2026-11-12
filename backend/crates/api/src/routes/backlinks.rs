use crate::{
    auth::Auth,
    error::{ApiResult, Problem},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use db::backlinks::BacklinkSnapshot;
use domain::backlinks::{BacklinkQuery, BacklinkSnapshotView, ImportBacklinksInput};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_backlink_snapshots, import_backlink_snapshot))
        .routes(routes!(get_backlink_snapshot, delete_backlink_snapshot))
}

#[utoipa::path(get,path="/projects/{project_id}/backlink-snapshots",tag="backlinks",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<BacklinkSnapshot>)),security(("session"=[]),("bearer"=[])))]
async fn list_backlink_snapshots(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<BacklinkSnapshot>>> {
    Ok(Json(domain::backlinks::list(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/backlink-snapshots",tag="backlinks",params(("project_id"=Uuid,Path)),request_body=ImportBacklinksInput,responses((status=CREATED,body=BacklinkSnapshot),(status=CONFLICT,body=Problem),(status=UNPROCESSABLE_ENTITY,body=Problem)),security(("session"=[]),("bearer"=[])))]
async fn import_backlink_snapshot(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(input): Json<ImportBacklinksInput>,
) -> ApiResult<(StatusCode, Json<BacklinkSnapshot>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::backlinks::import(&s.pool, &a, pid, input).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/backlink-snapshots/{snapshot_id}",tag="backlinks",params(("project_id"=Uuid,Path),("snapshot_id"=Uuid,Path),BacklinkQuery),responses((status=OK,body=BacklinkSnapshotView)),security(("session"=[]),("bearer"=[])))]
async fn get_backlink_snapshot(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
    Query(q): Query<BacklinkQuery>,
) -> ApiResult<Json<BacklinkSnapshotView>> {
    Ok(Json(domain::backlinks::get(&s.pool, &a, pid, id, q).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/backlink-snapshots/{snapshot_id}",tag="backlinks",params(("project_id"=Uuid,Path),("snapshot_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_backlink_snapshot(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::backlinks::delete(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
