//! Source-labelled city demand and observed Google AI Overviews, never synthetic answers.
use crate::{Actor, Role};
use common::{AppError, AppResult, Config};
use db::observations::{self as repo, ResearchObservation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureOverviewInput {
    pub keyword: String,
    pub location: Option<String>,
    /// Google search plus at most one token continuation. Cached observations make no requests.
    pub expected_max_requests: u8,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportOverviewInput {
    pub keyword: String,
    pub location: Option<String>,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub source_note: String,
    /// SerpApi-format Google search response; only normalized text and safe citations are retained.
    pub response: Value,
}

fn keyword(value: &str) -> AppResult<String> {
    crate::validate::non_empty("keyword", &scoring::normalize_keyword(value), 120)
}
fn location(value: Option<&str>) -> AppResult<Option<String>> {
    value
        .map(|s| crate::validate::non_empty("location", s, 160))
        .transpose()
}
fn website(p: &db::models::Project) -> AppResult<()> {
    if p.domain.is_none() {
        return Err(AppError::Conflict(
            "set a project website before saving research".into(),
        ));
    }
    Ok(())
}
fn past(value: chrono::DateTime<chrono::Utc>) -> AppResult<()> {
    if value > chrono::Utc::now() + chrono::Duration::minutes(5) {
        return Err(AppError::Validation(
            "observed_at must not be in the future".into(),
        ));
    }
    Ok(())
}
pub async fn overview_history(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Vec<ResearchObservation>> {
    let p = db::projects::get(pool, a.org_id, pid).await?;
    repo::list(pool, &p, "ai_overview").await
}
pub async fn import_overview(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    input: ImportOverviewInput,
) -> AppResult<ResearchObservation> {
    a.require(Role::Member)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    website(&p)?;
    let keyword = keyword(&input.keyword)?;
    let location = location(input.location.as_deref())?;
    past(input.observed_at)?;
    let note = crate::validate::non_empty("source_note", &input.source_note, 300)?;
    let overview =
        providers::serpapi::overview(&input.response).map_err(jobs::provider_ctx::provider_err)?;
    repo::save(
        pool,
        &p,
        "ai_overview",
        json!({"keyword":keyword,"location":location,"provider":"imported_google_search",
        "observed_at":input.observed_at,"source_note":note,"overview":overview}),
    )
    .await
}
#[derive(Serialize, ToSchema)]
pub struct OverviewCapture {
    pub observation: ResearchObservation,
    pub cached: bool,
    pub requests: u8,
}
pub async fn capture_overview(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    input: CaptureOverviewInput,
) -> AppResult<OverviewCapture> {
    a.require(Role::Member)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    website(&p)?;
    let keyword = keyword(&input.keyword)?;
    let location = location(input.location.as_deref())?;
    if input.expected_max_requests != 2 {
        return Err(AppError::Conflict(
            "review the maximum budget of two provider requests".into(),
        ));
    }
    for observation in repo::list(pool, &p, "ai_overview").await? {
        if observation.data["keyword"] == keyword
            && observation.data["location"] == json!(location)
            && observation.data["provider"] == "serpapi_google"
            && observation.created_at > chrono::Utc::now() - chrono::Duration::hours(24)
        {
            return Ok(OverviewCapture {
                observation,
                cached: true,
                requests: 0,
            });
        }
    }
    let key = crate::local_scans::provider_key(pool, a, cfg).await?;
    let claim = format!(
        "{}:{}:{}:{}:{}:{}",
        pid,
        p.domain.as_deref().unwrap_or_default(),
        p.country_code,
        p.language_code,
        keyword,
        location.as_deref().unwrap_or_default()
    );
    if !db::providers::claim_lookup(pool, "aio_capture_v1", &common::token::sha256_hex(&claim))
        .await?
    {
        return Err(AppError::Conflict(
            "this keyword was recently requested; retry after the cooldown".into(),
        ));
    }
    let mut params = vec![
        ("engine", "google".into()),
        ("q", keyword.clone()),
        ("gl", p.country_code.to_lowercase()),
        ("hl", p.language_code.clone()),
    ];
    if let Some(l) = &location {
        params.push(("location", l.clone()));
    }
    let mut response = providers::serpapi::request(&cfg.serpapi_url, &key, &params)
        .await
        .map_err(jobs::provider_ctx::provider_err)?;
    let mut requests = 1;
    if let Some(token) = response
        .pointer("/ai_overview/page_token")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        if token.len() > 16_384 {
            return Err(AppError::Unavailable(
                "invalid AI Overview continuation".into(),
            ));
        }
        response = providers::serpapi::request(
            &cfg.serpapi_url,
            &key,
            &[
                ("engine", "google_ai_overview".into()),
                ("page_token", token),
            ],
        )
        .await
        .map_err(jobs::provider_ctx::provider_err)?;
        requests = 2;
    }
    let overview =
        providers::serpapi::overview(&response).map_err(jobs::provider_ctx::provider_err)?;
    let observation=repo::save(pool,&p,"ai_overview",json!({"keyword":keyword,"location":location,
        "provider":"serpapi_google","observed_at":chrono::Utc::now(),"source_note":"Google search AI Overview via SerpApi; citations are reported links, not verified source support.","overview":overview})).await?;
    Ok(OverviewCapture {
        observation,
        cached: false,
        requests,
    })
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CityDemandRow {
    pub keyword: String,
    /// Explicit imported city searches/month; not inferred by multiplying national volume.
    pub monthly_searches: Option<i64>,
    pub national_monthly_searches: Option<i64>,
    /// Source-exported normalized city interest, 0–100. This is not search volume.
    pub relative_interest: Option<f64>,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportCityDemandInput {
    pub city: String,
    pub source_note: String,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
    pub rows: Vec<CityDemandRow>,
}
pub async fn city_history(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Vec<ResearchObservation>> {
    let p = db::projects::get(pool, a.org_id, pid).await?;
    repo::list(pool, &p, "city_demand").await
}
pub async fn import_city(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    input: ImportCityDemandInput,
) -> AppResult<ResearchObservation> {
    a.require(Role::Member)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    website(&p)?;
    let city = crate::validate::non_empty("city", &input.city, 160)?;
    let note = crate::validate::non_empty("source_note", &input.source_note, 300)?;
    if input.period_start > input.period_end
        || input.period_end > chrono::Utc::now().date_naive()
        || input.rows.is_empty()
        || input.rows.len() > 500
    {
        return Err(AppError::Validation(
            "use a past period and 1–500 keyword rows".into(),
        ));
    }
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in input.rows {
        let keyword = keyword(&r.keyword)?;
        if !seen.insert(keyword.clone())
            || r.monthly_searches
                .is_some_and(|n| !(0..=1_000_000_000).contains(&n))
            || r.national_monthly_searches
                .is_some_and(|n| !(0..=1_000_000_000).contains(&n))
            || r.relative_interest
                .is_some_and(|n| !n.is_finite() || !(0.0..=100.0).contains(&n))
            || (r.monthly_searches.is_none() && r.relative_interest.is_none())
        {
            return Err(AppError::Validation(
                "use unique keywords with valid city volume or 0–100 relative interest".into(),
            ));
        }
        rows.push(json!({"keyword":keyword,"monthly_searches":r.monthly_searches,
            "national_monthly_searches":r.national_monthly_searches,"relative_interest":r.relative_interest}));
    }
    repo::save(pool,&p,"city_demand",json!({"city":city,"source_note":note,"period_start":input.period_start,"period_end":input.period_end,
        "rows":rows,"note":"Imported city context. Relative interest is normalized within the source export and is not searches/month. Missing values remain unknown; national volume is not scaled into a city estimate."})).await
}
