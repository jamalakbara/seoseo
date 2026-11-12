use crate::internal;
use common::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiscoveryConfig {
    pub search: bool,
    pub source_urls: Vec<String>,
    pub country: String,
    pub language: String,
    pub created_by: Uuid,
}
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct BacklinkDiscovery {
    pub id: Uuid,
    pub project_id: Uuid,
    pub domain: String,
    pub job_id: Option<Uuid>,
    pub status: String,
    #[sqlx(json)]
    pub config: DiscoveryConfig,
    pub snapshot_id: Option<Uuid>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
macro_rules! cols {
    () => {
        "id,project_id,domain,job_id,status,config,snapshot_id,error,created_at"
    };
}
pub async fn reconcile(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE backlink_discoveries d SET status='failed',error='Background job stopped; previous snapshots preserved' FROM projects p WHERE p.id=d.project_id AND p.organization_id=$1 AND p.id=$2 AND d.status IN ('queued','running') AND (d.job_id IS NULL OR EXISTS(SELECT 1 FROM jobs j WHERE j.id=d.job_id AND j.status IN ('failed','cancelled')))").bind(org).bind(pid).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn list(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<BacklinkDiscovery>> {
    reconcile(pool, org, pid).await?;
    sqlx::query_as(concat!("SELECT ",cols!()," FROM backlink_discoveries WHERE project_id=$2 AND domain=(SELECT domain::text FROM projects WHERE organization_id=$1 AND id=$2) ORDER BY created_at DESC LIMIT 20")).bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}
pub async fn start(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    expected: &str,
    config: DiscoveryConfig,
) -> AppResult<BacklinkDiscovery> {
    reconcile(pool, org, pid).await?;
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL AND domain::text=$3 FOR UPDATE").bind(org).bind(pid).bind(expected).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active current-website project"))?;
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM backlink_discoveries WHERE project_id=$1 AND (status IN ('queued','running') OR created_at>clock_timestamp()-interval '1 minute'))").bind(pid).fetch_one(&mut *tx).await.map_err(internal)?;
    if active {
        return Err(AppError::Conflict(
            "Discovery is already active or cooling down for one minute".into(),
        ));
    }
    let quota: i64 =
        sqlx::query_scalar("SELECT count(*) FROM backlink_snapshots WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if quota >= 100 {
        return Err(AppError::Conflict(
            "Delete an old backlink snapshot before discovery (100/project)".into(),
        ));
    }
    sqlx::query("DELETE FROM backlink_discoveries WHERE project_id=$1 AND id IN (SELECT id FROM backlink_discoveries WHERE project_id=$1 AND status IN ('completed','failed') ORDER BY created_at DESC OFFSET 19)").bind(pid).execute(&mut *tx).await.map_err(internal)?;
    let id = new_id();
    let job = new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'backlink_discovery',$2,$3,$4,1)").bind(job).bind(org).bind(pid).bind(serde_json::json!({"discovery_id":id})).execute(&mut *tx).await.map_err(internal)?;
    let row=sqlx::query_as(concat!("INSERT INTO backlink_discoveries(id,project_id,domain,job_id,config) VALUES($1,$2,$3,$4,$5) RETURNING ",cols!())).bind(id).bind(pid).bind(expected).bind(job).bind(sqlx::types::Json(config)).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn any(pool: &PgPool, id: Uuid) -> AppResult<BacklinkDiscovery> {
    sqlx::query_as(concat!(
        "SELECT ",
        cols!(),
        " FROM backlink_discoveries WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("discovery"))
}
pub async fn status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    snapshot: Option<Uuid>,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE backlink_discoveries SET status=$2,snapshot_id=$3,error=$4 WHERE id=$1")
        .bind(id)
        .bind(status)
        .bind(snapshot)
        .bind(error)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
pub async fn previous(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    domain: &str,
) -> AppResult<Vec<crate::backlinks::BacklinkEvidence>> {
    let id:Option<Uuid>=sqlx::query_scalar("SELECT s.id FROM backlink_snapshots s JOIN projects p ON p.id=s.project_id WHERE p.organization_id=$1 AND p.id=$2 AND p.domain::text=$3 AND s.domain=$3 AND s.source='serp_crawl' AND s.dataset='crawl_v1' ORDER BY s.created_at DESC,s.id DESC LIMIT 1").bind(org).bind(pid).bind(domain).fetch_optional(pool).await.map_err(internal)?;
    if let Some(id) = id {
        Ok(crate::backlinks::get(pool, org, pid, id)
            .await?
            .1
            .into_iter()
            .filter(|r| r.state != "lost")
            .collect())
    } else {
        Ok(vec![])
    }
}
