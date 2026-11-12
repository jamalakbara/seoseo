use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiTrackerConfig {
    pub brands: Vec<String>,
    pub prompts: Vec<String>,
    pub models: Vec<String>,
    pub country: String,
    pub language: String,
}
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiVisibilityTracker {
    pub id: Uuid,
    pub project_id: Uuid,
    pub domain: String,
    pub name: String,
    #[sqlx(json)]
    pub config: AiTrackerConfig,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiVisibilityRun {
    pub id: Uuid,
    pub tracker_id: Uuid,
    pub job_id: Option<Uuid>,
    #[sqlx(json)]
    pub config: AiTrackerConfig,
    pub gateway_id: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AiAnswerRow {
    pub prompt_index: i32,
    pub model: String,
    pub answer: Option<String>,
    pub mentions: Option<serde_json::Value>,
    pub sources: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}
macro_rules! tracker_cols {
    () => {
        "id,project_id,domain,name,config,created_at"
    };
}
macro_rules! run_cols {
    () => {
        "id,tracker_id,job_id,config,gateway_id,status,error,created_at,finished_at"
    };
}

async fn project_lock(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    org: Uuid,
    pid: Uuid,
) -> AppResult<crate::models::Project> {
    let p: crate::models::Project =
        sqlx::query_as("SELECT * FROM projects WHERE organization_id=$1 AND id=$2 FOR UPDATE")
            .bind(org)
            .bind(pid)
            .fetch_optional(&mut **tx)
            .await
            .map_err(internal)?
            .ok_or(AppError::NotFound("project"))?;
    if p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "Restore the project before changing AI trackers".into(),
        ));
    }
    Ok(p)
}

pub async fn create(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    domain: &str,
    name: &str,
    config: &AiTrackerConfig,
) -> AppResult<AiVisibilityTracker> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p = project_lock(&mut tx, org, pid).await?;
    if p.domain.as_deref() != Some(domain) {
        return Err(AppError::Conflict(
            "Website changed; reload before creating a tracker".into(),
        ));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai_visibility_trackers WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if count >= 10 {
        return Err(AppError::Conflict(
            "Limit of 10 AI trackers per project; delete an old tracker first".into(),
        ));
    }
    let row=sqlx::query_as(concat!("INSERT INTO ai_visibility_trackers(id,project_id,domain,name,config) VALUES($1,$2,$3,$4,$5) RETURNING ",tracker_cols!()))
        .bind(new_id()).bind(pid).bind(domain).bind(name).bind(sqlx::types::Json(config)).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn list(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<AiVisibilityTracker>> {
    sqlx::query_as(concat!("SELECT ",tracker_cols!()," FROM ai_visibility_trackers WHERE project_id=$2 AND EXISTS(SELECT 1 FROM projects WHERE organization_id=$1 AND id=$2) ORDER BY created_at DESC,id DESC"))
        .bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}
pub async fn tracker(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    tid: Uuid,
) -> AppResult<AiVisibilityTracker> {
    sqlx::query_as(concat!("SELECT ",tracker_cols!()," FROM ai_visibility_trackers WHERE project_id=$2 AND id=$3 AND domain=(SELECT domain::text FROM projects WHERE organization_id=$1 AND id=$2)"))
        .bind(org).bind(pid).bind(tid).fetch_optional(pool).await.map_err(internal)?.ok_or(AppError::NotFound("AI tracker"))
}

pub async fn reconcile(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE ai_visibility_runs r SET status='failed',error='Background job stopped before the run finished; collected answers are preserved',finished_at=clock_timestamp() FROM ai_visibility_trackers t,projects p WHERE r.tracker_id=t.id AND t.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND r.status IN ('queued','running') AND (r.job_id IS NULL OR EXISTS(SELECT 1 FROM jobs j WHERE j.id=r.job_id AND j.status IN ('failed','cancelled')))")
        .bind(org).bind(pid).execute(pool).await.map_err(internal)?;
    Ok(())
}

pub async fn start(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    tid: Uuid,
    expected_domain: &str,
    gateway_id: &str,
) -> AppResult<AiVisibilityRun> {
    reconcile(pool, org, pid).await?;
    let mut tx = pool.begin().await.map_err(internal)?;
    let p = project_lock(&mut tx, org, pid).await?;
    if p.domain.as_deref() != Some(expected_domain) {
        return Err(AppError::Conflict(
            "Project website changed; reload before running".into(),
        ));
    }
    let t: AiVisibilityTracker = sqlx::query_as(concat!(
        "SELECT ",
        tracker_cols!(),
        " FROM ai_visibility_trackers WHERE project_id=$1 AND id=$2 AND domain=$3"
    ))
    .bind(pid)
    .bind(tid)
    .bind(expected_domain)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("AI tracker"))?;
    let active:i64=sqlx::query_scalar("SELECT count(*) FROM ai_visibility_runs r JOIN ai_visibility_trackers t ON t.id=r.tracker_id WHERE t.project_id=$1 AND r.status IN ('queued','running')").bind(pid).fetch_one(&mut *tx).await.map_err(internal)?;
    let own_active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai_visibility_runs WHERE tracker_id=$1 AND status IN ('queued','running'))").bind(tid).fetch_one(&mut *tx).await.map_err(internal)?;
    if active >= 2 || own_active {
        return Err(AppError::Conflict(
            "An AI run is already active for this tracker, or two are active in this project"
                .into(),
        ));
    }
    let (count,recent):(i64,bool)=sqlx::query_as("SELECT count(*),COALESCE(bool_or(created_at>clock_timestamp()-interval '1 minute'),false) FROM ai_visibility_runs WHERE tracker_id=$1").bind(tid).fetch_one(&mut *tx).await.map_err(internal)?;
    if count >= 20 {
        return Err(AppError::Conflict(
            "Limit of 20 runs per tracker; delete an old run first".into(),
        ));
    }
    if recent {
        return Err(AppError::Conflict(
            "Wait one minute between runs of this tracker".into(),
        ));
    }
    let id = new_id();
    let job_id = new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'ai_visibility',$2,$3,$4,3)")
        .bind(job_id).bind(org).bind(pid).bind(serde_json::json!({"run_id":id})).execute(&mut *tx).await.map_err(internal)?;
    let row=sqlx::query_as(concat!("INSERT INTO ai_visibility_runs(id,tracker_id,job_id,config,gateway_id) VALUES($1,$2,$3,$4,$5) RETURNING ",run_cols!()))
        .bind(id).bind(tid).bind(job_id).bind(sqlx::types::Json(t.config)).bind(gateway_id).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}

pub async fn runs(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    tid: Uuid,
) -> AppResult<Vec<AiVisibilityRun>> {
    reconcile(pool, org, pid).await?;
    tracker(pool, org, pid, tid).await?;
    sqlx::query_as(concat!(
        "SELECT ",
        run_cols!(),
        " FROM ai_visibility_runs WHERE tracker_id=$1 ORDER BY created_at DESC,id DESC LIMIT 20"
    ))
    .bind(tid)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn run_any(pool: &PgPool, id: Uuid) -> AppResult<AiVisibilityRun> {
    sqlx::query_as(concat!(
        "SELECT ",
        run_cols!(),
        " FROM ai_visibility_runs WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("AI run"))
}
pub async fn current(pool: &PgPool, org: Uuid, pid: Uuid, run: &AiVisibilityRun) -> AppResult<()> {
    tracker(pool, org, pid, run.tracker_id).await?;
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL)").bind(org).bind(pid).fetch_one(pool).await.map_err(internal)?;
    if !active {
        return Err(AppError::Conflict(
            "Project was archived while the run was active".into(),
        ));
    }
    Ok(())
}
pub async fn answers(pool: &PgPool, id: Uuid) -> AppResult<Vec<AiAnswerRow>> {
    sqlx::query_as("SELECT prompt_index,model,answer,mentions,sources,error,created_at FROM ai_visibility_answers WHERE run_id=$1 ORDER BY prompt_index,model")
        .bind(id).fetch_all(pool).await.map_err(internal)
}
pub async fn status(pool: &PgPool, id: Uuid, status: &str, error: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE ai_visibility_runs SET status=$2,error=$3,finished_at=CASE WHEN $2 IN ('completed','failed') THEN clock_timestamp() END WHERE id=$1")
        .bind(id).bind(status).bind(error).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub struct NewAiAnswer<'a> {
    pub run_id: Uuid,
    pub prompt_index: i32,
    pub model: &'a str,
    pub answer: Option<&'a str>,
    pub mentions: Option<serde_json::Value>,
    pub sources: Option<serde_json::Value>,
    pub error: Option<&'a str>,
}
pub async fn save_answer(pool: &PgPool, row: NewAiAnswer<'_>) -> AppResult<()> {
    sqlx::query("INSERT INTO ai_visibility_answers(run_id,prompt_index,model,answer,mentions,sources,error) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(run_id,prompt_index,model) DO NOTHING")
        .bind(row.run_id).bind(row.prompt_index).bind(row.model).bind(row.answer).bind(row.mentions).bind(row.sources).bind(row.error).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn delete(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    tid: Uuid,
    rid: Option<Uuid>,
) -> AppResult<()> {
    reconcile(pool, org, pid).await?;
    let mut tx = pool.begin().await.map_err(internal)?;
    project_lock(&mut tx, org, pid).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ai_visibility_trackers WHERE id=$1 AND project_id=$2)",
    )
    .bind(tid)
    .bind(pid)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if !exists {
        return Err(AppError::NotFound("AI tracker"));
    }
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai_visibility_runs WHERE tracker_id=$1 AND status IN ('queued','running'))").bind(tid).fetch_one(&mut *tx).await.map_err(internal)?;
    if active {
        return Err(AppError::Conflict(
            "Wait for active runs to finish before deleting this tracker or its runs".into(),
        ));
    }
    let result = if let Some(rid) = rid {
        sqlx::query("DELETE FROM ai_visibility_runs WHERE id=$1 AND tracker_id=$2 AND EXISTS(SELECT 1 FROM ai_visibility_trackers WHERE id=$2 AND project_id=$3)")
            .bind(rid).bind(tid).bind(pid).execute(&mut *tx).await.map_err(internal)?
    } else {
        sqlx::query("DELETE FROM ai_visibility_trackers WHERE id=$1 AND project_id=$2")
            .bind(tid)
            .bind(pid)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
    };
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("AI tracker or run"));
    }
    tx.commit().await.map_err(internal)?;
    Ok(())
}
