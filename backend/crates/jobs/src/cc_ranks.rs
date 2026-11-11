//! `cc_rank_import` job: load the top N domains of the latest Common Crawl domain graph
//! into `domain_ranks` and score them (SeoSeo Rank). Upserts make a retry idempotent.

use crate::JobCtx;
use crate::provider_ctx::ProviderCtx;
use chrono::Utc;
use db::providers::NewDomainRank;
use providers::commoncrawl::{self, RankStream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

pub const KIND: &str = "cc_rank_import";
/// `instance_settings` key holding [`AuthorityStatus`].
pub const STATUS_KEY: &str = "authority_commoncrawl";
pub const DEFAULT_LIMIT: u32 = 1_000_000;
pub const MAX_LIMIT: u32 = 20_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthorityStatus {
    /// Common Crawl web-graph release, e.g. `cc-main-2026-aug-sep-oct`.
    pub release: String,
    /// Domains in the whole graph (the worst possible rank).
    pub total_domains: i64,
    /// Domains imported (top of the ranking).
    pub imported: i64,
    pub imported_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize)]
struct Payload {
    limit: Option<u32>,
}

pub fn register(registry: crate::Registry, pctx: ProviderCtx) -> crate::Registry {
    registry.register(KIND, move |ctx: JobCtx| {
        let pctx = pctx.clone();
        async move { run(ctx, pctx).await }
    })
}

async fn run(ctx: JobCtx, pctx: ProviderCtx) -> anyhow::Result<()> {
    let payload: Payload = serde_json::from_value(ctx.job.payload.clone())?;
    let limit = payload
        .limit
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1_000, MAX_LIMIT) as usize;
    let pool = &ctx.pool;

    let release = commoncrawl::latest_release(pctx.cc_graphinfo_url.as_deref()).await?;
    ctx.progress(json!({ "release": release.id, "imported": 0, "limit": limit }))
        .await?;
    let mut stream = RankStream::open(&release.ranks_url(), limit).await?;
    let mut imported = 0i64;
    let mut pending = Vec::new();
    while let Some(rows) = stream.next_batch().await? {
        pending.extend(rows);
        if pending.len() < 5_000 {
            continue;
        }
        imported += flush(pool, &release, &mut pending).await?;
        ctx.progress(json!({ "release": release.id, "imported": imported, "limit": limit }))
            .await?;
    }
    imported += flush(pool, &release, &mut pending).await?;
    if imported == 0 {
        anyhow::bail!("the Common Crawl rank file contained no rows");
    }

    let removed = db::providers::delete_other_releases(pool, &release.id).await?;
    let status = AuthorityStatus {
        release: release.id.clone(),
        total_domains: release.total_domains,
        imported,
        imported_at: Utc::now(),
    };
    db::settings::set(pool, STATUS_KEY, serde_json::to_value(&status)?).await?;
    ctx.progress(
        json!({ "release": release.id, "imported": imported, "limit": limit, "done": true }),
    )
    .await?;
    tracing::info!(release = %release.id, imported, removed, "domain ranks imported");
    Ok(())
}

async fn flush(
    pool: &sqlx::PgPool,
    release: &commoncrawl::Release,
    pending: &mut Vec<commoncrawl::RankRow>,
) -> anyhow::Result<i64> {
    // One upsert statement may not touch the same row twice.
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<NewDomainRank<'_>> = pending
        .iter()
        .filter(|r| seen.insert(r.domain.as_str()))
        .filter_map(|r| {
            Some(NewDomainRank {
                domain: &r.domain,
                harmonic_rank: r.harmonic_rank,
                pagerank_rank: r.pagerank_rank,
                sr: scoring::sr_from_rank(r.harmonic_rank, release.total_domains)? as i16,
            })
        })
        .collect();
    let n = rows.len() as i64;
    db::providers::upsert_domain_ranks(pool, &release.id, &rows).await?;
    drop(rows);
    pending.clear();
    Ok(n)
}
