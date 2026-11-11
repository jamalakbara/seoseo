//! Instance-wide SERP snapshots. Search results are public data, so snapshots are shared
//! across organizations (cache-first keeps request volume low).

use crate::internal;
use chrono::{DateTime, Utc};
use common::AppResult;
use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Snapshot {
    pub id: Uuid,
    pub provider: String,
    pub exact: bool,
    pub keyword: String,
    pub country: String,
    pub language: String,
    pub features: serde_json::Value,
    pub related: serde_json::Value,
    pub questions: serde_json::Value,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SnapshotItem {
    pub position: i32,
    pub url: String,
    /// Registrable domain of the result.
    pub domain: String,
    pub title: String,
    pub snippet: Option<String>,
}

pub struct NewSnapshot<'a> {
    pub id: Uuid,
    pub provider: &'a str,
    pub exact: bool,
    pub keyword: &'a str,
    pub country: &'a str,
    pub language: &'a str,
    pub features: &'a [String],
    pub related: &'a [String],
    pub questions: &'a [String],
}

pub async fn insert(
    pool: &PgPool,
    snap: &NewSnapshot<'_>,
    items: &[SnapshotItem],
) -> AppResult<Snapshot> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let row: Snapshot = sqlx::query_as(
        "INSERT INTO serp_snapshots (id, provider, exact, keyword, country, language, features, related, questions)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id, provider, exact, keyword, country, language, features, related, questions, fetched_at",
    )
    .bind(snap.id)
    .bind(snap.provider)
    .bind(snap.exact)
    .bind(snap.keyword)
    .bind(snap.country)
    .bind(snap.language)
    .bind(serde_json::json!(snap.features))
    .bind(serde_json::json!(snap.related))
    .bind(serde_json::json!(snap.questions))
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if !items.is_empty() {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO serp_items (snapshot_id, position, url, domain, title, snippet) ",
        );
        qb.push_values(items, |mut b, i| {
            b.push_bind(snap.id)
                .push_bind(i.position)
                .push_bind(&i.url)
                .push_bind(&i.domain)
                .push_bind(&i.title)
                .push_bind(&i.snippet);
        });
        qb.build().execute(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(row)
}

/// Newest snapshot for a keyword and market that is younger than `max_age_days`.
pub async fn latest(
    pool: &PgPool,
    keyword: &str,
    country: &str,
    language: &str,
    max_age_days: i32,
) -> AppResult<Option<Snapshot>> {
    sqlx::query_as(
        "SELECT id, provider, exact, keyword, country, language, features, related, questions, fetched_at
         FROM serp_snapshots
         WHERE keyword = $1 AND country = $2 AND language = $3
           AND fetched_at > now() - make_interval(days => $4)
         ORDER BY exact DESC, fetched_at DESC LIMIT 1",
    )
    .bind(keyword)
    .bind(country)
    .bind(language)
    .bind(max_age_days)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn items(pool: &PgPool, snapshot_id: Uuid) -> AppResult<Vec<SnapshotItem>> {
    sqlx::query_as(
        "SELECT position, url, domain, title, snippet FROM serp_items
         WHERE snapshot_id = $1 ORDER BY position",
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}
