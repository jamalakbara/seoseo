//! Tenant-scoped local business tracker persistence.

use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LocalTracker {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub business_source: String,
    pub business_id: String,
    pub business_name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub category: Option<String>,
    pub grid_size: i16,
    pub spacing_km: f64,
    pub language: String,
    pub created_at: DateTime<Utc>,
    pub keyword_count: i64,
}

pub struct NewLocalTracker<'a> {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: &'a str,
    pub business_source: &'a str,
    pub business_id: &'a str,
    pub business_name: &'a str,
    pub address: &'a str,
    pub latitude: f64,
    pub longitude: f64,
    pub category: Option<&'a str>,
    pub grid_size: i16,
    pub spacing_km: f64,
    pub language: &'a str,
    pub created_by: Uuid,
}

macro_rules! columns {
    () => {
        "t.id,t.project_id,t.name,t.business_source,t.business_id,t.business_name,t.address,\
         t.latitude,t.longitude,t.category,t.grid_size,t.spacing_km,t.language,t.created_at,\
         (SELECT count(*) FROM local_tracker_keywords k WHERE k.tracker_id=t.id) AS keyword_count"
    };
}

pub async fn insert(
    pool: &PgPool,
    t: &NewLocalTracker<'_>,
    keywords: &[String],
    max_trackers: i64,
) -> AppResult<LocalTracker> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(t.project_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_trackers WHERE project_id=$1")
        .bind(t.project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    if count >= max_trackers {
        return Err(AppError::Conflict(format!(
            "a project can have {max_trackers} local trackers"
        )));
    }
    sqlx::query("INSERT INTO local_trackers(id,project_id,name,business_source,business_id,business_name,address,latitude,longitude,category,grid_size,spacing_km,language,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(t.id).bind(t.project_id).bind(t.name).bind(t.business_source).bind(t.business_id)
        .bind(t.business_name).bind(t.address).bind(t.latitude).bind(t.longitude).bind(t.category)
        .bind(t.grid_size).bind(t.spacing_km).bind(t.language).bind(t.created_by)
        .execute(&mut *tx).await.map_err(|e| crate::map_err(e, "a local tracker with that name already exists"))?;
    let mut q: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO local_tracker_keywords(tracker_id,keyword) ");
    q.push_values(keywords, |mut b, keyword| {
        b.push_bind(t.id).push_bind(keyword);
    });
    q.build().execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    get(pool, t.project_id, t.id).await
}

pub async fn list(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<LocalTracker>> {
    sqlx::query_as::<_, LocalTracker>(concat!(
        "SELECT ",
        columns!(),
        " FROM local_trackers t WHERE t.project_id=$1 ORDER BY t.created_at"
    ))
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn get(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<LocalTracker> {
    sqlx::query_as::<_, LocalTracker>(concat!(
        "SELECT ",
        columns!(),
        " FROM local_trackers t WHERE t.project_id=$1 AND t.id=$2"
    ))
    .bind(project_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("local tracker"))
}

pub async fn keywords(pool: &PgPool, tracker_id: Uuid) -> AppResult<Vec<String>> {
    sqlx::query_scalar("SELECT keyword::text FROM local_tracker_keywords WHERE tracker_id=$1 ORDER BY created_at,keyword")
        .bind(tracker_id).fetch_all(pool).await.map_err(internal)
}
