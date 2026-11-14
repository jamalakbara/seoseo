//! M8 bounded log aggregates, measured volatility and merchant crawler credentials.
use crate::{Actor, Role};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeMap;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LogInput {
    pub content: String,
}

/// Combined/CLF only. Retain status and declared bot family, never IPs, query strings,
/// cookies, referrers, paths (which can themselves contain personal identifiers), or UA.
pub fn analyze_log(content: &str) -> AppResult<Value> {
    if content.is_empty() || content.len() > 1024 * 1024 {
        return Err(AppError::Validation(
            "upload 1 byte to 1 MiB of Apache/Nginx common or combined access logs".into(),
        ));
    }
    let mut statuses = BTreeMap::<u16, u64>::new();
    let mut bots = BTreeMap::<&str, u64>::new();
    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_bytes = 0u64;
    let mut unknown_bytes = 0u64;
    for line in content.lines().take(20001) {
        if accepted + rejected >= 20000 {
            return Err(AppError::Validation("maximum 20,000 log lines".into()));
        }
        if line.len() > 16384 {
            rejected += 1;
            continue;
        }
        let quoted: Vec<&str> = line.split('"').collect();
        let parsed = (|| {
            // Require CLF timestamp and request/status fields; JSON/W3C are not guessed.
            if !quoted.first()?.contains('[') || !quoted.first()?.contains(']') {
                return None;
            }
            let req: Vec<_> = quoted.get(1)?.split_whitespace().collect();
            if req.len() != 3 || !req[2].starts_with("HTTP/") {
                return None;
            }
            let fields: Vec<_> = quoted.get(2)?.split_whitespace().collect();
            let status = fields.first()?.parse::<u16>().ok()?;
            if !(100..=599).contains(&status) {
                return None;
            }
            let bytes = fields.get(1)?;
            let bytes = if *bytes == "-" {
                None
            } else {
                Some(bytes.parse::<u64>().ok()?)
            };
            Some((status, bytes))
        })();
        let Some((status, bytes)) = parsed else {
            rejected += 1;
            continue;
        };
        accepted += 1;
        *statuses.entry(status).or_default() += 1;
        if let Some(bytes) = bytes {
            total_bytes = total_bytes.saturating_add(bytes);
        } else {
            unknown_bytes += 1;
        }
        let ua = quoted.get(5).unwrap_or(&"").to_ascii_lowercase();
        let family = [
            ("googlebot", "Googlebot"),
            ("bingbot", "Bingbot"),
            ("gptbot", "GPTBot"),
            ("claudebot", "ClaudeBot"),
            ("seoseobot", "SeoSeoBot"),
        ]
        .into_iter()
        .find(|(needle, _)| ua.contains(needle))
        .map(|(_, family)| family)
        .unwrap_or("other / unknown");
        *bots.entry(family).or_default() += 1;
    }
    Ok(
        json!({"source":"uploaded_access_log","accepted":accepted,"rejected":rejected,"statuses":statuses,"declared_agents":bots,"known_response_bytes":total_bytes,"unknown_bytes_rows":unknown_bytes,"note":"Sample counts only. User agents are self-declared, not verified bots. Raw logs and request identifiers are discarded."}),
    )
}
pub async fn save_log(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    v: LogInput,
) -> AppResult<db::extras::LogAnalysis> {
    actor.require(Role::Admin)?;
    db::projects::get(pool, actor.org_id, pid).await?;
    let summary = analyze_log(&v.content)?;
    db::extras::save_log(pool, pid, summary).await
}
pub async fn logs(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
) -> AppResult<Vec<db::extras::LogAnalysis>> {
    db::projects::get(pool, actor.org_id, pid).await?;
    db::extras::logs(pool, pid).await
}
pub async fn delete_log(pool: &PgPool, actor: &Actor, pid: Uuid, id: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::projects::get(pool, actor.org_id, pid).await?;
    db::extras::delete_log(pool, pid, id).await
}

#[derive(Serialize, ToSchema)]
pub struct Volatility {
    pub index: Option<f64>,
    pub rank_movement_index: Option<f64>,
    pub serp_pairs: Vec<SerpPairView>,
    pub comparable_keywords: usize,
    pub excluded_keywords: usize,
    pub source: String,
    pub note: String,
}
#[derive(Serialize, ToSchema)]
pub struct SerpPairView {
    pub keyword: String,
    pub current_id: Option<Uuid>,
    pub previous_id: Option<Uuid>,
    pub current_urls: usize,
    pub previous_urls: usize,
    pub churn: Option<f64>,
}
fn serp_churn(a: &[String], b: &[String]) -> Option<f64> {
    if a.len() != 10 || b.len() != 10 {
        return None;
    }
    let overlap = a.iter().filter(|url| b.contains(url)).count();
    Some((10 - overlap) as f64 * 10.0)
}
pub fn volatility(rows: &[db::extras::Movement]) -> Volatility {
    let comparable: Vec<_> = rows
        .iter()
        .filter(|r| r.comparable && r.depth > 0)
        .collect();
    let index = if comparable.is_empty() {
        None
    } else {
        Some(
            comparable
                .iter()
                .map(|r| {
                    let before = r.previous.unwrap_or(r.depth + 1).clamp(1, r.depth + 1);
                    let after = r.current.unwrap_or(r.depth + 1).clamp(1, r.depth + 1);
                    f64::from((after - before).abs()) / f64::from(r.depth) * 100.0
                })
                .sum::<f64>()
                / comparable.len() as f64,
        )
    };
    Volatility { index, rank_movement_index:index,serp_pairs:vec![],comparable_keywords:comparable.len(),excluded_keywords:rows.len()-comparable.len(),source:"own_rank_measurements".into(),note:"0–100 mean normalized absolute position change in the latest completed pair per current-market tracker; not a web-wide Google volatility index. Same provider, exactness and depth required. Measured absence is depth+1; unchecked/incompatible rows excluded.".into() }
}
pub async fn get_volatility(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<Volatility> {
    db::projects::get(pool, actor.org_id, pid).await?;
    let mut v = volatility(&db::extras::movements(pool, pid).await?);
    v.serp_pairs = db::extras::serp_pairs(pool, pid)
        .await?
        .into_iter()
        .map(|r| {
            let churn = serp_churn(&r.current_urls, &r.previous_urls);
            SerpPairView {
                keyword: r.keyword,
                current_id: r.current_id,
                previous_id: r.previous_id,
                current_urls: r.current_urls.len(),
                previous_urls: r.previous_urls.len(),
                churn,
            }
        })
        .collect();
    let measured: Vec<_> = v.serp_pairs.iter().filter_map(|r| r.churn).collect();
    v.index = if measured.is_empty() {
        None
    } else {
        Some(measured.iter().sum::<f64>() / measured.len() as f64)
    };
    v.comparable_keywords = measured.len();
    v.excluded_keywords = v.serp_pairs.len() - measured.len();
    v.source = "own_cached_serp_top_ten".into();
    v.note="SERP index: mean percentage of replaced top-ten URLs across latest same-provider exact-engine snapshot pairs for up to 500 saved keywords in the current project market. Both lists must contain ten distinct URLs; snapshots at most 30 days old, pair gap at most 30 days. Missing snapshots and incomplete lists remain unknown. Not a web-wide Google index. rank_movement_index separately summarizes compatible completed tracker pairs; absent ranks use depth+1, unchecked rows are excluded.".into();
    Ok(v)
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CrawlerCredentialInput {
    pub host: String,
    pub signature: String,
    pub signature_input: String,
    pub signature_agent: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Serialize, ToSchema)]
pub struct CredentialView {
    pub configured: bool,
    pub host: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}
pub async fn credential_status(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
) -> AppResult<CredentialView> {
    db::projects::get(pool, actor.org_id, pid).await?;
    let c = db::extras::credential(pool, pid).await?;
    Ok(CredentialView {
        configured: c.is_some(),
        host: c.as_ref().map(|v| v.host.clone()),
        expires_at: c.map(|v| v.expires_at),
    })
}
pub async fn save_credential(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    cfg: &Config,
    v: CrawlerCredentialInput,
) -> AppResult<CredentialView> {
    actor.require(Role::Admin)?;
    let project = db::projects::get(pool, actor.org_id, pid).await?;
    let host = v.host.trim().to_ascii_lowercase();
    // Exact reviewed host, no wildcard or credential transfer to subdomains.
    let u = url::Url::parse(&format!("https://{host}/"))
        .map_err(|_| AppError::Validation("enter the exact project host".into()))?;
    crawler::UrlPolicy::default()
        .check_url(&u)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    if u.host_str() != Some(host.as_str())
        || u.port().is_some()
        || project.domain.as_deref() != Some(host.as_str())
    {
        return Err(AppError::Validation(
            "credential host must exactly match the project's website host".into(),
        ));
    }
    for s in [&v.signature, &v.signature_input, &v.signature_agent] {
        if s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "signed headers must be nonempty single-line values, max 8192 bytes".into(),
            ));
        }
    }
    if !v.signature_input.contains("tag=\"web-bot-auth\"")
        || !v.signature_input.contains("\"@authority\"")
        || !v.signature_input.contains("\"signature-agent\"")
    {
        return Err(AppError::Validation(
            "use the complete Web Bot Auth headers supplied by your store admin".into(),
        ));
    }
    if v.expires_at <= Utc::now() || v.expires_at > Utc::now() + chrono::Duration::days(93) {
        return Err(AppError::Validation(
            "expiry must be in the next 93 days".into(),
        ));
    }
    let key = cfg
        .app_encryption_key
        .as_deref()
        .ok_or_else(|| AppError::Conflict("operator must set APP_ENCRYPTION_KEY".into()))?;
    let secret = common::crypto::SecretBox::from_base64(key)
        .map_err(AppError::internal)?
        .encrypt(&serde_json::to_string(&v).map_err(AppError::internal)?)
        .map_err(AppError::internal)?;
    db::extras::set_credential(pool, pid, &host, secret, v.expires_at).await?;
    credential_status(pool, actor, pid).await
}
pub async fn delete_credential(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::projects::get(pool, actor.org_id, pid).await?;
    db::extras::delete_credential(pool, pid).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serp_churn_requires_complete_lists_and_counts_replacements() {
        let a: Vec<_> = (0..10)
            .map(|i| format!("https://example.com/{i}"))
            .collect();
        let mut b = a.clone();
        b[0] = "https://new.example.com/".into();
        assert_eq!(serp_churn(&a, &b), Some(10.0));
        assert_eq!(serp_churn(&a, &a), Some(0.0));
        assert!(serp_churn(&a[..9], &b).is_none());
    }
    #[test]
    fn logs_discard_identifiers_and_reject_malformed_rows() {
        let v=analyze_log("203.0.113.1 - person [10/Oct/2000:13:55:36 -0700] \"GET /secret?token=private HTTP/1.1\" 404 12 \"private-referrer\" \"Googlebot\"\nbad line").unwrap();
        assert_eq!(v["accepted"], 1);
        assert_eq!(v["rejected"], 1);
        assert_eq!(v["declared_agents"]["Googlebot"], 1);
        let s = v.to_string();
        for private in ["203.0.113.1", "person", "secret", "token", "referrer"] {
            assert!(!s.contains(private));
        }
    }
    #[test]
    fn volatility_preserves_unknown_and_measured_absence() {
        assert!(volatility(&[]).index.is_none());
        let rows = [
            db::extras::Movement {
                previous: Some(1),
                current: None,
                depth: 10,
                comparable: true,
            },
            db::extras::Movement {
                previous: None,
                current: Some(2),
                depth: 10,
                comparable: false,
            },
        ];
        let v = volatility(&rows);
        assert_eq!(v.index, Some(100.0));
        assert_eq!(v.excluded_keywords, 1);
    }
}
