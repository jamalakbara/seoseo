//! Read-only Analytics integration. All entry points authorize the project first.
use crate::{Actor, ActorVia, Role};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, NaiveDate, Utc};
use common::{AppError, AppResult, Config, crypto::SecretBox, token};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const SCOPE: &str = "https://www.googleapis.com/auth/analytics.readonly";
const RECONNECT: &str =
    "Google access expired or was revoked; reconnect in this project's Analytics tab";

async fn authorize(pool: &PgPool, actor: &Actor, project: Uuid, write: bool) -> AppResult<()> {
    let p = db::projects::get(pool, actor.org_id, project).await?;
    if write {
        actor.require(Role::Admin)?;
        if p.archived_at.is_some() {
            return Err(AppError::Conflict(
                "Restore this project before changing its Google connection".into(),
            ));
        }
    }
    Ok(())
}

fn credentials(cfg: &Config) -> AppResult<(&str, &str, SecretBox)> {
    let Some(id) = cfg.google_client_id.as_deref().filter(|v| !v.is_empty()) else {
        return Err(AppError::Conflict(
            "Set GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET and APP_ENCRYPTION_KEY on the server first"
                .into(),
        ));
    };
    let secret = cfg
        .google_client_secret
        .as_deref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Conflict("Set GOOGLE_CLIENT_SECRET on the server first".into()))?;
    let key = cfg
        .app_encryption_key
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Set APP_ENCRYPTION_KEY on the server first".into()))?;
    Ok((
        id,
        secret,
        SecretBox::from_base64(key).map_err(|e| AppError::Conflict(e.to_string()))?,
    ))
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("SeoSeo/0.1 (Analytics integration)")
        .build()
        .map_err(AppError::internal)
}

// Never expose Google's token response or transport errors (which can contain URLs).
async fn response(req: reqwest::RequestBuilder, oauth: bool) -> AppResult<Value> {
    let mut res = req.send().await.map_err(|_| {
        AppError::Unavailable("Google could not be reached; try again shortly".into())
    })?;
    if !res.status().is_success() {
        return Err(match res.status().as_u16() {
            401 => AppError::Conflict(RECONNECT.into()),
            400 if oauth => AppError::Conflict(RECONNECT.into()),
            403 => AppError::Forbidden("Google refused access; check property permissions and that the Analytics API is enabled".into()),
            429 => AppError::Unavailable("Google quota reached; wait before trying again".into()),
            400 | 404 => AppError::Validation("Google could not process this property, query or URL; check the inputs".into()),
            _ => AppError::Unavailable("Google returned an error; try again shortly".into()),
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("Google response was interrupted".into()))?
    {
        if body.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err(AppError::Unavailable(
                "Google response is too large; request fewer rows".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .map_err(|_| AppError::Unavailable("Google returned an invalid response".into()))
}

async fn exchange(cfg: &Config, fields: &[(&str, &str)]) -> AppResult<Value> {
    let (id, secret, _) = credentials(cfg)?;
    let form = {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("client_id", id)
            .append_pair("client_secret", secret);
        for (k, v) in fields {
            form.append_pair(k, v);
        }
        form.finish()
    };
    response(
        client()?
            .post(&cfg.google_token_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form),
        true,
    )
    .await
}

fn redirect_uri(cfg: &Config) -> String {
    format!(
        "{}/api/v1/ga4/oauth/callback",
        cfg.web_base_url.trim_end_matches('/')
    )
}

#[derive(Serialize, ToSchema)]
pub struct Ga4ConnectionView {
    pub configured: bool,
    pub connected: bool,
    pub connected_by: Option<Uuid>,
    pub property_id: Option<String>,
}

pub async fn status(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<Ga4ConnectionView> {
    authorize(pool, actor, project, false).await?;
    let c = db::ga4::connection(pool, actor.org_id, project).await?;
    Ok(Ga4ConnectionView {
        configured: credentials(cfg).is_ok(),
        connected: c.is_some(),
        connected_by: c.as_ref().map(|c| c.connected_by),
        property_id: c.and_then(|c| c.property_id),
    })
}

#[derive(Serialize, ToSchema)]
pub struct Ga4ConnectUrl {
    pub url: String,
}

pub async fn connect(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<Ga4ConnectUrl> {
    authorize(pool, actor, project, true).await?;
    if matches!(actor.via, ActorVia::ApiKey | ActorVia::OAuth) {
        return Err(AppError::Forbidden(
            "Connect Google from the dashboard using a browser session".into(),
        ));
    }
    let (id, _, secrets) = credentials(cfg)?;
    let state = token::random_token(32);
    let verifier = token::random_token(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = url::Url::parse(&cfg.google_auth_url).map_err(AppError::internal)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", id),
        ("redirect_uri", &redirect_uri(cfg)),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    let enc = secrets.encrypt(&verifier).map_err(AppError::internal)?;
    db::ga4::begin(
        pool,
        actor.org_id,
        project,
        actor.user_id,
        &token::sha256_hex(&state),
        &enc,
    )
    .await?;
    Ok(Ga4ConnectUrl { url: url.into() })
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct Ga4Callback {
    pub state: String,
    pub code: Option<String>,
    pub error: Option<String>,
}

pub async fn callback(
    pool: &PgPool,
    actor: &Actor,
    cfg: &Config,
    input: Ga4Callback,
) -> AppResult<Uuid> {
    if matches!(actor.via, ActorVia::ApiKey | ActorVia::OAuth) {
        return Err(AppError::Forbidden(
            "Use the browser session that started the connection".into(),
        ));
    }
    let (project, enc) = db::ga4::consume(
        pool,
        actor.org_id,
        actor.user_id,
        &token::sha256_hex(&input.state),
    )
    .await?;
    authorize(pool, actor, project, true).await?;
    if input.error.is_some() {
        return Err(AppError::Validation(
            "Google connection was declined; return to Analytics and try again".into(),
        ));
    }
    let code = input
        .code
        .filter(|c| !c.is_empty() && c.len() < 8192)
        .ok_or_else(|| {
            AppError::Validation(
                "Google did not return an authorization code; connect again".into(),
            )
        })?;
    let (_, _, secrets) = credentials(cfg)?;
    let verifier = secrets
        .decrypt(&enc)
        .map_err(|_| AppError::Conflict(RECONNECT.into()))?;
    let data = exchange(
        cfg,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri(cfg)),
            ("code_verifier", &verifier),
        ],
    )
    .await?;
    if data["scope"]
        .as_str()
        .is_some_and(|s| !s.split_whitespace().any(|s| s == SCOPE))
    {
        return Err(AppError::Forbidden(
            "Google did not grant read-only Analytics access; connect again and allow it".into(),
        ));
    }
    let refresh = data["refresh_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Conflict(
                "Google did not return offline access; connect again with consent".into(),
            )
        })?;
    db::ga4::save(
        pool,
        actor.org_id,
        project,
        actor.user_id,
        &secrets.encrypt(refresh).map_err(AppError::internal)?,
    )
    .await?;
    Ok(project)
}

async fn access(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<(String, Option<String>)> {
    let c = db::ga4::connection(pool, actor.org_id, project)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("Connect Google in this project's Analytics tab first".into())
        })?;
    let (_, _, secrets) = credentials(cfg)?;
    let refresh = secrets
        .decrypt(&c.refresh_token_enc)
        .map_err(|_| AppError::Conflict(RECONNECT.into()))?;
    let data = exchange(
        cfg,
        &[("grant_type", "refresh_token"), ("refresh_token", &refresh)],
    )
    .await?;
    if let Some(rotated) = data["refresh_token"].as_str().filter(|s| !s.is_empty()) {
        db::ga4::rotate(
            pool,
            actor.org_id,
            project,
            &c.refresh_token_enc,
            &secrets.encrypt(rotated).map_err(AppError::internal)?,
        )
        .await?;
    }
    let token = data["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Conflict(RECONNECT.into()))?;
    Ok((token.into(), c.property_id))
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Ga4Property {
    pub property: String,
    pub display_name: String,
}
#[derive(Serialize, ToSchema)]
pub struct Ga4Properties {
    pub properties: Vec<Ga4Property>,
    pub next_page_token: Option<String>,
}
pub async fn properties(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    page: Option<String>,
) -> AppResult<Ga4Properties> {
    authorize(pool, actor, pid, true).await?;
    if page.as_ref().is_some_and(|v| v.len() > 4096) {
        return Err(AppError::Validation("invalid page token".into()));
    }
    let (token, _) = access(pool, actor, pid, cfg).await?;
    let mut url = url::Url::parse(&format!(
        "{}/accountSummaries",
        cfg.ga4_admin_url.trim_end_matches('/')
    ))
    .map_err(AppError::internal)?;
    url.query_pairs_mut().append_pair("pageSize", "200");
    if let Some(page) = page {
        url.query_pairs_mut().append_pair("pageToken", &page);
    }
    let req = client()?.get(url).bearer_auth(token);
    let data = response(req, false).await?;
    let mut properties = vec![];
    for account in data["accountSummaries"].as_array().into_iter().flatten() {
        for item in account["propertySummaries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(property) = item["property"].as_str() {
                properties.push(Ga4Property {
                    property: property.into(),
                    display_name: item["displayName"].as_str().unwrap_or(property).into(),
                });
            }
        }
    }
    Ok(Ga4Properties {
        properties,
        next_page_token: data["nextPageToken"].as_str().map(str::to_string),
    })
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Ga4PropertyInput {
    pub property_id: String,
}
pub async fn set_property(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    input: Ga4PropertyInput,
) -> AppResult<Ga4ConnectionView> {
    if !input
        .property_id
        .strip_prefix("properties/")
        .is_some_and(|v| !v.is_empty() && v.len() < 30 && v.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(AppError::Validation(
            "property must be properties/{numeric id}".into(),
        ));
    }
    let mut page = None;
    let mut found = false;
    for _ in 0..20 {
        let result = properties(pool, actor, pid, cfg, page).await?;
        found = result
            .properties
            .iter()
            .any(|p| p.property == input.property_id);
        if found || result.next_page_token.is_none() {
            break;
        }
        page = result.next_page_token;
    }
    if !found {
        return Err(AppError::Validation(
            "Choose a property returned by your Analytics account".into(),
        ));
    }
    db::ga4::select_property(pool, actor.org_id, pid, &input.property_id).await?;
    status(pool, actor, pid, cfg).await
}
pub async fn disconnect(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<()> {
    authorize(pool, actor, pid, true).await?;
    db::ga4::disconnect(pool, actor.org_id, pid).await
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Ga4ReportKind {
    OrganicOverview,
    OrganicLandingPages,
    PagePerformance,
    TrafficAcquisition,
    KeyEvents,
    EcommercePerformance,
    SiteSearch,
    AudienceBreakdown,
    MeasurementHealth,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Ga4ReportInput {
    pub start_date: String,
    pub end_date: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
#[derive(Serialize, ToSchema)]
pub struct Ga4ReportView {
    pub property_id: String,
    pub report: Ga4ReportKind,
    pub channel: String,
    pub data: Value,
    pub note: String,
}
fn report_query(
    kind: &Ga4ReportKind,
    input: &Ga4ReportInput,
    today: NaiveDate,
) -> AppResult<Value> {
    let bad = || {
        AppError::Validation("Use valid ordered dates (at most 366 days), ending at least two UTC days ago, limit 1-1000 and offset <=1000000".into())
    };
    let start = NaiveDate::parse_from_str(&input.start_date, "%Y-%m-%d").map_err(|_| bad())?;
    let end = NaiveDate::parse_from_str(&input.end_date, "%Y-%m-%d").map_err(|_| bad())?;
    if start > end
        || end - start > Duration::days(365)
        || end > today - Duration::days(2)
        || !(1..=1000).contains(&input.limit.unwrap_or(100))
        || input.offset.unwrap_or(0) > 1000000
    {
        return Err(bad());
    }
    let (dims, metrics, organic): (&[&str], &[&str], bool) = match kind {
        Ga4ReportKind::OrganicOverview => (
            &["date"],
            &["sessions", "engagedSessions", "activeUsers", "keyEvents"],
            true,
        ),
        Ga4ReportKind::OrganicLandingPages => (
            &["hostName", "landingPagePlusQueryString"],
            &["sessions", "engagedSessions", "keyEvents"],
            true,
        ),
        Ga4ReportKind::PagePerformance => (
            &["hostName", "pagePath"],
            &["screenPageViews", "activeUsers", "userEngagementDuration"],
            true,
        ),
        Ga4ReportKind::TrafficAcquisition => (
            &["sessionDefaultChannelGroup", "sessionSourceMedium"],
            &["sessions", "engagedSessions", "keyEvents"],
            false,
        ),
        Ga4ReportKind::KeyEvents => (&["eventName"], &["keyEvents"], true),
        Ga4ReportKind::EcommercePerformance => (
            &["date"],
            &["ecommercePurchases", "purchaseRevenue", "transactions"],
            true,
        ),
        Ga4ReportKind::SiteSearch => (&["searchTerm"], &["eventCount", "totalUsers"], false),
        Ga4ReportKind::AudienceBreakdown => (
            &["country", "deviceCategory"],
            &["activeUsers", "sessions", "engagedSessions"],
            false,
        ),
        Ga4ReportKind::MeasurementHealth => (&["eventName"], &["eventCount", "totalUsers"], false),
    };
    let mut q = json!({"dateRanges":[{"startDate":input.start_date,"endDate":input.end_date}],"dimensions":dims.iter().map(|name|json!({"name":name})).collect::<Vec<_>>(),"metrics":metrics.iter().map(|name|json!({"name":name})).collect::<Vec<_>>(),"limit":input.limit.unwrap_or(100).to_string(),"offset":input.offset.unwrap_or(0).to_string(),"keepEmptyRows":false,"returnPropertyQuota":true});
    if organic {
        q["dimensionFilter"] = json!({"filter":{"fieldName":"sessionDefaultChannelGroup","stringFilter":{"matchType":"EXACT","value":"Organic Search"}}});
    }
    if matches!(kind, Ga4ReportKind::SiteSearch) {
        q["dimensionFilter"] = json!({"filter":{"fieldName":"eventName","stringFilter":{"matchType":"EXACT","value":"view_search_results"}}});
    }
    Ok(q)
}
pub async fn report(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    kind: Ga4ReportKind,
    input: Ga4ReportInput,
) -> AppResult<Ga4ReportView> {
    authorize(pool, actor, pid, false).await?;
    let query = report_query(&kind, &input, Utc::now().date_naive())?;
    let (token, property) = access(pool, actor, pid, cfg).await?;
    let property =
        property.ok_or_else(|| AppError::Conflict("Select an Analytics property first".into()))?;
    let mut data = response(
        client()?
            .post(format!(
                "{}/{}:runReport",
                cfg.ga4_data_url.trim_end_matches('/'),
                property
            ))
            .bearer_auth(token)
            .json(&query),
        false,
    )
    .await?;
    // A restricted value is unknown even if Google supplies a numeric placeholder.
    let restricted: std::collections::HashSet<String> =
        data["metadata"]["schemaRestrictionResponse"]["activeMetricRestrictions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["metricName"].as_str().map(str::to_string))
            .collect();
    let indices: Vec<usize> = data["metricHeaders"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, h)| {
            h["name"]
                .as_str()
                .filter(|n| restricted.contains(*n))
                .map(|_| i)
        })
        .collect();
    if let Some(rows) = data["rows"].as_array_mut() {
        for row in rows {
            for i in &indices {
                if let Some(value) = row["metricValues"].get_mut(*i) {
                    value["value"] = Value::Null;
                }
            }
        }
    }
    Ok(Ga4ReportView{property_id:property,report:kind,channel:if query.get("dimensionFilter").is_some()&&query["dimensionFilter"]["filter"]["fieldName"]=="sessionDefaultChannelGroup"{"organic_search"}else{"all"}.into(),data,note:"Measured GA4 data in the property's time zone and currency (response metadata). Two UTC days excluded conservatively. GA4 attribution differs from Search Console. Missing/restricted/thresholded values remain unknown; inspect metadata, rowCount and propertyQuota. Measurement health describes received events, not a tag-installation test.".into()})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_reports_are_bounded_and_organic() {
        let i = Ga4ReportInput {
            start_date: "2026-01-01".into(),
            end_date: "2026-01-28".into(),
            limit: Some(100),
            offset: None,
        };
        let today = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let q = report_query(&Ga4ReportKind::KeyEvents, &i, today).unwrap();
        assert_eq!(
            q["dimensionFilter"]["filter"]["stringFilter"]["value"],
            "Organic Search"
        );
        assert!(
            report_query(
                &Ga4ReportKind::OrganicOverview,
                &Ga4ReportInput {
                    limit: Some(1001),
                    ..i
                },
                today
            )
            .is_err()
        );
    }
}
