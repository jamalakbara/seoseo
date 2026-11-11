//! Durable bulk refresh. The payload freezes selection; progress checkpoints each keyword.
use crate::{JobCtx, ProviderCtx};
use db::keywords::{self, SerpUpdate};
use providers::Market;
use serde_json::json;
use uuid::Uuid;

pub const KIND: &str = "saved_keyword_refresh";

pub fn register(registry: crate::Registry, providers: ProviderCtx) -> crate::Registry {
    registry.register(KIND, move |ctx| {
        let providers = providers.clone();
        async move { run(ctx, providers).await }
    })
}

async fn run(ctx: JobCtx, providers: ProviderCtx) -> anyhow::Result<()> {
    let project_id = ctx
        .job
        .project_id
        .ok_or_else(|| anyhow::anyhow!("missing project"))?;
    let org_id = ctx
        .job
        .organization_id
        .ok_or_else(|| anyhow::anyhow!("missing organization"))?;
    let project = db::projects::get(&ctx.pool, org_id, project_id).await?;
    anyhow::ensure!(project.archived_at.is_none(), "project is archived");
    let ids: Vec<Uuid> = serde_json::from_value(ctx.job.payload["ids"].clone())?;
    let registry = providers.registry(&ctx.pool, org_id).await?;
    anyhow::ensure!(
        registry.has_serp(),
        "configure a SERP provider before refreshing keywords"
    );
    let done = ctx.job.progress["done"].as_u64().unwrap_or(0) as usize;
    for (i, id) in ids.iter().enumerate().skip(done) {
        if let Some(k) = keywords::saved_by_id(&ctx.pool, project_id, *id).await? {
            // A crash after the write but before the checkpoint must not fetch twice.
            if k.metrics_updated_at.is_none_or(|t| t < ctx.job.created_at) {
                let market = Market::new(&k.country, &k.language);
                if registry.wikipedia {
                    let titles = providers::wikipedia::resolve_titles(
                        market.lang(),
                        std::slice::from_ref(&k.keyword),
                    )
                    .await?;
                    let views = match titles.get(&k.keyword) {
                        Some(title) => {
                            crate::keyword_research::cached_views(&ctx.pool, market.lang(), title)
                                .await?
                        }
                        None => Vec::new(),
                    };
                    let volume = if views.is_empty() {
                        None
                    } else {
                        let avg = views.iter().map(|v| v.views).sum::<i64>() / views.len() as i64;
                        Some(keywords::VolumeUpdate {
                            est_volume: avg,
                            bucket: scoring::volume_bucket(avg),
                            source: providers::wikipedia::ID,
                            confidence: "estimated",
                            trend: json!(views),
                        })
                    };
                    keywords::set_saved_volume(&ctx.pool, project_id, k.id, volume.as_ref())
                        .await?;
                }
                let a = crate::serp::analyze(
                    &ctx.pool,
                    &registry,
                    &k.keyword,
                    &market,
                    project.brand_name.as_deref(),
                    true,
                )
                .await?;
                let update = SerpUpdate {
                    kd: a.kd,
                    kd_source: a.kd_source.as_deref(),
                    intent: &a.intent,
                    features: &a.features,
                    snapshot_id: a.snapshot_id,
                };
                keywords::set_saved_serp(
                    &ctx.pool,
                    project_id,
                    &k.keyword,
                    &k.country,
                    &k.language,
                    &update,
                )
                .await?;
            }
        }
        ctx.progress(json!({"done": i + 1, "total": ids.len()}))
            .await?;
    }
    Ok(())
}
