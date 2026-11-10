use crate::internal;
use crate::models::SessionRow;
use chrono::{DateTime, Utc};
use common::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert(
    pool: &PgPool,
    token_hash: &str,
    user_id: Uuid,
    org_id: Option<Uuid>,
    user_agent: Option<&str>,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, organization_id, user_agent, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(org_id)
    .bind(user_agent)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Find a live session and bump `last_seen_at`.
pub async fn touch(pool: &PgPool, token_hash: &str) -> AppResult<Option<SessionRow>> {
    sqlx::query_as(
        "UPDATE sessions SET last_seen_at = now()
         WHERE token_hash = $1 AND expires_at > now()
         RETURNING user_id, organization_id, expires_at",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn set_org(pool: &PgPool, token_hash: &str, org_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE sessions SET organization_id = $2 WHERE token_hash = $1")
        .bind(token_hash)
        .bind(org_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn delete(pool: &PgPool, token_hash: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn delete_expired(pool: &PgPool) -> AppResult<u64> {
    let r = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(r.rows_affected())
}
