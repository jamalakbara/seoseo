//! Workspace members and invitations. Invitation links carry a random token; only its hash
//! is stored. Accepting requires the signed-in email to match the invited email.

use crate::actor::{Actor, Role};
use crate::validate;
use chrono::{Duration, Utc};
use common::token::{random_token, sha256_hex};
use common::{AppError, AppResult, new_id};
use db::invitations::{self, Invitation, InvitationPreview, Member};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

pub const INVITE_TTL_DAYS: i64 = 7;

#[derive(Debug, Deserialize, ToSchema)]
pub struct InviteInput {
    pub email: String,
    /// `admin` (default) or `member`.
    pub role: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedInvitation {
    pub invitation: Invitation,
    /// Secret token for the invite link (`/invite/{token}`). Shown once.
    pub token: String,
}

pub async fn invite(
    pool: &PgPool,
    actor: &Actor,
    input: InviteInput,
) -> AppResult<CreatedInvitation> {
    actor.require(Role::Admin)?;
    let email = validate::email(&input.email)?;
    let role = input.role.as_deref().unwrap_or("admin");
    if !matches!(role, "admin" | "member") {
        return Err(AppError::Validation("role must be admin or member".into()));
    }
    if invitations::members(pool, actor.org_id)
        .await?
        .iter()
        .any(|m| m.email.eq_ignore_ascii_case(&email))
    {
        return Err(AppError::Conflict("this person is already a member".into()));
    }
    let token = random_token(24);
    let invitation = invitations::insert(
        pool,
        new_id(),
        actor.org_id,
        &email,
        role,
        &sha256_hex(&token),
        actor.user_id,
        Utc::now() + Duration::days(INVITE_TTL_DAYS),
    )
    .await?;
    Ok(CreatedInvitation { invitation, token })
}

pub async fn list_invitations(pool: &PgPool, actor: &Actor) -> AppResult<Vec<Invitation>> {
    actor.require(Role::Admin)?;
    invitations::list_open(pool, actor.org_id).await
}

pub async fn revoke_invitation(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    invitations::revoke(pool, actor.org_id, id).await
}

pub async fn members(pool: &PgPool, actor: &Actor) -> AppResult<Vec<Member>> {
    invitations::members(pool, actor.org_id).await
}

pub async fn remove_member(pool: &PgPool, actor: &Actor, user_id: Uuid) -> AppResult<()> {
    if user_id != actor.user_id {
        actor.require(Role::Admin)?;
    }
    invitations::remove_member(pool, actor.org_id, user_id).await
}

/// Public: what an invite link is for (no secrets revealed beyond the token holder's own invite).
pub async fn preview(pool: &PgPool, token: &str) -> AppResult<InvitationPreview> {
    invitations::preview(pool, &sha256_hex(token))
        .await?
        .ok_or_else(|| AppError::NotFound("invitation (it may have expired or been revoked)"))
}

/// Signed-in user accepts an invitation addressed to their email.
pub async fn accept(pool: &PgPool, user_id: Uuid, token: &str) -> AppResult<Uuid> {
    let user = db::users::find(pool, user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    let (org_id, role, email) = invitations::claim(&mut tx, &sha256_hex(token))
        .await?
        .ok_or_else(|| AppError::NotFound("invitation (it may have expired or been revoked)"))?;
    if !email.eq_ignore_ascii_case(&user.email) {
        return Err(AppError::Forbidden(format!(
            "this invitation is for {email}; sign in with that account to accept it"
        )));
    }
    // Never downgrade an existing membership.
    sqlx::query(
        "INSERT INTO members (organization_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (organization_id, user_id) DO NOTHING",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(&role)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;
    tx.commit().await.map_err(AppError::internal)?;
    Ok(org_id)
}

/// New account created from an invite link: joins the inviting workspace, no personal one.
pub async fn sign_up_invited(
    pool: &PgPool,
    token: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> AppResult<(Uuid, Uuid)> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    let (org_id, role, invited_email) = invitations::claim(&mut tx, &sha256_hex(token))
        .await?
        .ok_or_else(|| AppError::NotFound("invitation (it may have expired or been revoked)"))?;
    if !invited_email.eq_ignore_ascii_case(email) {
        return Err(AppError::Forbidden(format!(
            "this invitation is for {invited_email}; sign up with that email"
        )));
    }
    let user_id = new_id();
    db::users::insert(&mut *tx, user_id, email, name, Some(password_hash), false).await?;
    db::orgs::add_member(&mut *tx, org_id, user_id, &role).await?;
    tx.commit().await.map_err(AppError::internal)?;
    Ok((user_id, org_id))
}
