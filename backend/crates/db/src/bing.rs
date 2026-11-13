//! Project-scoped Bing Webmaster credentials. Plaintext keys never reach this layer.
use crate::internal;
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct Connection {
    pub connected_by: Uuid,
    pub api_key_enc: Vec<u8>,
    pub site_url: Option<String>,
}

pub async fn connection(pool: &PgPool, org: Uuid, project: Uuid) -> AppResult<Option<Connection>> {
    sqlx::query_as("SELECT c.connected_by,c.api_key_enc,c.site_url FROM bing_webmaster_connections c JOIN projects p ON p.id=c.project_id WHERE p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).fetch_optional(pool).await.map_err(internal)
}

pub async fn save(
    pool: &PgPool,
    org: Uuid,
    project: Uuid,
    user: Uuid,
    key: &[u8],
) -> AppResult<()> {
    let result = sqlx::query("INSERT INTO bing_webmaster_connections(project_id,connected_by,api_key_enc) SELECT id,$3,$4 FROM projects WHERE id=$1 AND organization_id=$2 ON CONFLICT(project_id) DO UPDATE SET connected_by=EXCLUDED.connected_by,api_key_enc=EXCLUDED.api_key_enc,site_url=NULL,updated_at=now()")
        .bind(project).bind(org).bind(user).bind(key).execute(pool).await.map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("project not found"));
    }
    Ok(())
}

pub async fn select_site(pool: &PgPool, org: Uuid, project: Uuid, site: &str) -> AppResult<()> {
    let result = sqlx::query("UPDATE bing_webmaster_connections c SET site_url=$3,updated_at=now() FROM projects p WHERE c.project_id=p.id AND p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).bind(site).execute(pool).await.map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Connect Bing Webmaster Tools first".into(),
        ));
    }
    Ok(())
}

pub async fn disconnect(pool: &PgPool, org: Uuid, project: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM bing_webmaster_connections c USING projects p WHERE c.project_id=p.id AND p.id=$1 AND p.organization_id=$2")
        .bind(project).bind(org).execute(pool).await.map_err(internal)?;
    Ok(())
}
