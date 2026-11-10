//! Organization-bound API keys for the REST API and MCP clients.

use crate::actor::{Actor, ActorVia, Role};
use crate::validate;
use common::token::{random_token, sha256_hex};
use common::{AppResult, new_id};
use db::models::ApiKey;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

pub const KEY_PREFIX: &str = "seo_";

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyInput {
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedApiKey {
    pub key: ApiKey,
    /// The full secret. Shown once; store it now.
    pub secret: String,
}

pub async fn create(
    pool: &PgPool,
    actor: &Actor,
    input: CreateApiKeyInput,
) -> AppResult<CreatedApiKey> {
    let name = validate::non_empty("name", &input.name, 80)?;
    let secret = format!("{KEY_PREFIX}{}", random_token(32));
    let prefix: String = secret.chars().take(12).collect();
    let key = db::api_keys::insert(
        pool,
        new_id(),
        actor.org_id,
        actor.user_id,
        &name,
        &prefix,
        &sha256_hex(&secret),
    )
    .await?;
    Ok(CreatedApiKey { key, secret })
}

pub async fn list(pool: &PgPool, actor: &Actor) -> AppResult<Vec<ApiKey>> {
    db::api_keys::list(pool, actor.org_id).await
}

pub async fn revoke(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<()> {
    db::api_keys::revoke(pool, actor.org_id, id).await
}

/// Resolve a bearer secret. Returns None for unknown/revoked keys or removed members.
pub async fn authenticate(pool: &PgPool, secret: &str) -> AppResult<Option<Actor>> {
    if !secret.starts_with(KEY_PREFIX) {
        return Ok(None);
    }
    let Some(row) = db::api_keys::authenticate(pool, &sha256_hex(secret)).await? else {
        return Ok(None);
    };
    let role = db::orgs::role(pool, row.organization_id, row.user_id)
        .await?
        .and_then(|r| Role::parse(&r));
    Ok(role.map(|role| Actor {
        user_id: row.user_id,
        org_id: row.organization_id,
        role,
        via: ActorVia::ApiKey,
    }))
}
