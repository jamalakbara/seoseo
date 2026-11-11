//! Keyword research: runs, ideas, saved keywords and on-demand SERP analysis.

use crate::actor::{Actor, Role};
use crate::audits::Paged;
use crate::validate;
use common::{AppError, AppResult, new_id};
use db::keywords::{
    self, IdeaFilter, KeywordIdea, KeywordResearch, NewSaved, SavedFilter, SavedKeyword, SerpUpdate,
};
use db::models::Project;
use jobs::ProviderCtx;
use jobs::keyword_research::{KIND, ResearchConfig};
use jobs::serp::SerpAnalysis;
use providers::Market;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub const MAX_IDEAS: u32 = 500;
pub const MAX_SERP_TOP: u32 = 30;
pub const MAX_SAVED_PER_PROJECT: i64 = 5_000;
const MAX_TAGS: usize = 10;

async fn project(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Project> {
    db::projects::get(pool, actor.org_id, project_id).await
}

/// Normalize and validate a keyword typed by a user.
fn keyword(raw: &str) -> AppResult<String> {
    let k = scoring::normalize_keyword(raw);
    if k.is_empty() {
        return Err(AppError::Validation("enter a keyword".into()));
    }
    if k.chars().count() > 120 || k.split_whitespace().count() > 12 {
        return Err(AppError::Validation(
            "a keyword can have at most 120 characters and 12 words".into(),
        ));
    }
    Ok(k)
}

fn market(p: &Project, country: Option<&str>, language: Option<&str>) -> AppResult<Market> {
    let country = validate::country(country.unwrap_or(&p.country_code))?;
    let language = validate::language(language.unwrap_or(&p.language_code))?;
    Ok(Market::new(&country, &language))
}

fn tags(raw: Option<Vec<String>>) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = raw
        .unwrap_or_default()
        .iter()
        .map(|t| scoring::normalize_keyword(t))
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    if out.len() > MAX_TAGS || out.iter().any(|t| t.chars().count() > 30) {
        return Err(AppError::Validation(format!(
            "at most {MAX_TAGS} tags of up to 30 characters each"
        )));
    }
    Ok(out)
}

// ---------------------------------------------------------------- research

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct StartResearchInput {
    /// The seed keyword, e.g. `standing desk`.
    pub seed: String,
    /// ISO 3166-1 alpha-2; defaults to the project's market.
    pub country: Option<String>,
    /// ISO 639-1; defaults to the project's market.
    pub language: Option<String>,
    /// Ideas to keep (10–500, default 150).
    pub max_ideas: Option<u32>,
    /// Most popular ideas that also get a SERP analysis (0–30, default 10).
    pub serp_top: Option<u32>,
}

pub async fn start(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: StartResearchInput,
) -> AppResult<KeywordResearch> {
    actor.require(Role::Member)?;
    let p = project(pool, actor, project_id).await?;
    if p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "restore the project before researching keywords".into(),
        ));
    }
    let seed = keyword(&input.seed)?;
    let m = market(&p, input.country.as_deref(), input.language.as_deref())?;
    let d = ResearchConfig::default();
    let max_ideas = input.max_ideas.unwrap_or(d.max_ideas);
    let serp_top = input.serp_top.unwrap_or(d.serp_top);
    if !(10..=MAX_IDEAS).contains(&max_ideas) {
        return Err(AppError::Validation(format!(
            "max_ideas must be between 10 and {MAX_IDEAS}"
        )));
    }
    if serp_top > MAX_SERP_TOP {
        return Err(AppError::Validation(format!(
            "serp_top must be between 0 and {MAX_SERP_TOP}"
        )));
    }
    if keywords::count_active_researches(pool, project_id).await? >= 3 {
        return Err(AppError::Conflict(
            "three keyword researches are already running for this project; wait for one to finish"
                .into(),
        ));
    }
    let id = new_id();
    let research = keywords::insert_research(
        pool,
        id,
        project_id,
        actor.user_id,
        &seed,
        &m.country,
        &m.language,
        serde_json::to_value(ResearchConfig {
            max_ideas,
            serp_top,
        })
        .map_err(AppError::internal)?,
    )
    .await?;
    let job = db::jobs::enqueue(
        pool,
        new_id(),
        KIND,
        Some(actor.org_id),
        Some(project_id),
        serde_json::json!({ "research_id": id }),
        3,
    )
    .await?;
    keywords::set_research_job(pool, id, job.id).await?;
    Ok(KeywordResearch {
        job_id: Some(job.id),
        ..research
    })
}

pub async fn list(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
) -> AppResult<Vec<KeywordResearch>> {
    project(pool, actor, project_id).await?;
    keywords::list_researches(pool, project_id, 50).await
}

pub async fn get(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    research_id: Uuid,
) -> AppResult<KeywordResearch> {
    project(pool, actor, project_id).await?;
    keywords::get_research(pool, project_id, research_id).await
}

pub async fn delete(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    research_id: Uuid,
) -> AppResult<()> {
    actor.require(Role::Member)?;
    let r = get(pool, actor, project_id, research_id).await?;
    if matches!(r.status.as_str(), "queued" | "running") {
        return Err(AppError::Conflict(
            "wait for the research to finish before deleting it".into(),
        ));
    }
    keywords::delete_research(pool, project_id, research_id).await
}

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct IdeaQuery {
    /// Substring of the keyword.
    pub search: Option<String>,
    /// informational | commercial | transactional | navigational
    pub intent: Option<String>,
    /// Only ideas from this source, e.g. `suggest_google`, `related`.
    pub source: Option<String>,
    /// Only question-style ideas.
    pub questions: Option<bool>,
    pub min_popularity: Option<i32>,
    /// Only ideas with a known difficulty at or below this.
    pub max_kd: Option<i32>,
    /// Only ideas with a known volume estimate at or above this.
    pub min_volume: Option<i64>,
    pub min_words: Option<i32>,
    pub max_words: Option<i32>,
    /// popularity (default) | keyword | est_volume | kd | word_count
    pub sort: Option<String>,
    pub desc: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn ideas(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    research_id: Uuid,
    q: IdeaQuery,
) -> AppResult<Paged<KeywordIdea>> {
    get(pool, actor, project_id, research_id).await?;
    let filter = IdeaFilter {
        search: q.search,
        intent: q.intent,
        source: q.source,
        questions: q.questions,
        min_popularity: q.min_popularity,
        max_kd: q.max_kd,
        min_volume: q.min_volume,
        min_words: q.min_words,
        max_words: q.max_words,
    };
    let (items, total) = keywords::ideas(
        pool,
        research_id,
        &filter,
        q.sort.as_deref(),
        q.desc,
        q.limit.unwrap_or(50).clamp(1, 1000),
        q.offset.unwrap_or(0).max(0),
    )
    .await?;
    Ok(Paged { items, total })
}

// ---------------------------------------------------------------- saved keywords

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct SaveKeywordsInput {
    /// Save ideas of this research (with their metrics)…
    pub research_id: Option<Uuid>,
    /// …these ones.
    pub idea_ids: Option<Vec<Uuid>>,
    /// Or save keywords typed by hand (project market, no metrics yet).
    pub keywords: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SaveKeywordsResult {
    pub saved: u64,
    /// Already saved before, so left unchanged.
    pub already_saved: u64,
}

pub async fn save(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: SaveKeywordsInput,
) -> AppResult<SaveKeywordsResult> {
    actor.require(Role::Member)?;
    let p = project(pool, actor, project_id).await?;
    let tags = tags(input.tags)?;
    let idea_ids = input.idea_ids.unwrap_or_default();
    let manual: Vec<String> = input
        .keywords
        .unwrap_or_default()
        .iter()
        .map(|k| keyword(k))
        .collect::<AppResult<_>>()?;
    let requested = (idea_ids.len() + manual.len()) as u64;
    if requested == 0 {
        return Err(AppError::Validation(
            "pass idea_ids with research_id, or keywords".into(),
        ));
    }
    if requested > 1000 {
        return Err(AppError::Validation(
            "save at most 1000 keywords per request".into(),
        ));
    }
    if keywords::count_saved(pool, project_id).await? + requested as i64 > MAX_SAVED_PER_PROJECT {
        return Err(AppError::Conflict(format!(
            "a project can hold {MAX_SAVED_PER_PROJECT} saved keywords; delete some first"
        )));
    }
    let mut saved = 0;
    if !idea_ids.is_empty() {
        let research_id = input.research_id.ok_or_else(|| {
            AppError::Validation("research_id is required together with idea_ids".into())
        })?;
        keywords::get_research(pool, project_id, research_id).await?;
        saved += keywords::save_ideas(
            pool,
            project_id,
            research_id,
            &idea_ids,
            &tags,
            actor.user_id,
        )
        .await?;
    }
    if !manual.is_empty() {
        let intents: Vec<&'static str> = manual
            .iter()
            .map(|k| scoring::classify_intent(k, p.brand_name.as_deref(), &[]).as_str())
            .collect();
        let rows: Vec<NewSaved<'_>> = manual
            .iter()
            .zip(&intents)
            .map(|(k, i)| NewSaved {
                keyword: k,
                intent: i,
            })
            .collect();
        saved += keywords::save_manual(
            pool,
            project_id,
            &p.country_code,
            &p.language_code,
            &rows,
            &tags,
            actor.user_id,
        )
        .await?;
    }
    Ok(SaveKeywordsResult {
        saved,
        already_saved: requested.saturating_sub(saved),
    })
}

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct SavedQuery {
    pub search: Option<String>,
    pub tag: Option<String>,
    pub intent: Option<String>,
    /// created_at (default) | keyword | popularity | est_volume | kd | word_count
    pub sort: Option<String>,
    pub desc: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn saved(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    q: SavedQuery,
) -> AppResult<Paged<SavedKeyword>> {
    project(pool, actor, project_id).await?;
    // Saved keywords have no word_count column; fall back to the default order.
    let sort = q
        .sort
        .as_deref()
        .filter(|s| !matches!(*s, "word_count" | "words"));
    let (items, total) = keywords::saved(
        pool,
        project_id,
        &SavedFilter {
            search: q.search,
            tag: q.tag,
            intent: q.intent,
        },
        sort,
        q.desc,
        q.limit.unwrap_or(50).clamp(1, 1000),
        q.offset.unwrap_or(0).max(0),
    )
    .await?;
    Ok(Paged { items, total })
}

pub async fn saved_tags(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Vec<String>> {
    project(pool, actor, project_id).await?;
    keywords::saved_tags(pool, project_id).await
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSavedInput {
    /// Replaces the keyword's tags.
    pub tags: Vec<String>,
}

pub async fn update_saved(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
    input: UpdateSavedInput,
) -> AppResult<SavedKeyword> {
    actor.require(Role::Member)?;
    project(pool, actor, project_id).await?;
    keywords::set_saved_tags(pool, project_id, id, &tags(Some(input.tags))?).await
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteSavedInput {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteSavedResult {
    pub deleted: u64,
}

pub async fn delete_saved(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: DeleteSavedInput,
) -> AppResult<DeleteSavedResult> {
    actor.require(Role::Member)?;
    project(pool, actor, project_id).await?;
    if input.ids.is_empty() || input.ids.len() > 1000 {
        return Err(AppError::Validation("pass 1 to 1000 ids".into()));
    }
    Ok(DeleteSavedResult {
        deleted: keywords::delete_saved(pool, project_id, &input.ids).await?,
    })
}

// ---------------------------------------------------------------- SERP analysis

#[derive(Debug, Deserialize, ToSchema)]
pub struct SerpAnalysisInput {
    pub keyword: String,
    /// Defaults to the project's market.
    pub country: Option<String>,
    pub language: Option<String>,
    /// Fetch a fresh page instead of a cached one (cached pages are reused for 7 days).
    pub refresh: Option<bool>,
}

/// Analyse the results page of a keyword. Matching saved keywords and research ideas of this
/// project get their difficulty, intent and SERP features updated.
pub async fn serp_analysis(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
    project_id: Uuid,
    input: SerpAnalysisInput,
) -> AppResult<SerpAnalysis> {
    actor.require(Role::Member)?;
    let p = project(pool, actor, project_id).await?;
    let kw = keyword(&input.keyword)?;
    let m = market(&p, input.country.as_deref(), input.language.as_deref())?;
    let registry = pctx.registry(pool, actor.org_id).await?;
    let a = jobs::serp::analyze(
        pool,
        &registry,
        &kw,
        &m,
        p.brand_name.as_deref(),
        input.refresh.unwrap_or(false),
    )
    .await?;
    let update = SerpUpdate {
        kd: a.kd,
        kd_source: a.kd_source.as_deref(),
        intent: &a.intent,
        features: &a.features,
        snapshot_id: a.snapshot_id,
    };
    keywords::set_saved_serp(pool, project_id, &kw, &m.country, &m.language, &update).await?;
    keywords::set_project_ideas_serp(pool, project_id, &kw, &m.country, &m.language, &update)
        .await?;
    Ok(a)
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct RefreshSavedInput {
    /// Omit or pass an empty list to refresh all saved keywords. Otherwise 1–1000 IDs.
    #[serde(default)]
    pub ids: Vec<Uuid>,
}

pub async fn refresh_saved(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
    project_id: Uuid,
    mut input: RefreshSavedInput,
) -> AppResult<db::models::Job> {
    actor.require(Role::Member)?;
    let p = project(pool, actor, project_id).await?;
    if p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "restore the project before refreshing keywords".into(),
        ));
    }
    if input.ids.len() > 1000 {
        return Err(AppError::Validation("select at most 1000 keywords".into()));
    }
    input.ids.sort_unstable();
    input.ids.dedup();
    if !pctx.registry(pool, actor.org_id).await?.has_serp() {
        return Err(AppError::Conflict(
            "configure a SERP provider before refreshing keywords".into(),
        ));
    }
    keywords::enqueue_refresh(pool, actor.org_id, project_id, &input.ids).await
}
