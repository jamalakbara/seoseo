use crate::{internal, map_err};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Invitation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
    pub role: String,
    pub invited_by: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

macro_rules! inv_columns {
    () => {
        "id, organization_id, email::text AS email, role, invited_by, expires_at, accepted_at, created_at"
    };
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    org_id: Uuid,
    email: &str,
    role: &str,
    token_hash: &str,
    invited_by: Uuid,
    expires_at: DateTime<Utc>,
) -> AppResult<Invitation> {
    let mut tx = pool.begin().await.map_err(internal)?;
    // An expired open invitation must not block a fresh one.
    sqlx::query(
        "UPDATE invitations SET revoked_at = now()
         WHERE organization_id = $1 AND email = $2 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at <= now()",
    )
    .bind(org_id)
    .bind(email)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let inv = sqlx::query_as(concat!(
        "INSERT INTO invitations (id, organization_id, email, role, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING ",
        inv_columns!()
    ))
    .bind(id)
    .bind(org_id)
    .bind(email)
    .bind(role)
    .bind(token_hash)
    .bind(invited_by)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_err(e, "this email already has an open invitation; revoke it to send a new one"))?;
    tx.commit().await.map_err(internal)?;
    Ok(inv)
}

pub async fn list_open(pool: &PgPool, org_id: Uuid) -> AppResult<Vec<Invitation>> {
    sqlx::query_as(concat!(
        "SELECT ",
        inv_columns!(),
        " FROM invitations WHERE organization_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL
          ORDER BY created_at DESC"
    ))
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn revoke(pool: &PgPool, org_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query(
        "UPDATE invitations SET revoked_at = now()
         WHERE id = $1 AND organization_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(org_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("invitation"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct InvitationPreview {
    pub email: String,
    pub role: String,
    pub organization_name: String,
    pub expires_at: DateTime<Utc>,
}

/// Look up a usable (open, unexpired) invitation by token hash.
pub async fn preview(pool: &PgPool, token_hash: &str) -> AppResult<Option<InvitationPreview>> {
    sqlx::query_as(
        "SELECT i.email::text AS email, i.role, o.name AS organization_name, i.expires_at
         FROM invitations i JOIN organizations o ON o.id = i.organization_id
         WHERE i.token_hash = $1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

/// Mark the invitation accepted and return (org, role, email). Must run in the caller's transaction.
pub async fn claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_hash: &str,
) -> AppResult<Option<(Uuid, String, String)>> {
    sqlx::query_as(
        "UPDATE invitations SET accepted_at = now()
         WHERE token_hash = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now()
         RETURNING organization_id, role, email::text",
    )
    .bind(token_hash)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

pub async fn members(pool: &PgPool, org_id: Uuid) -> AppResult<Vec<Member>> {
    sqlx::query_as(
        "SELECT u.id AS user_id, u.email::text AS email, u.name, m.role, m.created_at AS joined_at
         FROM members m JOIN users u ON u.id = m.user_id
         WHERE m.organization_id = $1
         ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, m.created_at",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn remove_member(pool: &PgPool, org_id: Uuid, user_id: Uuid) -> AppResult<()> {
    let r = sqlx::query(
        "DELETE FROM members WHERE organization_id = $1 AND user_id = $2 AND role <> 'owner'",
    )
    .bind(org_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "member not found, or is the owner (transfer ownership first)".into(),
        ));
    }
    // Their sessions keep working for other workspaces; drop the active pointer here.
    sqlx::query(
        "UPDATE sessions SET organization_id = NULL WHERE user_id = $1 AND organization_id = $2",
    )
    .bind(user_id)
    .bind(org_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}
