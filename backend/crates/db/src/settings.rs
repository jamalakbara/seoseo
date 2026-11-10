use crate::internal;
use common::AppResult;
use sqlx::PgPool;

pub async fn get(pool: &PgPool, key: &str) -> AppResult<Option<serde_json::Value>> {
    sqlx::query_scalar("SELECT value FROM instance_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}

pub async fn set(pool: &PgPool, key: &str, value: serde_json::Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO instance_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}
