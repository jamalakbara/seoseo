use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

// Includes the grant timestamp so reconnecting the same property invalidates old data.
macro_rules! identity_sql {
    () => {
        "jsonb_build_array(p.domain,p.country_code,p.language_code,c.site_url,c.updated_at)::text"
    };
}

#[derive(FromRow)]
pub struct Source {
    pub identity: String,
    pub site_url: Option<String>,
}
pub async fn source(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Source> {
    sqlx::query_as(concat!("SELECT ",identity_sql!()," AS identity,c.site_url FROM projects p LEFT JOIN gsc_connections c ON c.project_id=p.id WHERE p.organization_id=$1 AND p.id=$2"))
        .bind(org).bind(pid).fetch_optional(pool).await.map_err(internal)?.ok_or(AppError::NotFound("project"))
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct Snapshot {
    pub data: Option<Value>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
    pub job_id: Option<Uuid>,
    pub refreshing: bool,
    pub error: Option<String>,
}
pub async fn snapshot(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Option<Snapshot>> {
    sqlx::query_as(concat!("SELECT s.data,s.fetched_at,s.next_attempt_at,s.job_id,COALESCE(j.status IN ('queued','running'),false) AS refreshing,COALESCE(s.error,CASE WHEN j.status='failed' THEN 'Refresh worker failed; try again after the retry time' END) AS error FROM dashboard_snapshots s JOIN projects p ON p.id=s.project_id LEFT JOIN gsc_connections c ON c.project_id=p.id LEFT JOIN jobs j ON j.id=s.job_id WHERE p.organization_id=$1 AND p.id=$2 AND s.identity=",identity_sql!()))
        .bind(org).bind(pid).fetch_optional(pool).await.map_err(internal)
}

/// Atomically claim and enqueue; concurrent visits share the same queued/running job.
pub async fn enqueue_refresh(pool: &PgPool, org: Uuid, pid: Uuid, user: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let source: Source = sqlx::query_as(concat!("SELECT ",identity_sql!()," AS identity,c.site_url FROM projects p LEFT JOIN gsc_connections c ON c.project_id=p.id WHERE p.organization_id=$1 AND p.id=$2 AND p.archived_at IS NULL FOR UPDATE OF p"))
        .bind(org).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if source.site_url.is_none() {
        return Err(AppError::Conflict(
            "Connect and select a Search Console property first".into(),
        ));
    }
    let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM dashboard_snapshots s LEFT JOIN jobs j ON j.id=s.job_id WHERE s.project_id=$1 AND s.identity=$2 AND (s.next_attempt_at>now() OR j.status IN ('queued','running')))")
        .bind(pid).bind(&source.identity).fetch_one(&mut *tx).await.map_err(internal)?;
    if !blocked {
        let job = new_id();
        sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'dashboard_refresh',$2,$3,$4,1)")
            .bind(job).bind(org).bind(pid).bind(json!({"identity":source.identity,"user_id":user})).execute(&mut *tx).await.map_err(internal)?;
        sqlx::query("INSERT INTO dashboard_snapshots(project_id,identity,job_id,next_attempt_at) VALUES($1,$2,$3,now()+interval '15 minutes') ON CONFLICT(project_id) DO UPDATE SET identity=excluded.identity,job_id=excluded.job_id,next_attempt_at=excluded.next_attempt_at,error=NULL,data=CASE WHEN dashboard_snapshots.identity=excluded.identity THEN dashboard_snapshots.data END,fetched_at=CASE WHEN dashboard_snapshots.identity=excluded.identity THEN dashboard_snapshots.fetched_at END")
            .bind(pid).bind(&source.identity).bind(job).execute(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)
}

pub async fn finish_refresh(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    job: Uuid,
    identity: &str,
    result: Result<Value, String>,
) -> AppResult<()> {
    let (data, error) = match result {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    // A result from an old site, market, grant or job must never replace current data.
    sqlx::query(concat!("UPDATE dashboard_snapshots s SET data=COALESCE($5,s.data),fetched_at=CASE WHEN $5::jsonb IS NOT NULL THEN now() ELSE s.fetched_at END,error=$6,next_attempt_at=now()+CASE WHEN $5::jsonb IS NOT NULL THEN interval '24 hours' ELSE interval '15 minutes' END FROM projects p LEFT JOIN gsc_connections c ON c.project_id=p.id WHERE s.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND s.job_id=$3 AND s.identity=$4 AND s.identity=",identity_sql!()))
        .bind(org).bind(pid).bind(job).bind(identity).bind(data).bind(error).execute(pool).await.map_err(internal)?;
    Ok(())
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct Counts {
    pub saved_keywords: i64,
    pub trackers: i64,
    pub reports: i64,
    pub memory_ready: bool,
}
pub async fn counts(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Counts> {
    sqlx::query_as("SELECT (SELECT count(*) FROM saved_keywords k WHERE k.project_id=p.id) AS saved_keywords,(SELECT count(*) FROM rank_trackers t WHERE t.project_id=p.id AND t.domain=p.domain::text AND t.country=p.country_code AND t.language=p.language_code) AS trackers,(SELECT count(*) FROM reports d WHERE d.project_id=p.id) AS reports,COALESCE((SELECT EXISTS(SELECT 1 FROM jsonb_array_elements(c.sections) s WHERE s->>'key'='business_overview' AND length(trim(s->>'content'))>0) FROM project_contexts c WHERE c.project_id=p.id),false) AS memory_ready FROM projects p WHERE p.organization_id=$1 AND p.id=$2")
        .bind(org).bind(pid).fetch_one(pool).await.map_err(internal)
}

pub async fn skipped(pool: &PgPool, org: Uuid, pid: Uuid, user: Uuid) -> AppResult<Vec<String>> {
    sqlx::query_scalar("SELECT d.step FROM dashboard_preferences d JOIN projects p ON p.id=d.project_id WHERE p.organization_id=$1 AND p.id=$2 AND d.user_id=$3 ORDER BY d.step")
        .bind(org).bind(pid).bind(user).fetch_all(pool).await.map_err(internal)
}
pub async fn skip(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
    step: &str,
    skip: bool,
) -> AppResult<()> {
    if skip {
        sqlx::query("INSERT INTO dashboard_preferences(project_id,user_id,step) SELECT id,$3,$4 FROM projects WHERE organization_id=$1 AND id=$2 ON CONFLICT DO NOTHING")
            .bind(org).bind(pid).bind(user).bind(step).execute(pool).await.map_err(internal)?;
    } else {
        sqlx::query("DELETE FROM dashboard_preferences d USING projects p WHERE d.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND d.user_id=$3 AND d.step=$4")
            .bind(org).bind(pid).bind(user).bind(step).execute(pool).await.map_err(internal)?;
    }
    Ok(())
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct RankMover {
    pub tracker_id: Uuid,
    pub keyword: String,
    pub position: i32,
    pub previous_position: i32,
    pub change: i32,
    pub provider: String,
    pub exact: bool,
    pub checked_at: DateTime<Utc>,
}
pub async fn movers(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<RankMover>> {
    sqlx::query_as("WITH history AS (SELECT t.id AS tracker_id,k.keyword,r.position,r.provider,r.exact,r.checked_at,r.depth,lead(r.position) OVER w AS previous_position,lead(r.provider) OVER w AS previous_provider,lead(r.depth) OVER w AS previous_depth,row_number() OVER w AS n FROM rank_results r JOIN rank_runs run ON run.id=r.run_id JOIN rank_tracker_keywords k ON k.id=r.keyword_id JOIN rank_trackers t ON t.id=k.tracker_id JOIN projects p ON p.id=t.project_id WHERE p.organization_id=$1 AND p.id=$2 AND t.domain=p.domain::text AND t.country=p.country_code AND t.language=p.language_code AND run.status='completed' WINDOW w AS(PARTITION BY k.id ORDER BY r.checked_at DESC,r.run_id DESC)) SELECT tracker_id,keyword,position,previous_position,previous_position-position AS change,provider,exact,checked_at FROM history WHERE n=1 AND position IS NOT NULL AND previous_position IS NOT NULL AND position<>previous_position AND provider=previous_provider AND depth=previous_depth ORDER BY abs(previous_position-position) DESC,keyword LIMIT 10")
        .bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct WebsitePage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub h1: Option<String>,
}
pub async fn website_pages(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    aid: Uuid,
) -> AppResult<Vec<WebsitePage>> {
    sqlx::query_as("SELECT ap.url,ap.title,ap.meta_description AS description,ap.h1 FROM audit_pages ap JOIN audits a ON a.id=ap.audit_id JOIN projects p ON p.id=a.project_id WHERE p.organization_id=$1 AND p.id=$2 AND a.id=$3 AND ap.status_code BETWEEN 200 AND 299 AND ap.title IS NOT NULL ORDER BY ap.depth,ap.url LIMIT 5")
        .bind(org).bind(pid).bind(aid).fetch_all(pool).await.map_err(internal)
}
