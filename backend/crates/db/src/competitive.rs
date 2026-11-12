//! Project keyword scope over the shared, public SERP cache. Never exposes another
//! project's keyword list. Reads only; no provider requests or inferred traffic.
use crate::internal;
use chrono::{DateTime, Utc};
use common::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct Evidence {
    pub keyword: String,
    pub scope_total: i64,
    pub provider: Option<String>,
    pub exact: Option<bool>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub domain: Option<String>,
    pub position: Option<i32>,
    pub url: Option<String>,
    pub monthly_volume: Option<i64>,
    pub volume_source: Option<String>,
}

pub async fn evidence(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    country: &str,
    language: &str,
) -> AppResult<Vec<Evidence>> {
    sqlx::query_as(
        "WITH scope AS (
            SELECT k.keyword FROM saved_keywords k
            JOIN projects p ON p.id=k.project_id
            WHERE p.organization_id=$1 AND p.id=$2 AND k.country=$3 AND k.language=$4
            UNION
            SELECT i.keyword FROM keyword_ideas i
            JOIN keyword_researches r ON r.id=i.research_id
            JOIN projects p ON p.id=r.project_id
            WHERE p.organization_id=$1 AND p.id=$2 AND r.country=$3 AND r.language=$4
            UNION
            SELECT k.keyword FROM rank_tracker_keywords k JOIN rank_trackers t ON t.id=k.tracker_id JOIN projects p ON p.id=t.project_id
            WHERE p.organization_id=$1 AND p.id=$2 AND t.country=$3 AND t.language=$4
            UNION
            SELECT v.keyword FROM competitive_volumes v JOIN projects p ON p.id=v.project_id
            WHERE p.organization_id=$1 AND p.id=$2 AND v.country=$3 AND v.language=$4
        ), bounded AS (
            SELECT keyword, count(*) OVER () AS scope_total FROM scope ORDER BY keyword LIMIT 500
        )
        SELECT b.keyword,b.scope_total,s.provider,s.exact,s.fetched_at,i.domain,i.position,i.url,v.monthly_volume,v.source_label AS volume_source
        FROM bounded b
        LEFT JOIN LATERAL (
            SELECT id,provider,exact,fetched_at FROM serp_snapshots
            WHERE keyword=b.keyword AND country=$3 AND language=$4
              AND fetched_at > now() - interval '30 days'
            ORDER BY fetched_at DESC,id DESC LIMIT 1
        ) s ON true
        LEFT JOIN serp_items i ON i.snapshot_id=s.id AND i.position BETWEEN 1 AND 30
        LEFT JOIN competitive_volumes v ON v.project_id=$2 AND v.keyword=b.keyword AND v.country=$3 AND v.language=$4 AND v.updated_at>now()-interval '90 days'
        ORDER BY b.keyword,i.position",
    )
    .bind(org_id)
    .bind(project_id)
    .bind(country)
    .bind(language)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub struct VolumeInput {
    pub keyword: String,
    pub volume: i64,
}
pub async fn import_volumes(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    country: &str,
    language: &str,
    source: &str,
    rows: &[VolumeInput],
) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE").bind(org).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(common::AppError::NotFound("active project"))?;
    let existing: i64 =
        sqlx::query_scalar("SELECT count(*) FROM competitive_volumes WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    let keywords: Vec<_> = rows.iter().map(|r| r.keyword.clone()).collect();
    let known:i64=sqlx::query_scalar("SELECT count(*) FROM competitive_volumes WHERE project_id=$1 AND country=$2 AND language=$3 AND keyword=ANY($4)").bind(pid).bind(country).bind(language).bind(&keywords).fetch_one(&mut *tx).await.map_err(internal)?;
    if existing + rows.len() as i64 - known > 10000 {
        return Err(common::AppError::Conflict(
            "At most 10000 imported volume rows per project".into(),
        ));
    }
    for row in rows {
        sqlx::query("INSERT INTO competitive_volumes(project_id,keyword,country,language,monthly_volume,source_label) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(project_id,keyword,country,language) DO UPDATE SET monthly_volume=excluded.monthly_volume,source_label=excluded.source_label,updated_at=clock_timestamp()")
        .bind(pid).bind(&row.keyword).bind(country).bind(language).bind(row.volume).bind(source).execute(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(())
}
