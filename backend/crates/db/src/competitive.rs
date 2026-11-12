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
        ), bounded AS (
            SELECT keyword, count(*) OVER () AS scope_total FROM scope ORDER BY keyword LIMIT 500
        )
        SELECT b.keyword,b.scope_total,s.provider,s.exact,s.fetched_at,i.domain,i.position,i.url
        FROM bounded b
        LEFT JOIN LATERAL (
            SELECT id,provider,exact,fetched_at FROM serp_snapshots
            WHERE keyword=b.keyword AND country=$3 AND language=$4
              AND fetched_at > now() - interval '30 days'
            ORDER BY fetched_at DESC,id DESC LIMIT 1
        ) s ON true
        LEFT JOIN serp_items i ON i.snapshot_id=s.id AND i.position BETWEEN 1 AND 30
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
