use crate::Actor;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use crawler::{Fetcher, FetcherConfig, UrlPolicy, technology::DetectedTechnology};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct TechnologyInput {
    /// Homepage URL or host; defaults to the project HTTPS homepage.
    pub url: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TechnologyRecord {
    pub url: String,
    pub checked_at: DateTime<Utc>,
    pub status: String,
    pub http_status: Option<u16>,
    pub technologies: Vec<DetectedTechnology>,
    pub note: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct TechnologyResult {
    pub record: TechnologyRecord,
    pub cached: bool,
}

pub async fn inspect(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    policy: UrlPolicy,
    input: TechnologyInput,
) -> AppResult<TechnologyResult> {
    let project = db::projects::get(pool, actor.org_id, pid).await?;
    let raw = input
        .url
        .or_else(|| project.domain.map(|d| format!("https://{d}/")))
        .ok_or_else(|| {
            AppError::Validation("enter a homepage URL or set the project website".into())
        })?;
    let raw = raw.trim();
    let raw = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}/")
    };
    let url = url::Url::parse(&raw)
        .ok()
        .filter(|u| u.port().is_none())
        .and_then(|u| crawler::urls::normalize(&u))
        .ok_or_else(|| AppError::Validation("enter an HTTP(S) homepage URL".into()))?;
    policy
        .check_url(&url)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    if url.as_str().len() > 2048
        || url.path() != "/"
        || url.query().is_some()
        || url.port().is_some()
    {
        return Err(AppError::Validation(
            "use a homepage root without query parameters or a custom port".into(),
        ));
    }
    let key = common::token::sha256_hex(url.as_str());
    if let Some(value) = db::providers::cache_get(pool, "technology_homepage_v1", &key, 1).await?
        && let Ok(record) = serde_json::from_value::<TechnologyRecord>(value)
        && record.url == url.as_str()
    {
        return Ok(TechnologyResult {
            record,
            cached: true,
        });
    }
    if !db::providers::claim_lookup(pool, "technology_project_claim_v1", &pid.to_string()).await? {
        return Err(AppError::Conflict(
            "one uncached homepage lookup per project per minute; retry shortly".into(),
        ));
    }
    if !db::providers::claim_lookup(pool, "technology_lookup_claim_v1", &key).await? {
        return Err(AppError::Conflict(
            "homepage lookup is already active or cooling down; retry in a minute".into(),
        ));
    }
    let record = collect(&url, policy).await?;
    db::providers::cache_put(
        pool,
        "technology_homepage_v1",
        &key,
        &serde_json::to_value(&record).map_err(AppError::internal)?,
    )
    .await?;
    Ok(TechnologyResult {
        record,
        cached: false,
    })
}

async fn collect(url: &url::Url, policy: UrlPolicy) -> AppResult<TechnologyRecord> {
    let fetcher = Fetcher::new(FetcherConfig {
        policy,
        user_agent: "SeoSeoBot/0.1 (+https://github.com/seoseo/seoseo; homepage technology lookup)"
            .into(),
        timeout: Duration::from_secs(10),
    })
    .map_err(AppError::internal)?;
    let mut record=TechnologyRecord{url:url.to_string(),checked_at:Utc::now(),status:"unknown".into(),http_status:None,technologies:vec![],note:"No verified technology stack. Only homepage markup hints are sampled; hidden/customized assets and backend services remain unknown.".into()};
    let robots_url = url.join("robots.txt").map_err(AppError::internal)?;
    let Ok((status, content_type, bytes)) = fetcher
        .get_exact_resource(&robots_url, crawler::robots::MAX_ROBOTS_BYTES)
        .await
    else {
        record.note = "Robots access was blocked or unavailable; homepage was not fetched.".into();
        return Ok(record);
    };
    if ![200, 404, 410].contains(&status)
        || (status == 200
            && content_type
                .as_deref()
                .is_some_and(|s| s.to_ascii_lowercase().contains("text/html")))
    {
        record.note =
            "Robots access was inconclusive, blocked or rate limited; homepage was not fetched."
                .into();
        return Ok(record);
    }
    let robots = crawler::robots::Robots::parse(
        crawler::ROBOTS_TOKEN,
        if status == 200 { &bytes } else { &[] },
        Some(status),
    );
    if !robots.allowed(url) {
        record.status = "robots_disallowed".into();
        record.note =
            "Robots policy disallows this homepage; no technology absence is inferred.".into();
        return Ok(record);
    }
    let delay = robots.crawl_delay_secs.unwrap_or(0.0);
    if !delay.is_finite() || delay > 5.0 {
        record.note = "Robots crawl delay exceeds this bounded lookup; use a full audit.".into();
        return Ok(record);
    }
    if delay > 0.0 {
        tokio::time::sleep(Duration::from_secs_f32(delay)).await;
    }
    let Ok((status, content_type, bytes)) = fetcher
        .get_exact_resource(url, crawler::fetch::MAX_HTML_BYTES)
        .await
    else {
        record.note =
            "Homepage blocked, unreachable or oversized; technology remains unknown.".into();
        return Ok(record);
    };
    record.http_status = Some(status);
    if (300..400).contains(&status) {
        record.status = "redirect".into();
        record.note =
            "Homepage redirects were not followed. Inspect the canonical homepage URL explicitly."
                .into();
        return Ok(record);
    }
    if status != 200
        || !content_type
            .as_deref()
            .is_some_and(|s| s.to_ascii_lowercase().contains("text/html"))
    {
        record.note = "No successful HTML homepage observation; technology remains unknown.".into();
        return Ok(record);
    }
    let facts = crawler::extract::extract(&bytes);
    let title = facts.title.unwrap_or_default().to_ascii_lowercase();
    if [
        "just a moment",
        "verify you are human",
        "access denied",
        "attention required",
    ]
    .iter()
    .any(|s| title.contains(s))
        || bytes.windows(7).any(|s| s.eq_ignore_ascii_case(b"cf-chl-"))
    {
        record.status = "blocked".into();
        record.note = "Bot challenge observed; stopped without bypassing it.".into();
        return Ok(record);
    }
    record.technologies = crawler::technology::detect(&bytes);
    record.status = "observed".into();
    Ok(record)
}
