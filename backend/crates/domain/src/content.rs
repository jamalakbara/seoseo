//! Content planning from cached SERP titles/snippets. Coverage is an editorial hint, not ranking quality.
use crate::{Actor, assistant, projects, validate};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ContentBriefInput {
    pub keyword: String,
    pub model: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OptimizeContentInput {
    pub keyword: String,
    pub content: String,
}
#[derive(Serialize, ToSchema)]
pub struct ContentEvidence {
    pub keyword: String,
    pub country: String,
    pub language: String,
    pub snapshot_id: Option<Uuid>,
    pub provider: Option<String>,
    pub exact: Option<bool>,
    pub fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    pub references: Vec<db::serp::SnapshotItem>,
    pub terms: Vec<String>,
}
#[derive(Serialize, ToSchema)]
pub struct ContentScore {
    /// Percentage of sampled reference terms present, not a ranking or quality prediction.
    pub coverage_score: Option<u32>,
    pub matched_terms: Vec<String>,
    pub missing_terms: Vec<String>,
    pub word_count: usize,
    pub evidence: ContentEvidence,
    pub note: String,
}
fn tokens(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "that", "this", "from", "your", "you", "are", "was", "have",
        "has", "how", "what", "why", "can", "will", "not", "our", "all", "into", "more", "best",
        "top", "com", "www", "http", "https", "yang", "dan", "untuk", "dengan", "dari", "ini",
        "atau", "pada", "dalam",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| {
            w.chars().count() >= 3
                && w.chars().count() <= 40
                && !STOP.contains(w)
                && !w.chars().all(char::is_numeric)
        })
        .take(30000)
        .map(str::to_string)
        .collect()
}
fn reference_terms(items: &[db::serp::SnapshotItem]) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for item in items {
        for t in tokens(&format!(
            "{} {}",
            item.title,
            item.snippet.as_deref().unwrap_or("")
        )) {
            *counts.entry(t).or_default() += 1;
        }
    }
    let mut terms: Vec<_> = counts.into_iter().filter(|(_, n)| *n >= 2).collect();
    terms.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    terms.into_iter().take(30).map(|(t, _)| t).collect()
}
pub async fn evidence(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    keyword: &str,
) -> AppResult<ContentEvidence> {
    let p = projects::get(pool, a, pid).await?;
    let keyword = validate::non_empty("keyword", keyword, 200)?.to_lowercase();
    let snapshot = db::serp::latest(pool, &keyword, &p.country_code, &p.language_code, 7).await?;
    let mut out = ContentEvidence {
        keyword,
        country: p.country_code,
        language: p.language_code,
        snapshot_id: None,
        provider: None,
        exact: None,
        fetched_at: None,
        references: vec![],
        terms: vec![],
    };
    if let Some(s) = snapshot {
        out.references = db::serp::items(pool, s.id)
            .await?
            .into_iter()
            .filter(|r| (1..=10).contains(&r.position))
            .take(10)
            .collect();
        out.terms = reference_terms(&out.references);
        out.snapshot_id = Some(s.id);
        out.provider = Some(s.provider);
        out.exact = Some(s.exact);
        out.fetched_at = Some(s.fetched_at);
    }
    Ok(out)
}
pub async fn optimize(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    input: OptimizeContentInput,
) -> AppResult<ContentScore> {
    let content = validate::non_empty("content", &input.content, 100000)?;
    let evidence = evidence(pool, a, pid, &input.keyword).await?;
    Ok(score(&content, evidence))
}
fn score(content: &str, evidence: ContentEvidence) -> ContentScore {
    let words = tokens(content);
    let (matched_terms, missing_terms): (Vec<_>, Vec<_>) = evidence
        .terms
        .iter()
        .cloned()
        .partition(|t| words.contains(t));
    let coverage_score = if evidence.terms.is_empty() {
        None
    } else {
        Some((matched_terms.len() * 100 / evidence.terms.len()) as u32)
    };
    ContentScore{coverage_score,matched_terms,missing_terms,word_count:content.split_whitespace().count(),evidence,
        note:"Editorial term overlap from up to ten cached result titles/snippets in the project market, at most seven days old. Terms must appear in two reference results. Token matching works best for languages with spaces; this does not analyze competitor page bodies, intent, readability or predict ranking. Missing evidence yields no score. Add terms only when useful to the reader. Content is scored locally and is not sent to a model.".into()}
}
pub async fn brief(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    input: ContentBriefInput,
) -> AppResult<assistant::AssistantChatView> {
    if !assistant::models(cfg).models.contains(&input.model) {
        return Err(AppError::Validation(
            "Select an approved assistant model".into(),
        ));
    }
    let e = evidence(pool, a, pid, &input.keyword).await?;
    if e.references.is_empty() {
        return Err(AppError::Conflict("No recent cached SERP results in this market. Run keyword SERP analysis before generating a brief.".into()));
    }
    let chat = assistant::create(
        pool,
        a,
        pid,
        assistant::CreateChatInput {
            title: format!("Brief: {}", e.keyword).chars().take(120).collect(),
        },
    )
    .await?;
    let message = format!(
        "Generate a content brief for keyword {}. First read cached_serp for that exact keyword. Use project_memory for the business goal. Include audience and intent assumptions, suggested title, outline, questions to answer, internal-link ideas only from saved key pages, sources and evidence gaps. Treat titles/snippets as sampled evidence, not full competitor content. Distinguish proposed editorial recommendations from observed facts. Do not invent search volume or ranking guarantees.",
        serde_json::to_string(&e.keyword).map_err(AppError::internal)?
    );
    if let Err(error) = assistant::send(
        pool,
        a,
        pid,
        chat.id,
        cfg,
        assistant::SendAssistantInput {
            model: input.model,
            message,
        },
    )
    .await
    {
        assistant::delete(pool, a, pid, chat.id).await?;
        return Err(error);
    }
    assistant::get(pool, a, pid, chat.id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(title: &str) -> db::serp::SnapshotItem {
        db::serp::SnapshotItem {
            position: 1,
            url: "https://example.com".into(),
            domain: "example.com".into(),
            title: title.into(),
            snippet: None,
        }
    }
    fn empty() -> ContentEvidence {
        ContentEvidence {
            keyword: "seo".into(),
            country: "US".into(),
            language: "en".into(),
            snapshot_id: None,
            provider: None,
            exact: None,
            fetched_at: None,
            references: vec![],
            terms: vec![],
        }
    }
    #[test]
    fn terms_require_distinct_reference_results_and_scores_preserve_missing() {
        assert!(reference_terms(&[item("Audits audits audits")]).is_empty());
        assert_eq!(
            reference_terms(&[item("SEO audits and audits"), item("seo audits tools")]),
            vec!["audits", "seo"]
        );
        assert_eq!(score("SEO", empty()).coverage_score, None);
        let mut e = empty();
        e.terms = vec!["seo".into(), "audits".into()];
        let s = score("SEO SEO seoul", e);
        assert_eq!(s.coverage_score, Some(50));
        assert_eq!(s.missing_terms, vec!["audits"]);
        assert!(!tokens("seoul").contains("seo"));
        assert!(tokens("ÁRBOL árbol 2026 and dan").contains("árbol"));
    }
}
