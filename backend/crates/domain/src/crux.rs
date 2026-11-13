//! Optional public Chrome real-user field data, never a Lighthouse substitute.
use crate::Actor;
use chrono::{DateTime, NaiveDate, Utc};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const METRICS: &[(&str, &str)] = &[
    ("largest_contentful_paint", "ms"),
    ("interaction_to_next_paint", "ms"),
    ("cumulative_layout_shift", "unitless"),
];

#[derive(Debug, Deserialize, ToSchema)]
pub struct CruxInput {
    /// Exact HTTP(S) URL on the project host (or its www equivalent).
    pub url: String,
    /// url or origin. There is no automatic origin fallback.
    pub scope: String,
    /// PHONE, DESKTOP, TABLET or ALL.
    pub form_factor: String,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CruxMetric {
    pub name: String,
    pub unit: String,
    pub p75: Option<f64>,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CruxRecord {
    pub target: String,
    pub scope: String,
    pub form_factor: String,
    pub status: String,
    pub checked_at: DateTime<Utc>,
    pub first_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
    /// The provider's returned key; may differ because of URL normalization.
    pub returned_target: Option<String>,
    pub metrics: Vec<CruxMetric>,
    pub note: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CruxResult {
    pub record: CruxRecord,
    pub cached: bool,
}

fn target(input: &CruxInput, domain: &str) -> AppResult<String> {
    let u = url::Url::parse(input.url.trim())
        .map_err(|_| AppError::Validation("enter an absolute HTTP(S) page URL".into()))?;
    let host = u.host_str().unwrap_or("");
    if input.url.len() > 2048
        || !["http", "https"].contains(&u.scheme())
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
        || host.trim_start_matches("www.") != domain.trim_start_matches("www.")
        || !["url", "origin"].contains(&input.scope.as_str())
        || !["PHONE", "DESKTOP", "TABLET", "ALL"].contains(&input.form_factor.as_str())
        || (input.scope == "origin" && u.path() != "/")
    {
        return Err(AppError::Validation("use the project host or its www equivalent, no credentials, query, fragment or custom port; scope url/origin, form factor PHONE/DESKTOP/TABLET/ALL; origin requires a root URL".into()));
    }
    Ok(if input.scope == "origin" {
        u.origin().ascii_serialization()
    } else {
        u.to_string()
    })
}

pub async fn lookup(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    input: CruxInput,
) -> AppResult<CruxResult> {
    let project = db::projects::get(pool, actor.org_id, pid).await?;
    let domain = project
        .domain
        .ok_or_else(|| AppError::Validation("set the project website first".into()))?;
    let target = target(&input, &domain)?;
    let mut record = CruxRecord {
        target, scope: input.scope, form_factor: input.form_factor,
        status: "not_configured".into(), checked_at: Utc::now(), first_date: None, last_date: None,
        returned_target: None,
        metrics: METRICS.iter().map(|(n,u)| CruxMetric{name:(*n).into(),unit:(*u).into(),p75:None}).collect(),
        note: "Ask the server operator to set CRUX_API_KEY with a free Chrome UX Report API key. Field data remains unknown.".into(),
    };
    let Some(secret) = cfg.crux_api_key.as_deref().filter(|s| !s.trim().is_empty()) else {
        return Ok(CruxResult {
            record,
            cached: false,
        });
    };
    // Public records can be shared, but isolate operator endpoint changes and device/scope.
    let key = common::token::sha256_hex(&format!(
        "{}|{}|{}|{}",
        cfg.crux_api_url, record.scope, record.target, record.form_factor
    ));
    if let Some(value) = db::providers::cache_get(pool, "crux_field_v1", &key, 1).await?
        && let Ok(cached) = serde_json::from_value::<CruxRecord>(value)
    {
        return Ok(CruxResult {
            record: cached,
            cached: true,
        });
    }
    if !db::providers::claim_lookup(pool, "crux_project_claim_v1", &pid.to_string()).await?
        || !db::providers::claim_lookup(pool, "crux_target_claim_v1", &key).await?
    {
        return Err(AppError::Conflict(
            "uncached CrUX lookups are limited to one per project and target per minute".into(),
        ));
    }
    let mut body = json!({"metrics":METRICS.iter().map(|(n,_)| *n).collect::<Vec<_>>()});
    body[&record.scope] = json!(record.target);
    if record.form_factor != "ALL" {
        body["formFactor"] = json!(record.form_factor);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(providers::USER_AGENT)
        .build()
        .map_err(AppError::internal)?;
    let mut endpoint = url::Url::parse(&cfg.crux_api_url)
        .map_err(|_| AppError::Unavailable("invalid operator CrUX endpoint".into()))?;
    endpoint.query_pairs_mut().append_pair("key", secret);
    let mut res = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|_| AppError::Unavailable("CrUX could not be reached".into()))?;
    match res.status().as_u16() {
        404 => {
            record.status = "no_data".into();
            record.note="CrUX has no published data for this exact scope and device. Insufficient coverage is unknown, not a performance score; no origin fallback was requested.".into();
        }
        200 => {
            let mut bytes = Vec::new();
            while let Some(chunk) = res
                .chunk()
                .await
                .map_err(|_| AppError::Unavailable("CrUX response interrupted".into()))?
            {
                if bytes.len() + chunk.len() > 262144 {
                    return Err(AppError::Unavailable(
                        "CrUX response exceeds the size limit".into(),
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| AppError::Unavailable("CrUX returned invalid JSON".into()))?;
            fill(&mut record, &value)?;
        }
        401 | 403 => {
            return Err(AppError::Unavailable(
                "CrUX refused access; check CRUX_API_KEY and enable Chrome UX Report API".into(),
            ));
        }
        429 => {
            return Err(AppError::Unavailable(
                "CrUX quota reached; retry later".into(),
            ));
        }
        _ => {
            return Err(AppError::Unavailable(
                "CrUX returned an error; verify the requested URL and retry later".into(),
            ));
        }
    }
    db::providers::cache_put(
        pool,
        "crux_field_v1",
        &key,
        &serde_json::to_value(&record).map_err(AppError::internal)?,
    )
    .await?;
    Ok(CruxResult {
        record,
        cached: false,
    })
}

fn date(v: &Value) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(
        i32::try_from(v["year"].as_i64()?).ok()?,
        u32::try_from(v["month"].as_u64()?).ok()?,
        u32::try_from(v["day"].as_u64()?).ok()?,
    )
}
fn fill(record: &mut CruxRecord, value: &Value) -> AppResult<()> {
    let r = &value["record"];
    if !r["metrics"].is_object() || !r["key"].is_object() {
        return Err(AppError::Unavailable(
            "CrUX returned an invalid record".into(),
        ));
    }
    let returned = r["key"][&record.scope]
        .as_str()
        .filter(|s| s.len() <= 2048)
        .ok_or_else(|| AppError::Unavailable("CrUX returned an unexpected scope".into()))?;
    if r["key"]["formFactor"].as_str().unwrap_or("ALL") != record.form_factor {
        return Err(AppError::Unavailable(
            "CrUX returned an unexpected device".into(),
        ));
    }
    // Keep provider normalization visible; do not interpret it as a redirect crawl.
    record.returned_target = Some(returned.into());
    record.first_date = date(&r["collectionPeriod"]["firstDate"]);
    record.last_date = date(&r["collectionPeriod"]["lastDate"]);
    for m in &mut record.metrics {
        let p = &r["metrics"][&m.name]["percentiles"]["p75"];
        m.p75 = p
            .as_f64()
            .or_else(|| p.as_str().and_then(|s| s.parse::<f64>().ok()))
            .filter(|n| n.is_finite() && *n >= 0.0);
    }
    record.status = "available".into();
    record.note="Aggregated Chrome real-user field data over the reported collection period; missing metrics remain unknown. This is independent of Lighthouse lab results and audit health scores.".into();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_validation_prevents_silent_fallback_and_cross_host() {
        let mut i = CruxInput {
            url: "https://www.example.com/page".into(),
            scope: "url".into(),
            form_factor: "PHONE".into(),
        };
        assert!(target(&i, "example.com").is_ok());
        for s in [
            "https://example.com.evil.test/",
            "https://a.example.com/",
            "https://user@example.com/",
            "https://example.com/?token=x",
            "https://example.com:1234/",
            "https://example.com/#x",
        ] {
            i.url = s.into();
            assert!(target(&i, "example.com").is_err());
        }
        i.url = "https://example.com/page".into();
        i.scope = "origin".into();
        assert!(target(&i, "example.com").is_err());
        i.url = "https://example.com/".into();
        assert_eq!(target(&i, "example.com").unwrap(), "https://example.com");
    }
    #[test]
    fn p75_accepts_cls_strings_and_preserves_missing() {
        let mut r = CruxRecord {
            target: "https://example.com".into(),
            scope: "origin".into(),
            form_factor: "ALL".into(),
            status: String::new(),
            checked_at: Utc::now(),
            first_date: None,
            last_date: None,
            returned_target: None,
            metrics: METRICS
                .iter()
                .map(|(n, u)| CruxMetric {
                    name: (*n).into(),
                    unit: (*u).into(),
                    p75: None,
                })
                .collect(),
            note: String::new(),
        };
        fill(&mut r,&json!({"record":{"key":{"origin":"https://example.com"},"metrics":{"largest_contentful_paint":{"percentiles":{"p75":2500}},"cumulative_layout_shift":{"percentiles":{"p75":"0.12"}}},"collectionPeriod":{"firstDate":{"year":2026,"month":10,"day":1},"lastDate":{"year":2026,"month":10,"day":28}}}})).unwrap();
        assert_eq!(r.metrics[0].p75, Some(2500.0));
        assert_eq!(r.metrics[1].p75, None);
        assert_eq!(r.metrics[2].p75, Some(0.12));
        assert_eq!(r.first_date.unwrap().to_string(), "2026-10-01");
        assert!(
            fill(
                &mut r,
                &json!({"record":{"key":{"url":"https://example.com"},"metrics":{}}})
            )
            .is_err()
        );
    }
}
