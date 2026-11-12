use crate::{Actor, Role, validate};
use common::{AppError, AppResult, Config};
use db::ai_visibility::{self as repo, AiTrackerConfig, AiVisibilityRun, AiVisibilityTracker};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct AiModelAvailability {
    pub ready: bool,
    pub models: Vec<String>,
    pub note: Option<String>,
}
pub fn models(cfg: &Config) -> AiModelAvailability {
    match jobs::ai_visibility::gateway(cfg) {
        Ok(g) => AiModelAvailability {
            ready: true,
            models: g.models().to_vec(),
            note: None,
        },
        Err(e) => AiModelAvailability {
            ready: false,
            models: vec![],
            note: Some(e.to_string()),
        },
    }
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAiTrackerInput {
    pub expected_domain: String,
    pub name: String,
    pub brands: Vec<String>,
    pub prompts: Vec<String>,
    pub models: Vec<String>,
    pub country: Option<String>,
    pub language: Option<String>,
}
fn strings(
    field: &str,
    raw: &[String],
    max_items: usize,
    max_chars: usize,
) -> AppResult<Vec<String>> {
    if raw.is_empty() || raw.len() > max_items {
        return Err(AppError::Validation(format!(
            "{field} requires 1-{max_items} entries"
        )));
    }
    let mut out = Vec::new();
    for item in raw {
        let item = validate::non_empty(field, item, max_chars)?;
        if item
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(AppError::Validation(format!(
                "{field} contains control characters"
            )));
        }
        if out
            .iter()
            .any(|old: &String| old.to_lowercase() == item.to_lowercase())
        {
            return Err(AppError::Validation(format!(
                "{field} contains duplicate entries"
            )));
        }
        out.push(item);
    }
    Ok(out)
}
pub async fn create(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    input: CreateAiTrackerInput,
) -> AppResult<AiVisibilityTracker> {
    a.require(Role::Admin)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    let name = validate::non_empty("tracker name", &input.name, 120)?;
    let brands = strings("brand terms", &input.brands, 10, 80)?;
    let prompts = strings("prompts", &input.prompts, 10, 500)?;
    let selected = strings("models", &input.models, 3, 120)?;
    let gateway = jobs::ai_visibility::gateway(cfg)?;
    if selected.iter().any(|m| !gateway.models().contains(m)) {
        return Err(AppError::Validation(
            "Select only operator-approved models".into(),
        ));
    }
    let config = AiTrackerConfig {
        brands,
        prompts,
        models: selected,
        country: validate::country(input.country.as_deref().unwrap_or(&p.country_code))?,
        language: validate::language(input.language.as_deref().unwrap_or(&p.language_code))?,
    };
    repo::create(pool, a.org_id, pid, &input.expected_domain, &name, &config).await
}
pub async fn list(pool: &PgPool, a: &Actor, pid: Uuid) -> AppResult<Vec<AiVisibilityTracker>> {
    db::projects::get(pool, a.org_id, pid).await?;
    repo::reconcile(pool, a.org_id, pid).await?;
    repo::list(pool, a.org_id, pid).await
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartAiRunInput {
    pub expected_domain: String,
}
pub async fn start(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    cfg: &Config,
    input: StartAiRunInput,
) -> AppResult<AiVisibilityRun> {
    a.require(Role::Admin)?;
    let t = repo::tracker(pool, a.org_id, pid, tid).await?;
    let gateway = jobs::ai_visibility::gateway(cfg)?;
    if t.config
        .models
        .iter()
        .any(|m| !gateway.models().contains(m))
    {
        return Err(AppError::Conflict(
            "An approved model changed; create a new tracker with current models".into(),
        ));
    }
    repo::start(
        pool,
        a.org_id,
        pid,
        tid,
        &input.expected_domain,
        &jobs::ai_visibility::gateway_id(&gateway),
    )
    .await
}
#[derive(Serialize, ToSchema)]
pub struct AiTrackerView {
    pub tracker: AiVisibilityTracker,
    pub runs: Vec<AiVisibilityRun>,
}
pub async fn get(pool: &PgPool, a: &Actor, pid: Uuid, tid: Uuid) -> AppResult<AiTrackerView> {
    let tracker = repo::tracker(pool, a.org_id, pid, tid).await?;
    let runs = repo::runs(pool, a.org_id, pid, tid).await?;
    Ok(AiTrackerView { tracker, runs })
}
#[derive(Serialize, ToSchema)]
pub struct AiAnswerView {
    pub prompt_index: i32,
    pub model: String,
    pub answer: Option<String>,
    pub mentions: Option<Vec<String>>,
    pub sources: Option<Vec<providers::ai_visibility::AiReportedSource>>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct AiBrandRate {
    pub brand: String,
    pub mentioned_answers: usize,
    pub mention_rate_pct: Option<f64>,
}
#[derive(Serialize, ToSchema)]
pub struct AiRunView {
    pub run: AiVisibilityRun,
    pub answers: Vec<AiAnswerView>,
    pub expected_answers: usize,
    pub successful_answers: usize,
    pub failed_answers: usize,
    pub brand_rates: Vec<AiBrandRate>,
    pub source: &'static str,
    pub confidence: &'static str,
    pub note: &'static str,
}
fn rates(brands: &[String], answers: &[AiAnswerView]) -> Vec<AiBrandRate> {
    let successful = answers
        .iter()
        .filter(|a| a.answer.is_some() && a.error.is_none())
        .count();
    brands
        .iter()
        .map(|brand| {
            let mentioned_answers = answers
                .iter()
                .filter(|a| {
                    a.answer.is_some()
                        && a.error.is_none()
                        && a.mentions.as_ref().is_some_and(|m| m.contains(brand))
                })
                .count();
            AiBrandRate {
                brand: brand.clone(),
                mentioned_answers,
                mention_rate_pct: (successful > 0)
                    .then(|| 100.0 * mentioned_answers as f64 / successful as f64),
            }
        })
        .collect()
}
pub async fn run(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    rid: Uuid,
) -> AppResult<AiRunView> {
    let view = get(pool, a, pid, tid).await?;
    let run = view
        .runs
        .into_iter()
        .find(|r| r.id == rid)
        .ok_or(AppError::NotFound("AI run"))?;
    let rows = repo::answers(pool, rid).await?;
    let mut answers = Vec::new();
    for row in rows {
        answers.push(AiAnswerView {
            prompt_index: row.prompt_index,
            model: row.model,
            answer: row.answer,
            mentions: row
                .mentions
                .map(serde_json::from_value)
                .transpose()
                .map_err(AppError::internal)?,
            sources: row
                .sources
                .map(serde_json::from_value)
                .transpose()
                .map_err(AppError::internal)?,
            error: row.error,
            created_at: row.created_at,
        });
    }
    let brand_rates = rates(&run.config.brands, &answers);
    let expected_answers = run.config.models.len() * run.config.prompts.len();
    let successful_answers = answers
        .iter()
        .filter(|a| a.answer.is_some() && a.error.is_none())
        .count();
    let failed_answers = answers.iter().filter(|a| a.error.is_some()).count();
    Ok(AiRunView {
        run,
        answers,
        expected_answers,
        successful_answers,
        failed_answers,
        brand_rates,
        source: "model_api",
        confidence: "relative",
        note: "API answer sample without web-search tools. Literal case-insensitive brand-term matches are not endorsements. Rates use successful answers only; failed/unattempted cells are unknown. Reported URLs are unverified model output, not confirmed citations. Multiple brands may match one answer; rates do not sum to 100%.",
    })
}
pub async fn delete(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    rid: Option<Uuid>,
) -> AppResult<()> {
    a.require(Role::Admin)?;
    repo::delete(pool, a.org_id, pid, tid, rid).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicates_and_unknown_denominators_are_explicit() {
        assert!(strings("brands", &["Acme".into(), "acme".into()], 10, 80).is_err());
        let empty = rates(&["Acme".into()], &[]);
        assert!(empty[0].mention_rate_pct.is_none());
        let answers = vec![
            AiAnswerView {
                prompt_index: 0,
                model: "m".into(),
                answer: Some("No matching brand".into()),
                mentions: Some(vec![]),
                sources: Some(vec![]),
                error: None,
                created_at: chrono::Utc::now(),
            },
            AiAnswerView {
                prompt_index: 1,
                model: "m".into(),
                answer: None,
                mentions: None,
                sources: None,
                error: Some("failed".into()),
                created_at: chrono::Utc::now(),
            },
        ];
        assert_eq!(
            rates(&["Acme".into()], &answers)[0].mention_rate_pct,
            Some(0.0)
        );
    }
}
