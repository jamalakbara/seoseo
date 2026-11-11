//! Shared HTTP client, status mapping and per-provider pacing.

use crate::ProviderError;
use reqwest::{Response, StatusCode};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(crate::USER_AGENT)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("http client")
});

pub(crate) fn client() -> &'static reqwest::Client {
    &CLIENT
}

type Slot = Arc<tokio::sync::Mutex<Option<Instant>>>;
static PACERS: LazyLock<Mutex<HashMap<&'static str, Slot>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Wait until at least `gap` has passed since the previous call for `key` in this process.
pub(crate) async fn pace(key: &'static str, gap: Duration) {
    let slot = PACERS
        .lock()
        .expect("pacer lock")
        .entry(key)
        .or_default()
        .clone();
    let mut last = slot.lock().await;
    if let Some(t) = *last {
        let since = t.elapsed();
        if since < gap {
            tokio::time::sleep(gap - since).await;
        }
    }
    *last = Some(Instant::now());
}

pub(crate) fn net_err(e: reqwest::Error) -> ProviderError {
    ProviderError::Unavailable(if e.is_timeout() {
        "request timed out".into()
    } else if e.is_connect() {
        "could not connect".into()
    } else {
        "network error".into()
    })
}

/// Map HTTP status to a provider error. Blocks are surfaced, never retried around.
pub(crate) fn check(res: Response) -> Result<Response, ProviderError> {
    let s = res.status();
    if s.is_success() {
        return Ok(res);
    }
    Err(match s {
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited,
        StatusCode::UNAUTHORIZED => {
            ProviderError::NotConfigured("the API key was rejected (401)".into())
        }
        StatusCode::FORBIDDEN => ProviderError::Blocked("access denied (403)".into()),
        s if s.is_server_error() => ProviderError::Unavailable(format!("server error ({s})")),
        s => ProviderError::BadResponse(format!("HTTP {s}")),
    })
}

pub(crate) async fn text(res: Response, max: usize) -> Result<String, ProviderError> {
    let bytes = res.bytes().await.map_err(net_err)?;
    if bytes.len() > max {
        return Err(ProviderError::BadResponse("response too large".into()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub(crate) fn bad_json(e: serde_json::Error) -> ProviderError {
    ProviderError::BadResponse(format!("invalid JSON ({e})"))
}

pub(crate) fn url(base: &str, params: &[(&str, &str)]) -> Result<url::Url, ProviderError> {
    url::Url::parse_with_params(base, params)
        .map_err(|e| ProviderError::NotConfigured(format!("invalid provider URL: {e}")))
}
