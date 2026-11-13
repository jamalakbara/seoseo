use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! sample_sql{()=>{"WITH selected AS(SELECT id,domain::text AS domain FROM projects WHERE organization_id=$1 AND id=ANY($2)),
 latest AS(SELECT DISTINCT ON(s.project_id,s.dataset,s.source) s.* FROM backlink_snapshots s JOIN selected p ON p.id=s.project_id AND p.domain=s.domain
 ORDER BY s.project_id,s.dataset,s.source,s.created_at DESC,s.id DESC),
 ranked AS(SELECT l.*,row_number() OVER(PARTITION BY project_id ORDER BY created_at DESC,id DESC) AS rn FROM latest l),
 snapshots AS(SELECT * FROM ranked WHERE rn<=10),
 links AS(SELECT s.project_id,s.created_at,l.source_domain,l.source_url,l.target_url,l.verification FROM snapshots s JOIN backlink_snapshot_links l ON l.snapshot_id=s.id WHERE l.state<>'lost') "};}
macro_rules! prospects_sql{()=>{"SELECT source_domain,count(DISTINCT project_id) AS competitor_count,array_agg(DISTINCT project_id ORDER BY project_id) AS competitor_project_ids,
 (array_agg(DISTINCT source_url ORDER BY source_url))[1:3] AS sample_source_urls,
 count(DISTINCT (project_id,source_url,target_url)) AS observed_links,
 count(DISTINCT (project_id,source_url,target_url)) FILTER(WHERE verification='verified') AS verified_links,
 count(DISTINCT (project_id,source_url,target_url)) FILTER(WHERE verification='imported') AS imported_links,
 max(created_at) AS newest_snapshot_at
 FROM links WHERE project_id<>$3 AND source_domain<>$5 AND NOT EXISTS(SELECT 1 FROM links mine WHERE mine.project_id=$3 AND mine.source_domain=links.source_domain)
 GROUP BY source_domain HAVING count(DISTINCT project_id)>=$4 "};}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct LinkIntersectScope {
    pub project_id: Uuid,
    pub domain: String,
    pub available_datasets: i64,
    pub sampled_datasets: i64,
    pub oldest_snapshot_at: Option<DateTime<Utc>>,
    pub newest_snapshot_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct LinkProspect {
    pub source_domain: String,
    pub competitor_count: i64,
    pub competitor_project_ids: Vec<Uuid>,
    pub sample_source_urls: Vec<String>,
    pub observed_links: i64,
    pub verified_links: i64,
    pub imported_links: i64,
    pub newest_snapshot_at: DateTime<Utc>,
}
pub struct IntersectSelection {
    pub org: Uuid,
    pub own: Uuid,
    pub current_root: String,
    pub identities: Vec<(Uuid, String)>,
    pub minimum: i64,
    pub limit: i64,
    pub offset: i64,
}
pub struct IntersectEvidence {
    pub scopes: Vec<LinkIntersectScope>,
    pub total: i64,
    pub items: Vec<LinkProspect>,
}

pub async fn evidence(pool: &PgPool, q: IntersectSelection) -> AppResult<IntersectEvidence> {
    let ids: Vec<_> = q.identities.iter().map(|(id, _)| *id).collect();
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let current: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id,domain::text FROM projects WHERE organization_id=$1 AND id=ANY($2)",
    )
    .bind(q.org)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    if current.len() != ids.len() {
        return Err(AppError::NotFound("project"));
    }
    if q.identities.iter().any(|(id, domain)| {
        !current
            .iter()
            .any(|(cid, cdom)| cid == id && cdom.as_deref() == Some(domain))
    }) {
        return Err(AppError::Conflict(
            "a selected website changed; reload before comparing link samples".into(),
        ));
    }
    let scopes=sqlx::query_as(concat!(sample_sql!(),"SELECT p.id AS project_id,p.domain,(SELECT count(*) FROM latest l WHERE l.project_id=p.id) AS available_datasets,
     count(s.id) AS sampled_datasets,min(s.created_at) AS oldest_snapshot_at,max(s.created_at) AS newest_snapshot_at
     FROM selected p LEFT JOIN snapshots s ON s.project_id=p.id GROUP BY p.id,p.domain ORDER BY p.id"))
        .bind(q.org).bind(&ids).fetch_all(&mut *tx).await.map_err(internal)?;
    let total = sqlx::query_scalar(concat!(
        sample_sql!(),
        "SELECT count(*) FROM (",
        prospects_sql!(),
        ") prospects"
    ))
    .bind(q.org)
    .bind(&ids)
    .bind(q.own)
    .bind(q.minimum)
    .bind(&q.current_root)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    let items = sqlx::query_as(concat!(
        sample_sql!(),
        prospects_sql!(),
        "ORDER BY competitor_count DESC,source_domain LIMIT $6 OFFSET $7"
    ))
    .bind(q.org)
    .bind(&ids)
    .bind(q.own)
    .bind(q.minimum)
    .bind(&q.current_root)
    .bind(q.limit)
    .bind(q.offset)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(IntersectEvidence {
        scopes,
        total,
        items,
    })
}
