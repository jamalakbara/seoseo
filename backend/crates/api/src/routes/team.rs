use crate::auth::{Auth, SESSION_COOKIE};
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::CookieJar;
use db::invitations::{Invitation, InvitationPreview, Member};
use domain::team::{CreatedInvitation, InviteInput};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_members))
        .routes(routes!(remove_member))
        .routes(routes!(list_invitations, create_invitation))
        .routes(routes!(revoke_invitation))
        .routes(routes!(preview_invitation))
        .routes(routes!(accept_invitation))
}

#[utoipa::path(get, path = "/team/members", tag = "team",
    responses((status = OK, body = Vec<Member>)), security(("session" = []), ("bearer" = [])))]
async fn list_members(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<Vec<Member>>> {
    Ok(Json(domain::team::members(&state.pool, &actor).await?))
}

/// Remove a member (admins), or leave the workspace (yourself). Owners cannot be removed.
#[utoipa::path(delete, path = "/team/members/{user_id}", tag = "team",
    params(("user_id" = Uuid, Path)), responses((status = NO_CONTENT), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn remove_member(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(user_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    domain::team::remove_member(&state.pool, &actor, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/team/invitations", tag = "team",
    responses((status = OK, body = Vec<Invitation>)), security(("session" = []), ("bearer" = [])))]
async fn list_invitations(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<Vec<Invitation>>> {
    Ok(Json(
        domain::team::list_invitations(&state.pool, &actor).await?,
    ))
}

/// Invite someone by email. Returns a one-time token for the link `/invite/{token}`;
/// share it with them (email delivery arrives with SMTP support).
#[utoipa::path(post, path = "/team/invitations", tag = "team", request_body = InviteInput,
    responses((status = CREATED, body = CreatedInvitation), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn create_invitation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(input): Json<InviteInput>,
) -> ApiResult<(StatusCode, Json<CreatedInvitation>)> {
    Ok((
        StatusCode::CREATED,
        Json(domain::team::invite(&state.pool, &actor, input).await?),
    ))
}

#[utoipa::path(delete, path = "/team/invitations/{id}", tag = "team",
    params(("id" = Uuid, Path)), responses((status = NO_CONTENT), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn revoke_invitation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    domain::team::revoke_invitation(&state.pool, &actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Public: details of an open invitation (for the invite landing page).
#[utoipa::path(get, path = "/invitations/{token}", tag = "team", params(("token" = String, Path)),
    responses((status = OK, body = InvitationPreview), (status = NOT_FOUND, body = Problem)))]
async fn preview_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Json<InvitationPreview>> {
    Ok(Json(domain::team::preview(&state.pool, &token).await?))
}

#[derive(Serialize, ToSchema)]
pub struct AcceptedInvitation {
    organization_id: Uuid,
}

/// Accept an invitation as the signed-in user and switch to that workspace.
#[utoipa::path(post, path = "/invitations/{token}/accept", tag = "team", params(("token" = String, Path)),
    responses((status = OK, body = AcceptedInvitation), (status = FORBIDDEN, body = Problem)),
    security(("session" = [])))]
async fn accept_invitation(
    State(state): State<AppState>,
    jar: CookieJar,
    Auth(actor): Auth,
    Path(token): Path<String>,
) -> ApiResult<Json<AcceptedInvitation>> {
    let org_id = domain::team::accept(&state.pool, actor.user_id, &token).await?;
    if let Some(c) = jar.get(SESSION_COOKIE) {
        domain::auth::switch_org(&state.pool, c.value(), actor.user_id, org_id).await?;
    }
    Ok(Json(AcceptedInvitation {
        organization_id: org_id,
    }))
}
