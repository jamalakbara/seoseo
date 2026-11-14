use crate::{auth::Auth, error::ApiResult, state::AppState};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use db::{notifications as nr, operations as or};
use domain::{notifications as n, operations as o};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_alert_settings, save_alert_settings))
        .routes(routes!(list_notification_events))
        .routes(routes!(evaluate_notification_events))
        .routes(routes!(acknowledge_notification_event))
        .routes(routes!(
            list_notification_endpoints,
            create_notification_endpoint
        ))
        .routes(routes!(
            toggle_notification_endpoint,
            delete_notification_endpoint
        ))
        .routes(routes!(test_notification_endpoint))
        .routes(routes!(list_notification_deliveries))
        .routes(routes!(get_digest_subscription, save_digest_subscription))
        .routes(routes!(generate_builtin_report))
        .routes(routes!(export_report_pdf))
        .routes(routes!(get_business_profile, save_business_profile))
        .routes(routes!(list_business_records, save_business_record))
        .routes(routes!(delete_business_record))
        .routes(routes!(get_live_places_details))
        .routes(routes!(
            get_account_deletion,
            request_account_deletion,
            cancel_account_deletion
        ))
}

#[utoipa::path(post,path="/projects/{project_id}/business/places",tag="local-seo",params(("project_id"=Uuid,Path)),request_body=o::PlacesDetailsInput,responses((status=OK,body=o::PlacesDetailsView)),security(("session"=[]),("bearer"=[])))]
async fn get_live_places_details(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<o::PlacesDetailsInput>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(o::places_details(&s.pool, &a, pid, &s.cfg, v).await?),
    ))
}

#[utoipa::path(get,path="/projects/{project_id}/notifications/settings",tag="notifications",params(("project_id"=Uuid,Path)),responses((status=OK,body=Option<nr::NotificationSettings>)),security(("session"=[]),("bearer"=[])))]
async fn get_alert_settings(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Option<nr::NotificationSettings>>> {
    Ok(Json(n::settings(&s.pool, &a, pid).await?))
}
#[utoipa::path(put,path="/projects/{project_id}/notifications/settings",tag="notifications",params(("project_id"=Uuid,Path)),request_body=n::AlertSettingsInput,responses((status=OK,body=nr::NotificationSettings)),security(("session"=[]),("bearer"=[])))]
async fn save_alert_settings(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<n::AlertSettingsInput>,
) -> ApiResult<Json<nr::NotificationSettings>> {
    Ok(Json(n::save_settings(&s.pool, &a, pid, v).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/notifications/events",tag="notifications",params(("project_id"=Uuid,Path),n::AlertQuery),responses((status=OK,body=Vec<nr::NotificationEvent>)),security(("session"=[]),("bearer"=[])))]
async fn list_notification_events(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Query(q): Query<n::AlertQuery>,
) -> ApiResult<Json<Vec<nr::NotificationEvent>>> {
    Ok(Json(n::events(&s.pool, &a, pid, q).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/notifications/evaluate",tag="notifications",params(("project_id"=Uuid,Path)),responses((status=OK,body=usize)),security(("session"=[]),("bearer"=[])))]
async fn evaluate_notification_events(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<usize>> {
    Ok(Json(n::evaluate(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/notifications/events/{event_id}/acknowledge",tag="notifications",params(("project_id"=Uuid,Path),("event_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn acknowledge_notification_event(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    n::acknowledge(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/projects/{project_id}/notifications/endpoints",tag="notifications",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<nr::NotificationEndpoint>)),security(("session"=[]),("bearer"=[])))]
async fn list_notification_endpoints(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<nr::NotificationEndpoint>>> {
    Ok(Json(n::endpoints(&s.pool, &a, pid).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/notifications/endpoints",tag="notifications",params(("project_id"=Uuid,Path)),request_body=n::EndpointInput,responses((status=OK,body=nr::NotificationEndpoint)),security(("session"=[]),("bearer"=[])))]
async fn create_notification_endpoint(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<n::EndpointInput>,
) -> ApiResult<Json<nr::NotificationEndpoint>> {
    Ok(Json(n::create_endpoint(&s.pool, &a, pid, &s.cfg, v).await?))
}
#[utoipa::path(patch,path="/projects/{project_id}/notifications/endpoints/{endpoint_id}",tag="notifications",params(("project_id"=Uuid,Path),("endpoint_id"=Uuid,Path)),request_body=n::EndpointToggle,responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn toggle_notification_endpoint(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
    Json(v): Json<n::EndpointToggle>,
) -> ApiResult<StatusCode> {
    n::toggle_endpoint(&s.pool, &a, pid, id, v).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete,path="/projects/{project_id}/notifications/endpoints/{endpoint_id}",tag="notifications",params(("project_id"=Uuid,Path),("endpoint_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_notification_endpoint(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    n::delete_endpoint(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/projects/{project_id}/notifications/endpoints/{endpoint_id}/test",tag="notifications",params(("project_id"=Uuid,Path),("endpoint_id"=Uuid,Path)),responses((status=ACCEPTED,body=Uuid)),security(("session"=[]),("bearer"=[])))]
async fn test_notification_endpoint(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<(StatusCode, Json<Uuid>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(n::test_endpoint(&s.pool, &a, pid, id).await?),
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/notifications/deliveries",tag="notifications",params(("project_id"=Uuid,Path)),responses((status=OK,body=Vec<nr::NotificationDelivery>)),security(("session"=[]),("bearer"=[])))]
async fn list_notification_deliveries(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Vec<nr::NotificationDelivery>>> {
    Ok(Json(n::deliveries(&s.pool, &a, pid).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/notifications/digest",tag="notifications",params(("project_id"=Uuid,Path)),responses((status=OK,body=n::DigestView)),security(("session"=[]),("bearer"=[])))]
async fn get_digest_subscription(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<n::DigestView>> {
    Ok(Json(n::digest(&s.pool, &a, pid, &s.cfg).await?))
}
#[utoipa::path(put,path="/projects/{project_id}/notifications/digest",tag="notifications",params(("project_id"=Uuid,Path)),request_body=n::DigestInput,responses((status=OK,body=n::DigestView)),security(("session"=[]),("bearer"=[])))]
async fn save_digest_subscription(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<n::DigestInput>,
) -> ApiResult<Json<n::DigestView>> {
    Ok(Json(n::save_digest(&s.pool, &a, pid, &s.cfg, v).await?))
}
#[utoipa::path(post,path="/projects/{project_id}/reports/generate",tag="reports",params(("project_id"=Uuid,Path)),request_body=o::GenerateReportInput,responses((status=OK,body=db::models::Report)),security(("session"=[]),("bearer"=[])))]
async fn generate_builtin_report(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<o::GenerateReportInput>,
) -> ApiResult<Json<db::models::Report>> {
    Ok(Json(o::generate_report(&s.pool, &a, pid, v).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/reports/{report_id}/pdf",tag="reports",params(("project_id"=Uuid,Path),("report_id"=Uuid,Path)),responses((status=OK,content_type="application/pdf",body=Vec<u8>)),security(("session"=[]),("bearer"=[])))]
async fn export_report_pdf(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let bytes = o::pdf(&s.pool, &a, pid, id, &s.cfg).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (header::CACHE_CONTROL, "no-store"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=seoseo-report.pdf",
            ),
        ],
        bytes,
    ))
}
#[utoipa::path(get,path="/projects/{project_id}/business/profile",tag="local-seo",params(("project_id"=Uuid,Path)),responses((status=OK,body=Option<or::BusinessProfile>)),security(("session"=[]),("bearer"=[])))]
async fn get_business_profile(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
) -> ApiResult<Json<Option<or::BusinessProfile>>> {
    Ok(Json(o::profile(&s.pool, &a, pid).await?))
}
#[utoipa::path(put,path="/projects/{project_id}/business/profile",tag="local-seo",params(("project_id"=Uuid,Path)),request_body=o::BusinessProfileInput,responses((status=OK,body=or::BusinessProfile)),security(("session"=[]),("bearer"=[])))]
async fn save_business_profile(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<o::BusinessProfileInput>,
) -> ApiResult<Json<or::BusinessProfile>> {
    Ok(Json(o::save_profile(&s.pool, &a, pid, v).await?))
}
#[utoipa::path(get,path="/projects/{project_id}/business/entries",tag="local-seo",params(("project_id"=Uuid,Path),n::AlertQuery),responses((status=OK,body=Vec<or::BusinessEntry>)),security(("session"=[]),("bearer"=[])))]
async fn list_business_records(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Query(q): Query<n::AlertQuery>,
) -> ApiResult<Json<Vec<or::BusinessEntry>>> {
    Ok(Json(o::business_entries(&s.pool, &a, pid, q).await?))
}
#[utoipa::path(put,path="/projects/{project_id}/business/entries",tag="local-seo",params(("project_id"=Uuid,Path)),request_body=o::BusinessEntryInput,responses((status=OK,body=or::BusinessEntry)),security(("session"=[]),("bearer"=[])))]
async fn save_business_record(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path(pid): Path<Uuid>,
    Json(v): Json<o::BusinessEntryInput>,
) -> ApiResult<Json<or::BusinessEntry>> {
    Ok(Json(o::save_business_entry(&s.pool, &a, pid, v).await?))
}
#[utoipa::path(delete,path="/projects/{project_id}/business/entries/{entry_id}",tag="local-seo",params(("project_id"=Uuid,Path),("entry_id"=Uuid,Path)),responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn delete_business_record(
    State(s): State<AppState>,
    Auth(a): Auth,
    Path((pid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    o::delete_business_entry(&s.pool, &a, pid, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/account/deletion",tag="auth",responses((status=OK,body=Option<or::AccountDeletion>)),security(("session"=[]),("bearer"=[])))]
async fn get_account_deletion(
    State(s): State<AppState>,
    Auth(a): Auth,
) -> ApiResult<Json<Option<or::AccountDeletion>>> {
    Ok(Json(o::account_deletion(&s.pool, &a).await?))
}
#[utoipa::path(post,path="/account/deletion",tag="auth",request_body=o::DeleteAccountInput,responses((status=OK,body=or::AccountDeletion)),security(("session"=[]),("bearer"=[])))]
async fn request_account_deletion(
    State(s): State<AppState>,
    Auth(a): Auth,
    Json(v): Json<o::DeleteAccountInput>,
) -> ApiResult<Json<or::AccountDeletion>> {
    Ok(Json(o::request_account_deletion(&s.pool, &a, v).await?))
}
#[utoipa::path(delete,path="/account/deletion",tag="auth",responses((status=NO_CONTENT)),security(("session"=[]),("bearer"=[])))]
async fn cancel_account_deletion(
    State(s): State<AppState>,
    Auth(a): Auth,
) -> ApiResult<StatusCode> {
    o::cancel_account_deletion(&s.pool, &a).await?;
    Ok(StatusCode::NO_CONTENT)
}
