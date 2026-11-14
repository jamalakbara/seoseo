use crate::internal;
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LocalScanRun {
    pub id: Uuid,
    pub tracker_id: Uuid,
    pub job_id: Option<Uuid>,
    pub provider: String,
    pub status: String,
    pub config: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

pub struct NewScan {
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub tracker_id: Uuid,
    pub provider: String,
    pub config: Value,
    pub status: String,
    /// An import is persisted atomically; live runs create a durable job.
    pub cells: Option<Vec<Value>>,
}

pub async fn create(pool: &PgPool, input: NewScan) -> AppResult<LocalScanRun> {
    reconcile(pool, input.tracker_id).await?;
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 AND organization_id=$2 FOR UPDATE")
        .bind(input.project_id)
        .bind(input.org_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound("project"))?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_scan_runs WHERE tracker_id=$1 AND status IN ('queued','running'))")
        .bind(input.tracker_id).fetch_one(&mut *tx).await.map_err(internal)?;
    if exists {
        return Err(AppError::Conflict("a local scan is already active".into()));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_scan_runs WHERE tracker_id=$1")
        .bind(input.tracker_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    if count >= 20 {
        return Err(AppError::Conflict(
            "delete an old scan before saving more than 20 per tracker".into(),
        ));
    }
    let id = new_id();
    let job_id = input.cells.is_none().then(new_id);
    if let Some(job) = job_id {
        sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'local_grid_scan',$2,$3,$4,3)")
            .bind(job).bind(input.org_id).bind(input.project_id)
            .bind(serde_json::json!({"run_id":id})).execute(&mut *tx).await.map_err(internal)?;
    }
    sqlx::query("INSERT INTO local_scan_runs(id,tracker_id,job_id,provider,status,config,finished_at) VALUES($1,$2,$3,$4,$5,$6,CASE WHEN $3::uuid IS NULL THEN now() ELSE NULL END)")
        .bind(id).bind(input.tracker_id).bind(job_id).bind(input.provider)
        .bind(input.status).bind(input.config)
        .execute(&mut *tx).await.map_err(internal)?;
    if let Some(cells) = input.cells {
        for c in cells {
            sqlx::query("INSERT INTO local_scan_cells(run_id,keyword,row_index,column_index,data) VALUES($1,$2,$3,$4,$5)")
                .bind(id).bind(c["keyword"].as_str().unwrap_or_default())
                .bind(c["row"].as_i64().unwrap_or_default() as i16)
                .bind(c["column"].as_i64().unwrap_or_default() as i16).bind(c)
                .execute(&mut *tx).await.map_err(internal)?;
        }
    }
    tx.commit().await.map_err(internal)?;
    get_any(pool, id).await
}

pub async fn get_any(pool: &PgPool, id: Uuid) -> AppResult<LocalScanRun> {
    sqlx::query_as("SELECT * FROM local_scan_runs WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound("local scan"))
}
pub async fn get(pool: &PgPool, tracker: Uuid, id: Uuid) -> AppResult<LocalScanRun> {
    reconcile(pool, tracker).await?;
    sqlx::query_as("SELECT * FROM local_scan_runs WHERE tracker_id=$1 AND id=$2")
        .bind(tracker)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound("local scan"))
}
pub async fn list(pool: &PgPool, tracker: Uuid) -> AppResult<Vec<LocalScanRun>> {
    reconcile(pool, tracker).await?;
    sqlx::query_as(
        "SELECT * FROM local_scan_runs WHERE tracker_id=$1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(tracker)
    .fetch_all(pool)
    .await
    .map_err(internal)
}
pub async fn cells(pool: &PgPool, run: Uuid) -> AppResult<Vec<Value>> {
    sqlx::query_scalar(
        "SELECT data FROM local_scan_cells WHERE run_id=$1 ORDER BY keyword,row_index,column_index",
    )
    .bind(run)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

async fn reconcile(pool: &PgPool, tracker: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE local_scan_runs r SET status='failed',error='worker exhausted retries; recorded cells retained',finished_at=now() FROM jobs j WHERE r.tracker_id=$1 AND r.job_id=j.id AND r.status IN ('queued','running') AND j.status='failed'")
        .bind(tracker).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn claim_cell(pool: &PgPool, run: Uuid, data: &Value) -> AppResult<bool> {
    let rows=sqlx::query("INSERT INTO local_scan_cells(run_id,keyword,row_index,column_index,data) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
        .bind(run).bind(data["keyword"].as_str().unwrap_or_default())
        .bind(data["row"].as_i64().unwrap_or_default() as i16)
        .bind(data["column"].as_i64().unwrap_or_default() as i16).bind(data)
        .execute(pool).await.map_err(internal)?.rows_affected();
    Ok(rows == 1)
}
pub async fn put_cell(pool: &PgPool, run: Uuid, data: &Value) -> AppResult<()> {
    sqlx::query("UPDATE local_scan_cells SET data=$5 WHERE run_id=$1 AND keyword=$2 AND row_index=$3 AND column_index=$4")
        .bind(run).bind(data["keyword"].as_str().unwrap_or_default())
        .bind(data["row"].as_i64().unwrap_or_default() as i16)
        .bind(data["column"].as_i64().unwrap_or_default() as i16).bind(data)
        .execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn finish(pool: &PgPool, run: Uuid, status: &str, error: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE local_scan_runs SET status=$2,error=$3,finished_at=CASE WHEN $2='running' THEN NULL ELSE now() END WHERE id=$1")
        .bind(run).bind(status).bind(error).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn delete(pool: &PgPool, tracker: Uuid, run: Uuid) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM local_scan_runs WHERE id=$1 AND tracker_id=$2 AND status NOT IN ('queued','running')")
        .bind(run).bind(tracker).execute(pool).await.map_err(internal)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict(
            "scan is active or no longer exists".into(),
        ));
    }
    Ok(())
}
