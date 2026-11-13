use crate::{Actor, validate};
use common::{AppError, AppResult};
use db::ai_index::{AiIndexSummary, IndexFilter, IndexedAiAnswer, IndexedAiPrompt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct AiIndexQuery {
    /// prompts (default) or answers. Brand filters match literal terms in saved answers.
    pub view: Option<String>,
    pub brand: Option<String>,
    pub topic: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct AiOwnIndex {
    pub country: String,
    pub language: String,
    pub view: String,
    pub brand: Option<String>,
    pub topic: Option<String>,
    pub summary: AiIndexSummary,
    pub mention_rate_percent: Option<f64>,
    pub total_matching: i64,
    pub prompts: Vec<IndexedAiPrompt>,
    pub answers: Vec<IndexedAiAnswer>,
    pub warnings: Vec<String>,
}
pub async fn inspect(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    q: AiIndexQuery,
) -> AppResult<AiOwnIndex> {
    let p = db::projects::get(pool, a.org_id, pid).await?;
    let country = validate::country(q.country.as_deref().unwrap_or(&p.country_code))?;
    let language = validate::language(q.language.as_deref().unwrap_or(&p.language_code))?;
    let view = q.view.unwrap_or_else(|| "prompts".into());
    if !["prompts", "answers"].contains(&view.as_str())
        || !(1..=100).contains(&q.limit.unwrap_or(25))
        || q.offset.unwrap_or(0) < 0
    {
        return Err(AppError::Validation(
            "choose prompts/answers, limit 1-100 and nonnegative offset".into(),
        ));
    }
    let brand = q
        .brand
        .map(|s| validate::non_empty("brand", &s, 80))
        .transpose()?;
    let topic = q
        .topic
        .map(|s| validate::non_empty("topic", &s, 120))
        .transpose()?;
    if brand
        .as_deref()
        .is_some_and(|s| s.chars().any(char::is_control))
        || topic
            .as_deref()
            .is_some_and(|s| s.chars().any(char::is_control))
    {
        return Err(AppError::Validation(
            "search terms cannot contain control characters".into(),
        ));
    }
    let pattern = brand.as_deref().map(literal_pattern);
    let result = db::ai_index::evidence(
        pool,
        IndexFilter {
            org: a.org_id,
            pid,
            country: &country,
            language: &language,
            topic: topic.as_deref(),
            pattern: pattern.as_deref(),
            view: &view,
            limit: q.limit.unwrap_or(25),
            offset: q.offset.unwrap_or(0),
        },
    )
    .await?;
    let rate = result
        .summary
        .brand_mentions
        .filter(|_| result.summary.successful_answers > 0)
        .map(|n| 100.0 * n as f64 / result.summary.successful_answers as f64);
    Ok(AiOwnIndex{country,language,view,brand,topic,summary:result.summary,mention_rate_percent:rate,total_matching:result.total_matching,prompts:result.prompts,answers:result.answers,
        warnings:vec!["Searches only retained model-answer evidence for this project's current website and selected market. It is not global prompt demand, consumer chat visibility or a recommendation score.".into(),"Literal case-insensitive brand matching uses alphanumeric boundaries. Failed/unattempted cells are not brand absence; rates use successful saved answers only. Topic filters search the saved prompt text.".into(),"Run/model gateway fingerprints and grounding labels remain separate evidence. Aliases and provider behavior can change; text-only answers are not web search measurements.".into()]})
}
fn literal_pattern(term: &str) -> String {
    let mut escaped = String::new();
    for c in term.chars() {
        if ".^$*+?()[]{}|\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("(^|[^[:alnum:]])({escaped})($|[^[:alnum:]])")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brand_input_is_literal_not_a_regular_expression() {
        assert_eq!(
            literal_pattern("C++ [Acme]."),
            r"(^|[^[:alnum:]])(C\+\+ \[Acme\]\.)($|[^[:alnum:]])"
        );
        assert!(literal_pattern(".*").contains(r"\.\*"));
    }
}
