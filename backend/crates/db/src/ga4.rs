//! Project-scoped Google grants. Tokens never appear in serialized views.
use crate::internal;
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct Connection {
    pub connected_by: Uuid,
    pub refresh_token_enc: Vec<u8>,
    pub property_id: Option<String>,
}

pub async fn connection(pool: &PgPool, org: Uuid, project: Uuid) -> AppResult<Option<Connection>> {
    sqlx::query_as("SELECT c.connected_by, c.refresh_token_enc, c.property_id FROM ga4_connections c JOIN projects p ON p.id=c.project_id WHERE p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).fetch_optional(pool).await.map_err(internal)
}

pub async fn begin(
    pool: &PgPool,
    org: Uuid,
    project: Uuid,
    user: Uuid,
    hash: &str,
    verifier: &[u8],
) -> AppResult<()> {
    sqlx::query("DELETE FROM ga4_oauth_states WHERE expires_at < now()")
        .execute(pool)
        .await
        .map_err(internal)?;
    sqlx::query("INSERT INTO ga4_oauth_states (state_hash,project_id,organization_id,user_id,verifier_enc) SELECT $1,id,organization_id,$4,$5 FROM projects WHERE id=$2 AND organization_id=$3")
        .bind(hash).bind(project).bind(org).bind(user).bind(verifier).execute(pool).await.map_err(internal)?;
    Ok(())
}

pub async fn consume(
    pool: &PgPool,
    org: Uuid,
    user: Uuid,
    hash: &str,
) -> AppResult<(Uuid, Vec<u8>)> {
    sqlx::query_as("DELETE FROM ga4_oauth_states WHERE state_hash=$1 AND organization_id=$2 AND user_id=$3 AND expires_at>now() RETURNING project_id,verifier_enc")
        .bind(hash).bind(org).bind(user).fetch_optional(pool).await.map_err(internal)?
        .ok_or_else(|| AppError::Validation("Google connection expired or already used; connect again".into()))
}

pub async fn save(
    pool: &PgPool,
    org: Uuid,
    project: Uuid,
    user: Uuid,
    token: &[u8],
) -> AppResult<()> {
    sqlx::query("INSERT INTO ga4_connections (project_id,connected_by,refresh_token_enc) SELECT id,$3,$4 FROM projects WHERE id=$1 AND organization_id=$2 ON CONFLICT (project_id) DO UPDATE SET connected_by=EXCLUDED.connected_by,refresh_token_enc=EXCLUDED.refresh_token_enc,property_id=NULL,updated_at=now()")
        .bind(project).bind(org).bind(user).bind(token).execute(pool).await.map_err(internal)?;
    Ok(())
}

pub async fn select_property(pool: &PgPool, org: Uuid, project: Uuid, site: &str) -> AppResult<()> {
    let r = sqlx::query("UPDATE ga4_connections c SET property_id=$3,updated_at=now() FROM projects p WHERE c.project_id=p.id AND p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).bind(site).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Connect Google in this project's Analytics tab first".into(),
        ));
    }
    Ok(())
}

pub async fn disconnect(pool: &PgPool, org: Uuid, project: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("DELETE FROM ga4_connections c USING projects p WHERE c.project_id=p.id AND p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("DELETE FROM ga4_oauth_states WHERE project_id=$1 AND organization_id=$2")
        .bind(project)
        .bind(org)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)
}

/// Rotate only the grant that was read; never overwrite a reconnect with a late response.
pub async fn rotate(
    pool: &PgPool,
    org: Uuid,
    project: Uuid,
    old: &[u8],
    new: &[u8],
) -> AppResult<()> {
    sqlx::query("UPDATE ga4_connections c SET refresh_token_enc=$4 FROM projects p WHERE c.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND c.refresh_token_enc=$3").bind(org).bind(project).bind(old).bind(new).execute(pool).await.map_err(internal)?;
    Ok(())
}
