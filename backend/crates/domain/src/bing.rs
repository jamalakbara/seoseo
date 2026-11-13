//! Read-only Bing Webmaster Tools integration using project-bound encrypted API keys.
use crate::{Actor, Role};
use common::{AppError, AppResult, Config, crypto::SecretBox};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const RECONNECT: &str = "Bing access was rejected; replace this project's API key";

async fn authorize(pool: &PgPool, actor: &Actor, project: Uuid, write: bool) -> AppResult<()> {
    let p = db::projects::get(pool, actor.org_id, project).await?;
    if write {
        actor.require(Role::Admin)?;
        if p.archived_at.is_some() {
            return Err(AppError::Conflict(
                "Restore this project before changing its Bing connection".into(),
            ));
        }
    }
    Ok(())
}

fn secrets(cfg: &Config) -> AppResult<SecretBox> {
    let key = cfg.app_encryption_key.as_deref().ok_or_else(|| {
        AppError::Conflict("Set APP_ENCRYPTION_KEY on the server before connecting Bing".into())
    })?;
    SecretBox::from_base64(key).map_err(|e| AppError::Conflict(e.to_string()))
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("SeoSeo/0.1 (Bing Webmaster integration)")
        .build()
        .map_err(AppError::internal)
}

async fn call(
    cfg: &Config,
    key: &str,
    operation: &str,
    params: &[(&str, String)],
) -> AppResult<Value> {
    let mut url = url::Url::parse(cfg.bing_webmaster_api_url.trim_end_matches('/'))
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid BING_WEBMASTER_API_URL")))?;
    url.set_path(&format!(
        "{}/{}",
        url.path().trim_end_matches('/'),
        operation
    ));
    {
        let mut q = url.query_pairs_mut();
        for (name, value) in params {
            q.append_pair(name, value);
        }
        q.append_pair("apikey", key);
    }
    let mut res = client()?.get(url).send().await.map_err(|_| {
        AppError::Unavailable("Bing Webmaster Tools could not be reached; try again shortly".into())
    })?;
    if !res.status().is_success() {
        return Err(match res.status().as_u16() {
            400 | 401 => AppError::Conflict(RECONNECT.into()),
            403 => AppError::Forbidden("Bing refused access to this site".into()),
            429 => AppError::Unavailable("Bing quota reached; wait before trying again".into()),
            _ => AppError::Unavailable("Bing returned an error; try again shortly".into()),
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("Bing response was interrupted".into()))?
    {
        if body.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err(AppError::Unavailable("Bing response is too large".into()));
        }
        body.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::Unavailable("Bing returned an invalid response".into()))?;
    Ok(value.get("d").cloned().unwrap_or(value))
}

async fn key(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<(db::bing::Connection, String)> {
    let c = db::bing::connection(pool, actor.org_id, project)
        .await?
        .ok_or_else(|| AppError::Conflict("Connect Bing Webmaster Tools first".into()))?;
    let plain = secrets(cfg)?
        .decrypt(&c.api_key_enc)
        .map_err(|_| AppError::Conflict(RECONNECT.into()))?;
    Ok((c, plain))
}

#[derive(Serialize, ToSchema)]
pub struct BingConnectionView {
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
) -> AppResult<BingConnectionView> {
    authorize(pool, actor, project, false).await?;
    let c = db::bing::connection(pool, actor.org_id, project).await?;
    Ok(BingConnectionView {
        configured: secrets(cfg).is_ok(),
        connected: c.is_some(),
        connected_by: c.as_ref().map(|x| x.connected_by),
        site_url: c.and_then(|x| x.site_url),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct BingConnectInput {
    pub api_key: String,
}

pub async fn connect(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    input: BingConnectInput,
) -> AppResult<BingConnectionView> {
    authorize(pool, actor, project, true).await?;
    let secret_box = secrets(cfg)?;
    let api_key = input.api_key.trim();
    if !(8..=512).contains(&api_key.len()) || api_key.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "Bing API key must be 8-512 non-whitespace characters".into(),
        ));
    }
    let _ = call(cfg, api_key, "GetUserSites", &[]).await?;
    let encrypted = secret_box.encrypt(api_key).map_err(AppError::internal)?;
    db::bing::save(pool, actor.org_id, project, actor.user_id, &encrypted).await?;
    status(pool, actor, project, cfg).await
}

#[derive(Serialize, ToSchema)]
pub struct BingSiteView {
    pub url: String,
    pub verified: bool,
}

async fn available_sites(cfg: &Config, api_key: &str) -> AppResult<Vec<BingSiteView>> {
    let data = call(cfg, api_key, "GetUserSites", &[]).await?;
    let rows = data
        .as_array()
        .ok_or_else(|| AppError::Unavailable("Bing returned an invalid sites response".into()))?;
    if rows.len() > 10_000 {
        return Err(AppError::Unavailable("Bing returned too many sites".into()));
    }
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some(BingSiteView {
                url: row.get("Url")?.as_str()?.to_string(),
                verified: row
                    .get("IsVerified")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect())
}

pub async fn sites(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
) -> AppResult<Vec<BingSiteView>> {
    // A user-level Bing key can reveal sites unrelated to this project.
    authorize(pool, actor, project, true).await?;
    let (_, api_key) = key(pool, actor, project, cfg).await?;
    available_sites(cfg, &api_key).await
}

#[derive(Deserialize, ToSchema)]
pub struct BingSiteInput {
    pub site_url: String,
}

pub async fn set_site(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    input: BingSiteInput,
) -> AppResult<BingConnectionView> {
    authorize(pool, actor, project, true).await?;
    let (_, api_key) = key(pool, actor, project, cfg).await?;
    let site = input.site_url.trim();
    if !available_sites(cfg, &api_key)
        .await?
        .iter()
        .any(|x| x.verified && x.url == site)
    {
        return Err(AppError::Validation(
            "Choose an exact verified site returned by Bing".into(),
        ));
    }
    db::bing::select_site(pool, actor.org_id, project, site).await?;
    status(pool, actor, project, cfg).await
}

#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BingReportKind {
    Queries,
    Pages,
    Crawl,
    InboundLinks,
}

#[derive(Deserialize, ToSchema)]
pub struct BingReportInput {
    pub page: Option<i16>,
}

#[derive(Serialize, ToSchema)]
pub struct BingReportView {
    pub kind: BingReportKind,
    pub site_url: String,
    pub freshness_note: String,
    pub data: Value,
}

fn sanitize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("__type");
            map.remove("AuthenticationCode");
            map.remove("DnsVerificationCode");
            for value in map.values_mut() {
                sanitize(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                sanitize(value);
            }
        }
        _ => {}
    }
}

pub async fn report(
    pool: &PgPool,
    actor: &Actor,
    project: Uuid,
    cfg: &Config,
    kind: BingReportKind,
    input: BingReportInput,
) -> AppResult<BingReportView> {
    authorize(pool, actor, project, false).await?;
    let (connection, api_key) = key(pool, actor, project, cfg).await?;
    let site = connection
        .site_url
        .ok_or_else(|| AppError::Conflict("Select a verified Bing site first".into()))?;
    let (operation, params, note) = match kind {
        BingReportKind::Queries => (
            "GetQueryStats",
            vec![("siteUrl", site.clone())],
            "Bing top-query statistics are updated weekly.",
        ),
        BingReportKind::Pages => (
            "GetPageStats",
            vec![("siteUrl", site.clone())],
            "Bing top-page statistics are updated weekly.",
        ),
        BingReportKind::Crawl => (
            "GetCrawlStats",
            vec![("siteUrl", site.clone())],
            "Bing crawl statistics are updated daily.",
        ),
        BingReportKind::InboundLinks => {
            let page = input.page.unwrap_or(0);
            if !(0..=1000).contains(&page) {
                return Err(AppError::Validation(
                    "page must be between 0 and 1000".into(),
                ));
            }
            (
                "GetLinkCounts",
                vec![("siteUrl", site.clone()), ("page", page.to_string())],
                "Inbound-link counts are Bing's paged first-party sample; absence is not proof of no link.",
            )
        }
    };
    let mut data = call(cfg, &api_key, operation, &params).await?;
    sanitize(&mut data);
    Ok(BingReportView {
        kind,
        site_url: site,
        freshness_note: note.into(),
        data,
    })
}

pub async fn disconnect(pool: &PgPool, actor: &Actor, project: Uuid) -> AppResult<()> {
    authorize(pool, actor, project, true).await?;
    db::bing::disconnect(pool, actor.org_id, project).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_provider_metadata_and_verification_secrets() {
        let mut value = serde_json::json!({"__type":"Site", "AuthenticationCode":"secret", "nested":[{"DnsVerificationCode":"secret", "Url":"https://example.com"}]});
        sanitize(&mut value);
        assert_eq!(
            value,
            serde_json::json!({"nested":[{"Url":"https://example.com"}]})
        );
    }
}
