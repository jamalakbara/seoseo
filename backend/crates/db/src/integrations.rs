use crate::internal;
use common::{AppError, AppResult, new_id, token};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct IndexNowSettings {
    pub domain: String,
    pub key: String,
    pub last_submitted_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub async fn indexnow(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<IndexNowSettings> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let domain:Option<String>=sqlx::query_scalar("SELECT domain::text FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE").bind(org).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let domain =
        domain.ok_or_else(|| AppError::Validation("Set the project website first".into()))?;
    let key = token::sha256_hex(&token::random_token(32))[..32].to_string();
    let row=sqlx::query_as("INSERT INTO indexnow_settings(project_id,domain,key) VALUES($1,$2,$3) ON CONFLICT(project_id) DO UPDATE SET domain=excluded.domain,key=CASE WHEN indexnow_settings.domain=excluded.domain THEN indexnow_settings.key ELSE excluded.key END,last_submitted_at=CASE WHEN indexnow_settings.domain=excluded.domain THEN indexnow_settings.last_submitted_at END RETURNING domain,key,last_submitted_at").bind(pid).bind(domain).bind(key).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn claim_indexnow(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    domain: &str,
    key: &str,
) -> AppResult<()> {
    let r=sqlx::query("UPDATE indexnow_settings s SET last_submitted_at=now() FROM projects p WHERE s.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND p.archived_at IS NULL AND p.domain::text=$3 AND s.domain=$3 AND s.key=$4 AND (s.last_submitted_at IS NULL OR s.last_submitted_at<now()-interval '1 minute')").bind(org).bind(pid).bind(domain).bind(key).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Website changed or IndexNow is cooling down; wait one minute and reload".into(),
        ));
    }
    Ok(())
}
#[derive(sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct LinkObservation {
    pub id: Uuid,
    pub source_url: String,
    pub target_url: String,
    pub state: String,
    pub observed_at: chrono::DateTime<chrono::Utc>,
}
pub async fn links(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<LinkObservation>> {
    sqlx::query_as("SELECT l.id,l.source_url,l.target_url,l.state,l.observed_at FROM link_observations l JOIN projects p ON p.id=l.project_id WHERE p.organization_id=$1 AND p.id=$2 AND (lower(split_part(split_part(split_part(l.target_url,'://',2),'/',1),':',1))=p.domain::text OR lower(split_part(split_part(split_part(l.target_url,'://',2),'/',1),':',1)) LIKE '%.'||p.domain::text) ORDER BY l.observed_at DESC LIMIT 100").bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}
pub async fn save_link(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    source: &str,
    target: &str,
    state: &str,
    expected_domain: &str,
) -> AppResult<LinkObservation> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL AND domain::text=$3 FOR UPDATE").bind(org).bind(pid).bind(expected_domain).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM link_observations WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM link_observations WHERE project_id=$1 AND source_url=$2 AND target_url=$3)").bind(pid).bind(source).bind(target).fetch_one(&mut *tx).await.map_err(internal)?;
    if !exists && count >= 10000 {
        return Err(AppError::Conflict(
            "Project link-observation limit reached".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO link_observations(id,project_id,source_url,target_url,state) VALUES($1,$2,$3,$4,$5) ON CONFLICT(project_id,source_url,target_url) DO UPDATE SET state=excluded.state,observed_at=CASE WHEN link_observations.state IS DISTINCT FROM excluded.state THEN now() ELSE link_observations.observed_at END RETURNING id,source_url,target_url,state,observed_at").bind(new_id()).bind(pid).bind(source).bind(target).bind(state).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
