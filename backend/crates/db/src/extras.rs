//! M8 repositories. Domain services authorize project reads and writes first.
use crate::internal;
use common::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn public_budget(pool: &PgPool, network: bool) -> AppResult<bool> {
    sqlx::query_scalar("INSERT INTO public_tool_limits(bucket,requests) VALUES($1,1) ON CONFLICT(bucket) DO UPDATE SET requests=CASE WHEN public_tool_limits.window_at<now()-interval '1 minute' THEN 1 ELSE public_tool_limits.requests+1 END,window_at=CASE WHEN public_tool_limits.window_at<now()-interval '1 minute' THEN now() ELSE public_tool_limits.window_at END RETURNING requests <= $2")
        .bind(if network { "network" } else { "analysis" }).bind(if network { 10 } else { 120 }).fetch_one(pool).await.map_err(internal)
}

#[derive(Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct LogAnalysis {
    pub id: Uuid,
    pub project_id: Uuid,
    pub summary: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
pub async fn save_log(pool: &PgPool, pid: Uuid, summary: Value) -> AppResult<LogAnalysis> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(pid)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let row = sqlx::query_as(
        "INSERT INTO log_analyses(id,project_id,summary) VALUES($1,$2,$3) RETURNING *",
    )
    .bind(common::new_id())
    .bind(pid)
    .bind(summary)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    sqlx::query("DELETE FROM log_analyses WHERE project_id=$1 AND id NOT IN(SELECT id FROM log_analyses WHERE project_id=$1 ORDER BY created_at DESC,id DESC LIMIT 20)").bind(pid).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn logs(pool: &PgPool, pid: Uuid) -> AppResult<Vec<LogAnalysis>> {
    sqlx::query_as(
        "SELECT * FROM log_analyses WHERE project_id=$1 ORDER BY created_at DESC,id DESC LIMIT 20",
    )
    .bind(pid)
    .fetch_all(pool)
    .await
    .map_err(internal)
}
pub async fn delete_log(pool: &PgPool, pid: Uuid, id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM log_analyses WHERE project_id=$1 AND id=$2")
        .bind(pid)
        .bind(id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
#[derive(sqlx::FromRow)]
pub struct Credential {
    pub host: String,
    pub secret: Vec<u8>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
pub async fn credential(pool: &PgPool, pid: Uuid) -> AppResult<Option<Credential>> {
    sqlx::query_as("SELECT host,secret,expires_at FROM crawler_credentials WHERE project_id=$1 AND expires_at>now()").bind(pid).fetch_optional(pool).await.map_err(internal)
}
pub async fn set_credential(
    pool: &PgPool,
    pid: Uuid,
    host: &str,
    secret: Vec<u8>,
    expires: chrono::DateTime<chrono::Utc>,
) -> AppResult<()> {
    sqlx::query("INSERT INTO crawler_credentials(project_id,host,secret,expires_at) VALUES($1,$2,$3,$4) ON CONFLICT(project_id) DO UPDATE SET host=$2,secret=$3,expires_at=$4,updated_at=now()").bind(pid).bind(host).bind(secret).bind(expires).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn delete_credential(pool: &PgPool, pid: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM crawler_credentials WHERE project_id=$1")
        .bind(pid)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
pub struct Movement {
    pub previous: Option<i32>,
    pub current: Option<i32>,
    pub depth: i32,
    pub comparable: bool,
}

#[derive(sqlx::FromRow)]
pub struct SerpPair {
    pub keyword: String,
    pub current_id: Option<Uuid>,
    pub previous_id: Option<Uuid>,
    pub current_urls: Vec<String>,
    pub previous_urls: Vec<String>,
}
pub async fn serp_pairs(pool: &PgPool, pid: Uuid) -> AppResult<Vec<SerpPair>> {
    sqlx::query_as("WITH keywords AS (SELECT DISTINCT k.keyword FROM saved_keywords k JOIN projects p ON p.id=k.project_id WHERE p.id=$1 AND k.country=p.country_code AND k.language=p.language_code ORDER BY k.keyword LIMIT 500) SELECT k.keyword,a.id AS current_id,b.id AS previous_id,ARRAY(SELECT DISTINCT url FROM serp_items WHERE snapshot_id=a.id AND position BETWEEN 1 AND 10 ORDER BY url) AS current_urls,ARRAY(SELECT DISTINCT url FROM serp_items WHERE snapshot_id=b.id AND position BETWEEN 1 AND 10 ORDER BY url) AS previous_urls FROM keywords k JOIN projects p ON p.id=$1 LEFT JOIN LATERAL(SELECT id,provider,exact,fetched_at FROM serp_snapshots WHERE keyword=k.keyword AND country=p.country_code AND language=p.language_code AND exact AND fetched_at>now()-interval '30 days' ORDER BY fetched_at DESC,id DESC LIMIT 1) a ON true LEFT JOIN LATERAL(SELECT id FROM serp_snapshots WHERE keyword=k.keyword AND country=p.country_code AND language=p.language_code AND provider=a.provider AND exact=a.exact AND (fetched_at,id)<(a.fetched_at,a.id) AND fetched_at>a.fetched_at-interval '30 days' ORDER BY fetched_at DESC,id DESC LIMIT 1) b ON true ORDER BY k.keyword").bind(pid).fetch_all(pool).await.map_err(internal)
}
/// Latest completed pair per tracker, capped to ten trackers / 500 keywords each.
pub async fn movements(pool: &PgPool, pid: Uuid) -> AppResult<Vec<Movement>> {
    sqlx::query_as("WITH pairs AS (SELECT t.id,r.id AS current,b.id AS previous FROM rank_trackers t CROSS JOIN LATERAL(SELECT id,created_at,finished_at FROM rank_runs WHERE tracker_id=t.id AND status='completed' ORDER BY created_at DESC,id DESC LIMIT 1) r LEFT JOIN LATERAL(SELECT id FROM rank_runs WHERE tracker_id=t.id AND status='completed' AND (created_at,id)<(r.created_at,r.id) AND finished_at<=r.finished_at ORDER BY created_at DESC,id DESC LIMIT 1) b ON true JOIN projects p ON p.id=t.project_id WHERE t.project_id=$1 AND t.domain=p.domain::text AND t.country=p.country_code AND t.language=p.language_code ORDER BY t.id LIMIT 10) SELECT b.position AS previous,a.position AS current,COALESCE(a.depth,b.depth,10) AS depth,COALESCE(a.provider=b.provider AND a.exact=b.exact AND a.depth=b.depth AND a.run_id IS NOT NULL AND b.run_id IS NOT NULL,false) AS comparable FROM pairs p JOIN rank_tracker_keywords k ON k.tracker_id=p.id LEFT JOIN rank_results a ON a.run_id=p.current AND a.keyword_id=k.id LEFT JOIN rank_results b ON b.run_id=p.previous AND b.keyword_id=k.id ORDER BY p.id,k.id LIMIT 5000").bind(pid).fetch_all(pool).await.map_err(internal)
}
