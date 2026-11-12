//! Read-only Search Console integration. All entry points authorize the project first.
use crate::{Actor, ActorVia, Role};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Months, NaiveDate, Utc};
use common::{AppError, AppResult, Config, crypto::SecretBox, token};
use futures::{StreamExt, TryStreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const SCOPE: &str = "https://www.googleapis.com/auth/webmasters.readonly";
const DIMENSIONS: &[&str] = &[
    "query",
    "page",
    "country",
    "device",
    "date",
    "searchAppearance",
];
const RECONNECT: &str =
    "Google access expired or was revoked; reconnect in this project's Search Console tab";

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
        .user_agent("SeoSeo/0.1 (Search Console integration)")
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
            403 => AppError::Forbidden("Google refused access; check property permissions and that the Search Console API is enabled".into()),
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
        "{}/api/v1/gsc/oauth/callback",
        cfg.web_base_url.trim_end_matches('/')
    )
}

#[derive(Serialize, ToSchema)]
pub struct ConnectionView {
    pub configured: bool,
    pub connected: bool,
    pub connected_by: Option<Uuid>,
    pub site_url: Option<String>,
}

pub async fn status(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<ConnectionView> {
    authorize(pool, actor, project, false).await?;
    let c = db::gsc::connection(pool, actor.org_id, project).await?;
    Ok(ConnectionView {
        configured: credentials(cfg).is_ok(),
        connected: c.is_some(),
        connected_by: c.as_ref().map(|c| c.connected_by),
        site_url: c.and_then(|c| c.site_url),
    })
}

#[derive(Serialize, ToSchema)]
pub struct ConnectUrl {
    pub url: String,
}

pub async fn connect(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<ConnectUrl> {
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
    db::gsc::begin(
        pool,
        actor.org_id,
        project,
        actor.user_id,
        &token::sha256_hex(&state),
        &enc,
    )
    .await?;
    Ok(ConnectUrl { url: url.into() })
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct Callback {
    pub state: String,
    pub code: Option<String>,
    pub error: Option<String>,
}

pub async fn callback(
    pool: &PgPool,
    actor: &Actor,
    cfg: &Config,
    input: Callback,
) -> AppResult<Uuid> {
    if matches!(actor.via, ActorVia::ApiKey | ActorVia::OAuth) {
        return Err(AppError::Forbidden(
            "Use the browser session that started the connection".into(),
        ));
    }
    let (project, enc) = db::gsc::consume(
        pool,
        actor.org_id,
        actor.user_id,
        &token::sha256_hex(&input.state),
    )
    .await?;
    authorize(pool, actor, project, true).await?;
    if input.error.is_some() {
        return Err(AppError::Validation(
            "Google connection was declined; return to Search Console and try again".into(),
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
            "Google did not grant read-only Search Console access; connect again and allow it"
                .into(),
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
    db::gsc::save(
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
    let c = db::gsc::connection(pool, actor.org_id, project)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("Connect Google in this project's Search Console tab first".into())
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
    let token = data["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Conflict(RECONNECT.into()))?;
    Ok((token.into(), c.site_url))
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub site_url: String,
    pub permission_level: String,
}

pub async fn sites(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<Vec<Site>> {
    authorize(pool, actor, project, true).await?;
    let (token, _) = access(pool, actor, project, cfg).await?;
    let data = response(
        client()?
            .get(format!("{}/sites", cfg.gsc_api_url.trim_end_matches('/')))
            .bearer_auth(token),
        false,
    )
    .await?;
    let sites: Vec<Site> = serde_json::from_value(
        data.get("siteEntry").cloned().unwrap_or(json!([])),
    )
    .map_err(|_| AppError::Unavailable("Google returned an invalid property list".into()))?;
    Ok(sites
        .into_iter()
        .filter(|s| s.permission_level != "siteUnverifiedUser")
        .collect())
}

#[derive(Deserialize, ToSchema)]
pub struct SiteInput {
    pub site_url: String,
}

pub async fn set_site(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    input: SiteInput,
) -> AppResult<ConnectionView> {
    let sites = sites(pool, actor, project, cfg).await?;
    if !sites.iter().any(|s| s.site_url == input.site_url) {
        return Err(AppError::Validation(
            "Choose a verified property returned by the property list".into(),
        ));
    }
    db::gsc::select_site(pool, actor.org_id, project, &input.site_url).await?;
    status(pool, actor, project, cfg).await
}

pub async fn disconnect(pool: &PgPool, actor: &Actor, project: Uuid) -> AppResult<()> {
    authorize(pool, actor, project, true).await?;
    db::gsc::disconnect(pool, actor.org_id, project).await
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformanceFilter {
    pub dimension: String,
    pub operator: String,
    pub expression: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PerformanceInput {
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default)]
    pub filters: Vec<PerformanceFilter>,
    pub search_type: Option<String>,
    pub row_limit: Option<u32>,
    pub start_row: Option<u32>,
}

impl PerformanceInput {
    fn query(&self, today: NaiveDate) -> AppResult<Value> {
        let bad = || {
            AppError::Validation("Use valid dates within the last 16 months, up to three days ago; 1–1000 rows, supported dimensions and AND filters".into())
        };
        let start = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").map_err(|_| bad())?;
        let end = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d").map_err(|_| bad())?;
        if start > end
            || start < today.checked_sub_months(Months::new(16)).ok_or_else(bad)?
            || end > today - Duration::days(3)
            || !(1..=1000).contains(&self.row_limit.unwrap_or(100))
            || self.start_row.unwrap_or(0) > 1_000_000
            || self.dimensions.len() > 6
            || self
                .dimensions
                .iter()
                .any(|d| !DIMENSIONS.contains(&d.as_str()))
            || self
                .dimensions
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.dimensions.len()
            || self.filters.len() > 10
            || self.filters.iter().any(|f| {
                !DIMENSIONS.contains(&f.dimension.as_str())
                    || ![
                        "equals",
                        "notEquals",
                        "contains",
                        "notContains",
                        "includingRegex",
                        "excludingRegex",
                    ]
                    .contains(&f.operator.as_str())
                    || f.expression.len() > 4096
            })
            || !["web", "image", "video", "news", "discover", "googleNews"]
                .contains(&self.search_type.as_deref().unwrap_or("web"))
        {
            return Err(bad());
        }
        let mut query = json!({"startDate":self.start_date,"endDate":self.end_date,"dimensions":self.dimensions,"type":self.search_type.as_deref().unwrap_or("web"),"rowLimit":self.row_limit.unwrap_or(100),"startRow":self.start_row.unwrap_or(0),"dataState":"final"});
        if !self.filters.is_empty() {
            query["dimensionFilterGroups"] = json!([{"groupType":"and","filters":self.filters}]);
        }
        Ok(query)
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PerformanceRow {
    #[serde(default)]
    pub keys: Vec<String>,
    pub clicks: f64,
    pub impressions: f64,
    pub ctr: f64,
    pub position: f64,
}
#[derive(Serialize, ToSchema)]
pub struct PerformanceView {
    pub site_url: String,
    pub dimensions: Vec<String>,
    pub rows: Vec<PerformanceRow>,
    pub next_start_row: Option<u32>,
    pub source: String,
    pub note: String,
}

pub async fn performance(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    input: PerformanceInput,
) -> AppResult<PerformanceView> {
    authorize(pool, actor, project, false).await?;
    let query = input.query(Utc::now().date_naive())?;
    let (token, site) = access(pool, actor, project, cfg).await?;
    let site =
        site.ok_or_else(|| AppError::Conflict("Select a Search Console property first".into()))?;
    let mut url = url::Url::parse(&cfg.gsc_api_url).map_err(AppError::internal)?;
    url.path_segments_mut()
        .map_err(|_| AppError::Conflict("GSC_API_URL must be a base URL".into()))?
        .pop_if_empty()
        .extend(["sites", &site, "searchAnalytics", "query"]);
    let data = response(client()?.post(url).bearer_auth(token).json(&query), false).await?;
    let rows: Vec<PerformanceRow> =
        serde_json::from_value(data.get("rows").cloned().unwrap_or(json!([]))).map_err(|_| {
            AppError::Unavailable("Google returned invalid performance data".into())
        })?;
    let next_start_row = (rows.len() == input.row_limit.unwrap_or(100) as usize)
        .then_some(input.start_row.unwrap_or(0) + rows.len() as u32);
    Ok(PerformanceView { site_url:site, dimensions:input.dimensions, rows, next_start_row, source:"google_search_console".into(), note:"Measured Google data; dates use Pacific Time. Recent three days excluded. Google returns top rows and may omit anonymized queries; grouped rows are not site totals.".into() })
}

#[derive(Deserialize, ToSchema)]
pub struct InspectInput {
    pub urls: Vec<String>,
}
#[derive(Serialize, ToSchema)]
pub struct InspectionRow {
    pub url: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

pub async fn inspect(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    input: InspectInput,
) -> AppResult<Vec<InspectionRow>> {
    authorize(pool, actor, project, false).await?;
    if input.urls.is_empty()
        || input.urls.len() > 10
        || input.urls.iter().any(|u| {
            u.len() > 2048
                || !url::Url::parse(u).is_ok_and(|u| {
                    matches!(u.scheme(), "http" | "https")
                        && u.host_str().is_some()
                        && u.username().is_empty()
                        && u.password().is_none()
                })
        })
    {
        return Err(AppError::Validation(
            "Inspect 1–10 fully qualified HTTP(S) URLs without credentials".into(),
        ));
    }
    let (token, site) = access(pool, actor, project, cfg).await?;
    let site =
        site.ok_or_else(|| AppError::Conflict("Select a Search Console property first".into()))?;
    let http = client()?;
    stream::iter(input.urls.into_iter().map(|url| {
        let request = http
            .post(&cfg.gsc_inspection_url)
            .bearer_auth(&token)
            .json(&json!({"inspectionUrl":url,"siteUrl":site}));
        async move {
            match response(request, false).await {
                Ok(data) => Ok(InspectionRow {
                    url,
                    result: Some(data),
                    error: None,
                }),
                Err(e @ AppError::Conflict(_))
                | Err(e @ AppError::Forbidden(_))
                | Err(e @ AppError::Unavailable(_)) => Err(e),
                Err(e) => Ok(InspectionRow {
                    url,
                    result: None,
                    error: Some(e.to_string()),
                }),
            }
        }
    }))
    .buffered(4)
    .try_collect()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn performance_bounds_and_filters() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let mut p = PerformanceInput {
            start_date: "2026-10-01".into(),
            end_date: "2026-11-09".into(),
            dimensions: vec!["query".into()],
            filters: vec![],
            search_type: None,
            row_limit: Some(1000),
            start_row: None,
        };
        assert_eq!(p.query(today).unwrap()["dataState"], "final");
        p.end_date = "2026-11-10".into();
        assert!(p.query(today).is_err());
        p.end_date = "2026-11-09".into();
        p.row_limit = Some(1001);
        assert!(p.query(today).is_err());
        p.row_limit = Some(10);
        p.dimensions.push("query".into());
        assert!(p.query(today).is_err());
        p.dimensions.pop();
        p.filters.push(PerformanceFilter {
            dimension: "page".into(),
            operator: "contains".into(),
            expression: "/blog".into(),
        });
        assert_eq!(
            p.query(today).unwrap()["dimensionFilterGroups"][0]["groupType"],
            "and"
        );
    }
}
