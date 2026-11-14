//! Explicit BYOK historical advertising metrics; never estimate CPC from free SEO signals.
use crate::{Actor, Role};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CpcInput {
    pub keywords: Vec<String>,
    /// Google geo target constant ID, e.g. 2840. Explicit to avoid guessing market.
    pub geo_target_id: String,
    /// Google language constant ID, e.g. 1000.
    pub language_id: String,
    /// Expected account currency, verified with Google before returning measured micros.
    pub currency: String,
}
#[derive(Serialize, ToSchema)]
pub struct CpcRow {
    pub keyword: String,
    pub average_cpc_micros: Option<i64>,
    pub competition: Option<String>,
    pub competition_index: Option<i64>,
    pub average_monthly_searches: Option<i64>,
}
#[derive(Serialize, ToSchema)]
pub struct CpcView {
    pub source: String,
    pub available: bool,
    pub currency: String,
    pub rows: Vec<CpcRow>,
    pub note: String,
}
async fn bounded(request: reqwest::RequestBuilder) -> AppResult<Value> {
    let mut r = request
        .send()
        .await
        .map_err(|_| AppError::Unavailable("Google Ads request failed".into()))?;
    if !r.status().is_success() {
        return Err(AppError::Unavailable(format!(
            "Google Ads returned HTTP {}; check operator credentials, access and quota",
            r.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = r
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("Google response interrupted".into()))?
    {
        if bytes.len() + chunk.len() > 512 * 1024 {
            return Err(AppError::Unavailable(
                "Google response exceeded 512 KiB".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Unavailable("Google returned invalid JSON".into()))
}
fn number(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| *n >= 0)
}
pub async fn metrics(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    v: CpcInput,
) -> AppResult<CpcView> {
    actor.require(Role::Admin)?;
    db::projects::get(pool, actor.org_id, pid).await?;
    let digits = |s: &str| !s.is_empty() && s.len() <= 12 && s.bytes().all(|c| c.is_ascii_digit());
    if v.keywords.is_empty()
        || v.keywords.len() > 50
        || v.keywords
            .iter()
            .any(|k| k.trim().is_empty() || k.len() > 240)
        || !digits(&v.geo_target_id)
        || !digits(&v.language_id)
        || v.currency.len() != 3
        || !v.currency.bytes().all(|c| c.is_ascii_uppercase())
    {
        return Err(AppError::Validation("pass 1–50 keywords, numeric Google geo/language IDs and a three-letter account currency".into()));
    }
    let configured = (
        cfg.google_ads_developer_token.as_deref(),
        cfg.google_ads_customer_id.as_deref(),
        cfg.google_ads_refresh_token.as_deref(),
        cfg.google_client_id.as_deref(),
        cfg.google_client_secret.as_deref(),
    );
    let (Some(developer), Some(customer), Some(refresh), Some(client_id), Some(client_secret)) =
        configured
    else {
        return Ok(CpcView{source:"google_ads".into(),available:false,currency:v.currency,rows:v.keywords.into_iter().map(|keyword|CpcRow{keyword,average_cpc_micros:None,competition:None,competition_index:None,average_monthly_searches:None}).collect(),note:"Unknown: operator Google Ads BYOK credentials are not configured. Free SEO research remains available.".into()});
    };
    if !digits(customer)
        || !cfg.google_ads_api_version.starts_with('v')
        || !digits(&cfg.google_ads_api_version[1..])
    {
        return Err(AppError::Conflict(
            "invalid operator Google Ads customer/version configuration".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(crawler::DEFAULT_USER_AGENT)
        .build()
        .map_err(AppError::internal)?;
    let form = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh),
            ("grant_type", "refresh_token"),
        ])
        .finish();
    let token = bounded(
        client
            .post("https://oauth2.googleapis.com/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form),
    )
    .await?;
    let access = token["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Unavailable("Google did not return an access token".into()))?;
    let mut account = client
        .post(format!(
            "https://googleads.googleapis.com/{}/customers/{customer}/googleAds:search",
            cfg.google_ads_api_version
        ))
        .bearer_auth(access)
        .header("developer-token", developer);
    if let Some(login) = &cfg.google_ads_login_customer_id {
        if !digits(login) {
            return Err(AppError::Conflict(
                "invalid Google Ads login customer ID".into(),
            ));
        }
        account = account.header("login-customer-id", login);
    }
    let account = bounded(
        account.json(&json!({"query":"SELECT customer.currency_code FROM customer LIMIT 1"})),
    )
    .await?;
    let currency = account["results"][0]["customer"]["currencyCode"]
        .as_str()
        .ok_or_else(|| {
            AppError::Unavailable("Google did not return the account currency".into())
        })?;
    if currency != v.currency {
        return Err(AppError::Validation(format!(
            "account currency is {currency}; use that currency before requesting metrics"
        )));
    }
    let mut request=client.post(format!("https://googleads.googleapis.com/{}/customers/{customer}:generateKeywordHistoricalMetrics",cfg.google_ads_api_version)).bearer_auth(access).header("developer-token",developer);
    if let Some(login) = &cfg.google_ads_login_customer_id {
        if !digits(login) {
            return Err(AppError::Conflict(
                "invalid Google Ads login customer ID".into(),
            ));
        }
        request = request.header("login-customer-id", login);
    }
    let data=bounded(request.json(&json!({"keywords":v.keywords,"geoTargetConstants":[format!("geoTargetConstants/{}",v.geo_target_id)],"language":format!("languageConstants/{}",v.language_id),"keywordPlanNetwork":"GOOGLE_SEARCH","historicalMetricsOptions":{"includeAverageCpc":true}}))).await?;
    let results = data["results"]
        .as_array()
        .ok_or_else(|| AppError::Unavailable("Google Ads omitted historical results".into()))?;
    let rows = decode_rows(v.keywords, results);
    Ok(CpcView{source:"google_ads".into(),available:true,currency:currency.into(),rows,note:"Google historical advertising metrics; close variants may be combined. Currency verified against the Google Ads account. Micros are one millionth of a currency unit. Advertising competition is not organic keyword difficulty. Missing values stay null.".into()})
}
fn decode_rows(keywords: Vec<String>, results: &[Value]) -> Vec<CpcRow> {
    keywords
        .into_iter()
        .map(|keyword| {
            let r = results.iter().find(|r| {
                r["text"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&keyword))
                    || r["closeVariants"].as_array().is_some_and(|a| {
                        a.iter()
                            .any(|s| s.as_str().is_some_and(|s| s.eq_ignore_ascii_case(&keyword)))
                    })
            });
            let m = r.map(|r| &r["keywordMetrics"]).unwrap_or(&Value::Null);
            CpcRow {
                keyword,
                average_cpc_micros: number(&m["averageCpcMicros"]),
                competition: m["competition"]
                    .as_str()
                    .filter(|s| !matches!(*s, "UNKNOWN" | "UNSPECIFIED"))
                    .map(str::to_string),
                competition_index: number(&m["competitionIndex"]),
                average_monthly_searches: number(&m["avgMonthlySearches"]),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_metrics_keep_missing_unknown_and_match_close_variants() {
        let results = vec![
            json!({"text":"coffee","closeVariants":["coffees"],"keywordMetrics":{"averageCpcMicros":"1234567","competition":"HIGH","competitionIndex":"76","avgMonthlySearches":"1000"}}),
            json!({"text":"tea","keywordMetrics":{"competition":"UNKNOWN"}}),
        ];
        let rows = decode_rows(
            vec!["coffees".into(), "tea".into(), "missing".into()],
            &results,
        );
        assert_eq!(rows[0].average_cpc_micros, Some(1234567));
        assert_eq!(rows[0].competition_index, Some(76));
        assert!(rows[1].competition.is_none());
        assert!(rows[2].average_cpc_micros.is_none());
        assert!(number(&json!("-1")).is_none());
    }
}
