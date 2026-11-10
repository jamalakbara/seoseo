use crate::internal;
use crate::models::{Membership, Organization};
use common::AppResult;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

pub async fn insert<'e>(exec: impl PgExecutor<'e>, id: Uuid, name: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(exec)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn add_member<'e>(
    exec: impl PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO members (organization_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(role)
    .execute(exec)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn memberships(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Membership>> {
    sqlx::query_as(
        "SELECT m.organization_id, o.name AS organization_name, m.role
         FROM members m JOIN organizations o ON o.id = m.organization_id
         WHERE m.user_id = $1 ORDER BY m.created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// Role of `user_id` in `org_id`, if the user is a member.
pub async fn role(pool: &PgPool, org_id: Uuid, user_id: Uuid) -> AppResult<Option<String>> {
    sqlx::query_scalar("SELECT role FROM members WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}

pub async fn find(pool: &PgPool, org_id: Uuid) -> AppResult<Option<Organization>> {
    sqlx::query_as("SELECT id, name, created_at FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}
