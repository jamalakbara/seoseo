use crate::internal;
use crate::models::{ApiKey, ApiKeyAuthRow};
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    org_id: Uuid,
    user_id: Uuid,
    name: &str,
    prefix: &str,
    key_hash: &str,
) -> AppResult<ApiKey> {
    sqlx::query_as(
        "INSERT INTO api_keys (id, organization_id, user_id, name, prefix, key_hash)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, organization_id, user_id, name, prefix, last_used_at, created_at",
    )
    .bind(id)
    .bind(org_id)
    .bind(user_id)
    .bind(name)
    .bind(prefix)
    .bind(key_hash)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> AppResult<Vec<ApiKey>> {
    sqlx::query_as(
        "SELECT id, organization_id, user_id, name, prefix, last_used_at, created_at
         FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn revoke(pool: &PgPool, org_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query(
        "UPDATE api_keys SET revoked_at = now()
         WHERE id = $1 AND organization_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(org_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("api key"));
    }
    Ok(())
}

/// Resolve a key hash to its owner, requiring a live membership in the bound organization.
pub async fn authenticate(pool: &PgPool, key_hash: &str) -> AppResult<Option<ApiKeyAuthRow>> {
    sqlx::query_as(
        "UPDATE api_keys k SET last_used_at = now()
         FROM members m
         WHERE k.key_hash = $1 AND k.revoked_at IS NULL
           AND m.organization_id = k.organization_id AND m.user_id = k.user_id
         RETURNING k.id, k.organization_id, k.user_id",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}
