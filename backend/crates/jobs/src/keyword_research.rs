//! `keyword_research` job: ideas → demand → serp.
//!
//! - **ideas**: autocomplete expansion of the seed (a–z, questions, prepositions) on every
//!   enabled suggestion engine; ideas get a relative popularity score.
//! - **demand**: keywords that match a Wikipedia article get measured monthly pageviews as a
//!   labelled volume estimate and 12-month trend. Everything else stays null.
//! - **serp**: the most popular ideas get a SERP analysis (features, difficulty, intent) and
//!   the seed's related searches/questions become extra ideas.
//!
//! State lives in Postgres and provider responses are cached, so a retry after a crash
//! redoes little work. Provider trouble becomes a warning on the research, not a failure,
//! unless no idea could be produced at all.

use crate::JobCtx;
use crate::provider_ctx::ProviderCtx;
use db::keywords::{self, NewIdea, SerpUpdate, VolumeUpdate};
use providers::suggest::{Expansion, expansions, question_words};
use providers::wikipedia::{self, MonthViews};
use providers::{Market, ProviderError, Registry, SuggestProvider};
use scoring::SuggestSignal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

pub const KIND: &str = "keyword_research";
const SUGGEST_TTL_DAYS: i32 = 30;
const VIEWS_TTL_DAYS: i32 = 30;
/// Pageview lookups per research (one request each).
const MAX_VIEW_LOOKUPS: usize = 25;
/// Extra ideas taken from the seed's related searches and questions.
const MAX_SERP_IDEAS: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConfig {
    pub max_ideas: u32,
    /// How many of the most popular ideas get a SERP analysis (0 = none).
    pub serp_top: u32,
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            max_ideas: 150,
            serp_top: 10,
        }
    }
}

#[derive(Deserialize)]
struct Payload {
    research_id: Uuid,
}

pub fn register(registry: crate::Registry, pctx: ProviderCtx) -> crate::Registry {
    registry.register(KIND, move |ctx: JobCtx| {
        let pctx = pctx.clone();
        async move { run(ctx, pctx).await }
    })
}

async fn run(ctx: JobCtx, pctx: ProviderCtx) -> anyhow::Result<()> {
    let payload: Payload = serde_json::from_value(ctx.job.payload.clone())?;
    let result = run_research(&ctx, &pctx, payload.research_id).await;
    if let Err(e) = &result
        && ctx.job.attempts >= ctx.job.max_attempts
    {
        let _ = keywords::fail_research(&ctx.pool, payload.research_id, &format!("{e:#}")).await;
    }
    result
}

async fn run_research(ctx: &JobCtx, pctx: &ProviderCtx, id: Uuid) -> anyhow::Result<()> {
    let pool = &ctx.pool;
    let research = keywords::get_research_any(pool, id).await?;
    if matches!(research.status.as_str(), "completed" | "failed") {
        return Ok(());
    }
    let cfg: ResearchConfig = serde_json::from_value(research.config.clone()).unwrap_or_default();
    let project = sqlx::query_as::<_, (Uuid, Option<String>)>(
        "SELECT organization_id, brand_name FROM projects WHERE id = $1",
    )
    .bind(research.project_id)
    .fetch_one(pool)
    .await?;
    let (org_id, brand) = project;
    let registry = pctx.registry(pool, org_id).await?;
    let market = Market::new(&research.country, &research.language);
    let seed = scoring::normalize_keyword(&research.seed);

    // ---- ideas
    if matches!(research.phase.as_str(), "queued" | "ideas") {
        keywords::set_research_phase(pool, id, "ideas").await?;
        ctx.progress(json!({ "phase": "ideas" })).await?;
        if registry.suggest.is_empty() {
            keywords::fail_research(
                pool,
                id,
                "No keyword idea provider is enabled. Turn on an autocomplete provider in Settings → Data providers.",
            )
            .await?;
            return Ok(());
        }
        let found = collect_ideas(ctx, &registry, id, &seed, &market).await?;
        if found.len() <= 1 {
            keywords::fail_research(
                pool,
                id,
                "No ideas found for this seed. Try a shorter or more common phrase, or check the provider status in Settings → Data providers.",
            )
            .await?;
            return Ok(());
        }
        let rows = rank_ideas(found, &seed, market.lang(), brand.as_deref(), cfg.max_ideas);
        keywords::insert_ideas(pool, id, &rows).await?;
        keywords::set_research_phase(pool, id, "demand").await?;
    }

    // ---- demand
    let phase = keywords::get_research_any(pool, id).await?.phase;
    if phase == "demand" {
        ctx.progress(json!({ "phase": "demand" })).await?;
        if registry.wikipedia
            && let Err(e) = demand(pool, id, &market, cfg.max_ideas).await
        {
            warn(pool, id, &format!("Wikipedia pageviews were skipped: {e}")).await;
        }
        keywords::set_research_phase(pool, id, "serp").await?;
    }

    // ---- serp
    let phase = keywords::get_research_any(pool, id).await?.phase;
    if phase == "serp" {
        ctx.progress(json!({ "phase": "serp" })).await?;
        if cfg.serp_top > 0 {
            if registry.has_serp() {
                serp(ctx, &registry, id, &seed, &market, brand.as_deref(), &cfg).await?;
            } else {
                warn(
                    pool,
                    id,
                    "No search results provider is set up, so keyword difficulty and SERP features are empty. Start SearXNG (compose profile `serp`) or add a key in Settings → Data providers.",
                )
                .await;
            }
        }
    }
    keywords::finish_research(pool, id).await?;
    Ok(())
}

async fn warn(pool: &PgPool, id: Uuid, message: &str) {
    if let Err(e) = keywords::add_research_warning(pool, id, message).await {
        tracing::warn!(error = %e, "could not store research warning");
    }
}

// ---------------------------------------------------------------- ideas

#[derive(Default)]
struct Found {
    signal: SuggestSignal,
    engines: BTreeSet<&'static str>,
    queries: HashSet<String>,
}

async fn cached_suggest(
    pool: &PgPool,
    engine: &Arc<dyn SuggestProvider>,
    query: &str,
    market: &Market,
) -> Result<Vec<String>, ProviderError> {
    let kind = format!("suggest:{}", engine.id());
    let key = format!("{}|{}|{}", market.country, market.language, query);
    if let Ok(Some(v)) = db::providers::cache_get(pool, &kind, &key, SUGGEST_TTL_DAYS).await
        && let Ok(list) = serde_json::from_value::<Vec<String>>(v)
    {
        return Ok(list);
    }
    let list = engine.suggest(query, market).await?;
    let _ = db::providers::cache_put(pool, &kind, &key, &json!(list)).await;
    Ok(list)
}

/// Run every expansion on one engine. Stops early (with a warning) when the engine rate
/// limits or blocks us; we never retry around a block.
async fn run_engine(
    pool: &PgPool,
    research_id: Uuid,
    engine: Arc<dyn SuggestProvider>,
    plan: &[Expansion],
    market: &Market,
) -> Vec<(Expansion, Vec<String>)> {
    let mut out = Vec::with_capacity(plan.len());
    let mut failures = 0u32;
    for exp in plan {
        match cached_suggest(pool, &engine, &exp.query, market).await {
            Ok(list) => {
                failures = 0;
                out.push((exp.clone(), list));
            }
            Err(e @ (ProviderError::RateLimited | ProviderError::Blocked(_))) => {
                warn(
                    pool,
                    research_id,
                    &format!("{} stopped early: {e}", engine.id()),
                )
                .await;
                break;
            }
            Err(e) => {
                failures += 1;
                if failures >= 3 {
                    warn(
                        pool,
                        research_id,
                        &format!("{} stopped early: {e}", engine.id()),
                    )
                    .await;
                    break;
                }
            }
        }
    }
    out
}

async fn collect_ideas(
    ctx: &JobCtx,
    registry: &Registry,
    research_id: Uuid,
    seed: &str,
    market: &Market,
) -> anyhow::Result<HashMap<String, Found>> {
    let plan = expansions(seed, market.lang());
    let runs = registry.suggest.iter().map(|engine| {
        let engine = engine.clone();
        let plan = &plan;
        async move {
            let id = engine.id();
            (
                id,
                run_engine(&ctx.pool, research_id, engine, plan, market).await,
            )
        }
    });
    let results = futures::future::join_all(runs).await;

    let mut found: HashMap<String, Found> = HashMap::new();
    for (engine, lists) in results {
        for (exp, list) in lists {
            for (rank, raw) in list.iter().enumerate() {
                let kw = scoring::normalize_keyword(raw);
                if !relevant(&kw, seed) {
                    continue;
                }
                let f = found.entry(kw).or_default();
                if f.queries.is_empty() || rank < f.signal.best_rank {
                    f.signal.best_rank = rank;
                    f.signal.best_kind = exp.kind.to_string();
                }
                f.engines.insert(engine);
                f.queries.insert(exp.query.clone());
            }
        }
    }
    for f in found.values_mut() {
        f.signal.engines = f.engines.len();
        f.signal.hits = f.queries.len();
    }
    // The seed itself is always an idea.
    found.entry(seed.to_string()).or_default();
    Ok(found)
}

/// Keep suggestions that are plausible keywords and share vocabulary with the seed
/// (autocomplete sometimes "corrects" a query into something unrelated).
fn relevant(keyword: &str, seed: &str) -> bool {
    let words: Vec<&str> = keyword.split_whitespace().collect();
    if keyword.is_empty() || keyword.chars().count() > 120 || words.len() > 12 {
        return false;
    }
    let seed_words: Vec<&str> = seed.split_whitespace().collect();
    let shared = seed_words.iter().filter(|w| words.contains(w)).count();
    shared >= seed_words.len().div_ceil(2).max(1) || keyword.contains(seed)
}

fn rank_ideas(
    found: HashMap<String, Found>,
    seed: &str,
    lang: &str,
    brand: Option<&str>,
    max: u32,
) -> Vec<NewIdea> {
    let questions = question_words(lang);
    let mut rows: Vec<NewIdea> = found
        .into_iter()
        .map(|(keyword, f)| {
            let is_seed = keyword == seed;
            let mut sources: Vec<String> = f.engines.iter().map(|e| e.to_string()).collect();
            if is_seed {
                sources.insert(0, "seed".into());
            }
            let first = keyword.split_whitespace().next().unwrap_or_default();
            if questions.contains(&first)
                || questions
                    .iter()
                    .any(|q| q.contains(' ') && keyword.starts_with(&format!("{q} ")))
            {
                sources.push("question".into());
            }
            NewIdea {
                id: common::new_id(),
                popularity: if is_seed {
                    100
                } else {
                    scoring::popularity(&f.signal)
                },
                intent: scoring::classify_intent(&keyword, brand, &[])
                    .as_str()
                    .to_string(),
                keyword,
                sources,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.popularity
            .cmp(&a.popularity)
            .then_with(|| a.keyword.cmp(&b.keyword))
    });
    rows.truncate(max as usize);
    rows
}

// ---------------------------------------------------------------- demand

pub(crate) async fn cached_views(
    pool: &PgPool,
    lang: &str,
    title: &str,
) -> Result<Vec<MonthViews>, ProviderError> {
    let key = format!("{lang}|{title}");
    if let Ok(Some(v)) = db::providers::cache_get(pool, "wiki_views", &key, VIEWS_TTL_DAYS).await
        && let Ok(views) = serde_json::from_value::<Vec<MonthViews>>(v)
    {
        return Ok(views);
    }
    let views = wikipedia::monthly_views(lang, title).await?;
    let _ = db::providers::cache_put(pool, "wiki_views", &key, &json!(views)).await;
    Ok(views)
}

async fn demand(
    pool: &PgPool,
    research_id: Uuid,
    market: &Market,
    max_ideas: u32,
) -> Result<(), ProviderError> {
    let ideas = keywords::top_ideas(pool, research_id, i64::from(max_ideas))
        .await
        .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
    let mut lookups = 0usize;
    for chunk in ideas.chunks(wikipedia::TITLES_PER_REQUEST) {
        let names: Vec<String> = chunk.iter().map(|i| i.keyword.clone()).collect();
        let titles = wikipedia::resolve_titles(market.lang(), &names).await?;
        for idea in chunk {
            let Some(title) = titles.get(&idea.keyword) else {
                continue;
            };
            if lookups >= MAX_VIEW_LOOKUPS {
                return Ok(());
            }
            lookups += 1;
            let views = cached_views(pool, market.lang(), title).await?;
            if views.is_empty() {
                continue;
            }
            let avg = views.iter().map(|m| m.views).sum::<i64>() / views.len() as i64;
            let update = VolumeUpdate {
                est_volume: avg,
                bucket: scoring::volume_bucket(avg),
                source: wikipedia::ID,
                confidence: "estimated",
                trend: json!(views),
            };
            keywords::set_idea_volume(pool, idea.id, &update)
                .await
                .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- serp

async fn serp(
    ctx: &JobCtx,
    registry: &Registry,
    research_id: Uuid,
    seed: &str,
    market: &Market,
    brand: Option<&str>,
    cfg: &ResearchConfig,
) -> anyhow::Result<()> {
    let pool = &ctx.pool;
    let ideas = keywords::top_ideas(pool, research_id, i64::from(cfg.serp_top)).await?;
    let mut authority_missing = false;
    for (n, idea) in ideas.iter().enumerate() {
        if idea.serp_features.is_some() {
            continue; // analysed before a restart
        }
        ctx.progress(json!({ "phase": "serp", "done": n, "total": ideas.len() }))
            .await?;
        let a =
            match crate::serp::analyze(pool, registry, &idea.keyword, market, brand, false).await {
                Ok(a) => a,
                Err(e) => {
                    warn(
                        pool,
                        research_id,
                        &format!("SERP analysis stopped after {n} keywords: {e}"),
                    )
                    .await;
                    break;
                }
            };
        authority_missing |= !a.authority_loaded;
        keywords::set_idea_serp(
            pool,
            idea.id,
            &SerpUpdate {
                kd: a.kd,
                kd_source: a.kd_source.as_deref(),
                intent: &a.intent,
                features: &a.features,
                snapshot_id: a.snapshot_id,
            },
        )
        .await?;

        // Related searches and questions of the seed's SERP become ideas (KW-02).
        if idea.keyword == seed {
            let known: HashSet<String> = keywords::top_ideas(pool, research_id, 1000)
                .await?
                .into_iter()
                .map(|i| i.keyword)
                .collect();
            let extra: Vec<NewIdea> = a
                .related
                .iter()
                .map(|k| (k, "related"))
                .chain(a.questions.iter().map(|k| (k, "question")))
                .map(|(k, tag)| (scoring::normalize_keyword(k), tag))
                .filter(|(k, _)| !k.is_empty() && k.chars().count() <= 120 && !known.contains(k))
                .take(MAX_SERP_IDEAS)
                .map(|(keyword, tag)| NewIdea {
                    id: common::new_id(),
                    popularity: scoring::popularity(&SuggestSignal {
                        best_kind: "related".into(),
                        ..Default::default()
                    }),
                    intent: scoring::classify_intent(&keyword, brand, &[])
                        .as_str()
                        .to_string(),
                    sources: vec![a.provider.clone(), tag.to_string()],
                    keyword,
                })
                .collect();
            keywords::insert_ideas(pool, research_id, &extra).await?;
        }
    }
    if authority_missing {
        warn(
            pool,
            research_id,
            "Domain authority data is not imported yet, so keyword difficulty is empty. Import it in Settings → Data providers.",
        )
        .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance_filter() {
        assert!(relevant("standing desk converter", "standing desk"));
        assert!(relevant("best desk for standing", "standing desk"));
        assert!(relevant("crm software", "crm"));
        assert!(!relevant("weather tomorrow", "standing desk"));
        assert!(!relevant("", "crm"));
        assert!(!relevant(&"crm ".repeat(13), "crm"));
        // One shared word out of three is not enough.
        assert!(!relevant("desk lamp", "adjustable standing desk"));
    }

    #[test]
    fn ranks_and_tags_ideas() {
        let mut found: HashMap<String, Found> = HashMap::new();
        found.entry("crm".into()).or_default();
        let mut top = Found {
            signal: SuggestSignal {
                best_rank: 0,
                best_kind: "seed".into(),
                engines: 2,
                hits: 2,
            },
            ..Default::default()
        };
        top.engines.extend(["suggest_bing", "suggest_google"]);
        found.insert("crm software".into(), top);
        let mut q = Found {
            signal: SuggestSignal {
                best_rank: 3,
                best_kind: "alphabet".into(),
                engines: 1,
                hits: 1,
            },
            ..Default::default()
        };
        q.engines.insert("suggest_google");
        found.insert("what is crm".into(), q);

        let rows = rank_ideas(found, "crm", "en", None, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].keyword, "crm");
        assert_eq!(rows[0].popularity, 100);
        assert_eq!(rows[0].sources[0], "seed");
        assert_eq!(rows[1].keyword, "crm software");
        assert_eq!(rows[1].sources, vec!["suggest_bing", "suggest_google"]);

        let mut found: HashMap<String, Found> = HashMap::new();
        found.entry("what is crm".into()).or_default();
        let rows = rank_ideas(found, "crm", "en", None, 10);
        assert!(rows[0].sources.contains(&"question".to_string()));
        assert_eq!(rows[0].intent, "informational");
    }
}
