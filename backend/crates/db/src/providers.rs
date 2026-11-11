//! Provider settings (per organization), the shared provider response cache and the
//! instance-wide domain authority table.

use crate::internal;
use chrono::{DateTime, Utc};
use common::AppResult;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderSettingRow {
    pub provider: String,
    pub enabled: Option<bool>,
    pub secret_enc: Option<Vec<u8>>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_settings(pool: &PgPool, org_id: Uuid) -> AppResult<Vec<ProviderSettingRow>> {
    sqlx::query_as(
        "SELECT provider, enabled, secret_enc, updated_at FROM provider_settings
         WHERE organization_id = $1",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// What to do with the stored secret on update.
pub enum SecretChange {
    Keep,
    Clear,
    Set(Vec<u8>),
}

pub async fn upsert_setting(
    pool: &PgPool,
    org_id: Uuid,
    provider: &str,
    enabled: Option<bool>,
    secret: SecretChange,
    updated_by: Uuid,
) -> AppResult<()> {
    let (touch_secret, secret_enc) = match secret {
        SecretChange::Keep => (false, None),
        SecretChange::Clear => (true, None),
        SecretChange::Set(v) => (true, Some(v)),
    };
    sqlx::query(
        "INSERT INTO provider_settings (organization_id, provider, enabled, secret_enc, updated_by)
         VALUES ($1, $2, $3, $5, $6)
         ON CONFLICT (organization_id, provider) DO UPDATE SET
            enabled = COALESCE($3, provider_settings.enabled),
            secret_enc = CASE WHEN $4 THEN $5 ELSE provider_settings.secret_enc END,
            updated_by = $6, updated_at = now()",
    )
    .bind(org_id)
    .bind(provider)
    .bind(enabled)
    .bind(touch_secret)
    .bind(secret_enc)
    .bind(updated_by)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ---------------------------------------------------------------- response cache

/// Cached payload if it is younger than `max_age_days`.
pub async fn cache_get(
    pool: &PgPool,
    kind: &str,
    key: &str,
    max_age_days: i32,
) -> AppResult<Option<serde_json::Value>> {
    sqlx::query_scalar(
        "SELECT payload FROM provider_cache
         WHERE kind = $1 AND cache_key = $2 AND fetched_at > now() - make_interval(days => $3)",
    )
    .bind(kind)
    .bind(key)
    .bind(max_age_days)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub async fn cache_put(
    pool: &PgPool,
    kind: &str,
    key: &str,
    payload: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO provider_cache (kind, cache_key, payload) VALUES ($1, $2, $3)
         ON CONFLICT (kind, cache_key) DO UPDATE SET payload = EXCLUDED.payload, fetched_at = now()",
    )
    .bind(kind)
    .bind(key)
    .bind(payload)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn cache_prune(pool: &PgPool, older_than_days: i32) -> AppResult<u64> {
    let r = sqlx::query(
        "DELETE FROM provider_cache WHERE fetched_at < now() - make_interval(days => $1)",
    )
    .bind(older_than_days)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(r.rows_affected())
}

// ---------------------------------------------------------------- domain ranks

pub struct NewDomainRank<'a> {
    pub domain: &'a str,
    pub harmonic_rank: i64,
    pub pagerank_rank: i64,
    pub sr: i16,
}

pub async fn upsert_domain_ranks(
    pool: &PgPool,
    release: &str,
    rows: &[NewDomainRank<'_>],
) -> AppResult<()> {
    for chunk in rows.chunks(2000) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO domain_ranks (domain, harmonic_rank, pagerank_rank, sr, release) ",
        );
        qb.push_values(chunk, |mut b, r| {
            b.push_bind(r.domain)
                .push_bind(r.harmonic_rank)
                .push_bind(r.pagerank_rank)
                .push_bind(r.sr)
                .push_bind(release);
        });
        qb.push(
            " ON CONFLICT (domain) DO UPDATE SET harmonic_rank = EXCLUDED.harmonic_rank,
                pagerank_rank = EXCLUDED.pagerank_rank, sr = EXCLUDED.sr, release = EXCLUDED.release",
        );
        qb.build().execute(pool).await.map_err(internal)?;
    }
    Ok(())
}

/// Drop rows left over from older releases once a new import has finished.
pub async fn delete_other_releases(pool: &PgPool, release: &str) -> AppResult<u64> {
    let r = sqlx::query("DELETE FROM domain_ranks WHERE release <> $1")
        .bind(release)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(r.rows_affected())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DomainRank {
    pub domain: String,
    pub harmonic_rank: i64,
    pub pagerank_rank: i64,
    pub sr: i16,
    pub release: String,
}

pub async fn domain_ranks(pool: &PgPool, domains: &[String]) -> AppResult<Vec<DomainRank>> {
    if domains.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "SELECT domain, harmonic_rank, pagerank_rank, sr, release FROM domain_ranks
         WHERE domain = ANY($1)",
    )
    .bind(domains)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// `(rows, weakest SR)` of the imported list; `None` when nothing is imported yet.
pub async fn domain_rank_stats(pool: &PgPool) -> AppResult<Option<(i64, i16)>> {
    let row: (i64, Option<i16>) = sqlx::query_as("SELECT count(*), min(sr) FROM domain_ranks")
        .fetch_one(pool)
        .await
        .map_err(internal)?;
    Ok(row.1.map(|min| (row.0, min)))
}
