//! SERP analysis shared by the keyword research job and the on-demand API: fetch a results
//! page (cache first), score each ranking domain, compute keyword difficulty and intent.

use crate::provider_ctx::provider_err;
use chrono::{DateTime, Utc};
use common::{AppResult, new_id};
use db::serp::{NewSnapshot, Snapshot, SnapshotItem};
use providers::{Market, Registry, SerpQuery};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// Research SERPs are reused for a week (see docs/05 data freshness).
pub const SERP_TTL_DAYS: i32 = 7;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SerpRow {
    pub position: i32,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub snippet: Option<String>,
    /// SeoSeo Rank of the domain (0–100); null when it is outside the imported list or
    /// no authority data is loaded.
    pub sr: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SerpAnalysis {
    pub snapshot_id: Uuid,
    pub keyword: String,
    pub country: String,
    pub language: String,
    /// Provider that fetched the page, e.g. `serp_searxng`.
    pub provider: String,
    /// True when positions are one search engine's real ranking; false for a meta-search blend.
    pub exact: bool,
    pub fetched_at: DateTime<Utc>,
    /// Served from the shared SERP cache.
    pub cached: bool,
    pub items: Vec<SerpRow>,
    pub features: Vec<String>,
    pub related: Vec<String>,
    pub questions: Vec<String>,
    /// Keyword difficulty 0–100; null without authority data or with fewer than 5 results.
    pub kd: Option<i32>,
    /// How KD was computed, e.g. `serp_searxng+authority_commoncrawl`.
    pub kd_source: Option<String>,
    pub intent: String,
    /// False until the Common Crawl ranks were imported (KD needs them).
    pub authority_loaded: bool,
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// Fetch a fresh results page from the first working provider and store it.
pub(crate) async fn fetch_snapshot(
    pool: &PgPool,
    registry: &Registry,
    keyword: &str,
    market: &Market,
) -> AppResult<(Snapshot, Vec<SnapshotItem>)> {
    let page = registry
        .search(&SerpQuery {
            keyword: keyword.to_string(),
            market: market.clone(),
        })
        .await
        .map_err(provider_err)?;
    let items: Vec<SnapshotItem> = page
        .items
        .into_iter()
        .filter_map(|i| {
            let host = scoring::host_of(&i.url)?;
            Some(SnapshotItem {
                position: i.position,
                domain: scoring::registrable_domain(&host).unwrap_or(host),
                url: i.url,
                title: i.title,
                snippet: i.snippet,
            })
        })
        .collect();
    let snap = db::serp::insert(
        pool,
        &NewSnapshot {
            id: new_id(),
            provider: page.provider,
            exact: page.exact,
            keyword,
            country: &market.country,
            language: &market.language,
            features: &page.features,
            related: &page.related,
            questions: &page.questions,
        },
        &items,
    )
    .await?;
    Ok((snap, items))
}

/// Analyse the SERP of `keyword` (already normalized). Uses a cached snapshot younger than
/// [`SERP_TTL_DAYS`] unless `refresh` is set.
pub async fn analyze(
    pool: &PgPool,
    registry: &Registry,
    keyword: &str,
    market: &Market,
    brand: Option<&str>,
    refresh: bool,
) -> AppResult<SerpAnalysis> {
    let cached = if refresh {
        None
    } else {
        db::serp::latest(
            pool,
            keyword,
            &market.country,
            &market.language,
            SERP_TTL_DAYS,
        )
        .await?
    };
    let (snap, items, was_cached) = match cached {
        Some(s) => {
            let items = db::serp::items(pool, s.id).await?;
            (s, items, true)
        }
        None => {
            let (s, items) = fetch_snapshot(pool, registry, keyword, market).await?;
            (s, items, false)
        }
    };

    let mut domains: Vec<String> = items.iter().map(|i| i.domain.clone()).collect();
    domains.sort_unstable();
    domains.dedup();
    let ranks: HashMap<String, i32> = db::providers::domain_ranks(pool, &domains)
        .await?
        .into_iter()
        .map(|r| (r.domain, i32::from(r.sr)))
        .collect();
    let stats = db::providers::domain_rank_stats(pool).await?;
    let rows: Vec<SerpRow> = items
        .into_iter()
        .map(|i| SerpRow {
            sr: ranks.get(&i.domain).copied(),
            position: i.position,
            url: i.url,
            domain: i.domain,
            title: i.title,
            snippet: i.snippet,
        })
        .collect();
    let kd = stats.and_then(|(_, weakest)| {
        let top: Vec<Option<i32>> = rows.iter().take(10).map(|r| r.sr).collect();
        scoring::keyword_difficulty(&top, scoring::sr_floor(i32::from(weakest)))
    });
    let features = strings(&snap.features);
    Ok(SerpAnalysis {
        snapshot_id: snap.id,
        intent: scoring::classify_intent(keyword, brand, &features)
            .as_str()
            .to_string(),
        kd_source: kd.map(|_| format!("{}+{}", snap.provider, providers::commoncrawl::ID)),
        kd,
        keyword: snap.keyword,
        country: snap.country,
        language: snap.language,
        provider: snap.provider,
        exact: snap.exact,
        fetched_at: snap.fetched_at,
        cached: was_cached,
        items: rows,
        features,
        related: strings(&snap.related),
        questions: strings(&snap.questions),
        authority_loaded: stats.is_some(),
    })
}
