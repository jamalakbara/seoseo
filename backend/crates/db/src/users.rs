use crate::models::UserRow;
use crate::{internal, map_err};
use common::AppResult;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

macro_rules! user_columns {
    () => {
        "id, email::text AS email, name, password_hash, is_instance_admin, created_at"
    };
}

pub async fn count(pool: &PgPool) -> AppResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(internal)
}

pub async fn insert<'e>(
    exec: impl PgExecutor<'e>,
    id: Uuid,
    email: &str,
    name: &str,
    password_hash: Option<&str>,
    is_instance_admin: bool,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO users (id, email, name, password_hash, is_instance_admin) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .bind(is_instance_admin)
    .execute(exec)
    .await
    .map_err(|e| map_err(e, "an account with this email already exists; sign in instead"))?;
    Ok(())
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<UserRow>> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM users WHERE email = $1"
    ))
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn find(pool: &PgPool, id: Uuid) -> AppResult<Option<UserRow>> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM users WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn set_password(pool: &PgPool, id: Uuid, password_hash: &str) -> AppResult<()> {
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(password_hash)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
