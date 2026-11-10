//! HTTP fetcher with SSRF-safe DNS, manual redirect handling and body size caps.

use crate::policy::UrlPolicy;
use crate::urls::{self, Scope};
use futures::StreamExt;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{self, HeaderMap};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;

/// HTML bytes kept for parsing (OpenSEO uses the same 1 MiB cap).
pub const MAX_HTML_BYTES: usize = 1024 * 1024;
/// Bytes read from the wire before giving up on a response.
pub const MAX_RESPONSE_BYTES: usize = 8 * MAX_HTML_BYTES;
pub const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone)]
pub struct FetcherConfig {
    pub user_agent: String,
    pub policy: UrlPolicy,
    pub timeout: Duration,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            user_agent: crate::DEFAULT_USER_AGENT.to_string(),
            policy: UrlPolicy::default(),
            timeout: Duration::from_secs(20),
        }
    }
}

/// Resolver that drops any address the policy forbids. Applied to every connection,
/// including redirects, so a public hostname cannot resolve to an internal IP.
struct GuardedResolver {
    policy: UrlPolicy,
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let policy = self.policy;
        Box::pin(async move {
            let host = name.as_str().to_string();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| policy.allows_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(
                    format!("{host} resolves only to private or reserved addresses").into(),
                );
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchClass {
    Html,
    NonHtml,
    ClientError,
    ServerError,
    Blocked,
    RateLimited,
    NetworkError,
    PolicyBlocked,
    RedirectLoop,
    TooManyRedirects,
    RedirectOffsite,
}

impl FetchClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FetchClass::Html => "html",
            FetchClass::NonHtml => "non_html",
            FetchClass::ClientError => "client_error",
            FetchClass::ServerError => "server_error",
            FetchClass::Blocked => "blocked",
            FetchClass::RateLimited => "rate_limited",
            FetchClass::NetworkError => "network_error",
            FetchClass::PolicyBlocked => "policy_blocked",
            FetchClass::RedirectLoop => "redirect_loop",
            FetchClass::TooManyRedirects => "too_many_redirects",
            FetchClass::RedirectOffsite => "redirect_offsite",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RedirectHop {
    pub url: String,
    pub status: u16,
}

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    pub requested: Url,
    pub final_url: Url,
    pub status: Option<u16>,
    pub class: FetchClass,
    pub redirects: Vec<RedirectHop>,
    pub content_type: Option<String>,
    pub x_robots_tag: Option<String>,
    /// `Link: <…>; rel="canonical"` response header.
    pub header_canonical: Option<String>,
    /// HTML body (first [`MAX_HTML_BYTES`]) for successful HTML responses only.
    pub body: Option<Vec<u8>>,
    pub size: usize,
    pub elapsed_ms: u32,
    pub retry_after: Option<Duration>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Fetcher {
    client: reqwest::Client,
    policy: UrlPolicy,
}

impl Fetcher {
    pub fn new(cfg: FetcherConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .user_agent(cfg.user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(cfg.timeout)
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(4)
            .dns_resolver(Arc::new(GuardedResolver { policy: cfg.policy }))
            .no_proxy()
            .build()?;
        Ok(Self {
            client,
            policy: cfg.policy,
        })
    }

    pub fn policy(&self) -> UrlPolicy {
        self.policy
    }

    /// Plain GET of a small resource (robots.txt, sitemap). Follows redirects within policy.
    pub async fn get_bytes(&self, url: &Url, max: usize) -> Result<(u16, Vec<u8>), String> {
        let out = self.fetch_inner(url, None, max, true).await;
        match (out.status, out.body) {
            (Some(s), Some(b)) => Ok((s, b)),
            (Some(s), None) => Ok((s, Vec::new())),
            _ => Err(out.error.unwrap_or_else(|| "request failed".into())),
        }
    }

    /// Fetch a page for the audit. Redirects that leave `scope` stop with `RedirectOffsite`.
    pub async fn fetch_page(&self, url: &Url, scope: Option<&Scope>) -> FetchOutcome {
        self.fetch_inner(url, scope, MAX_HTML_BYTES, false).await
    }

    async fn fetch_inner(
        &self,
        url: &Url,
        scope: Option<&Scope>,
        keep: usize,
        any_type: bool,
    ) -> FetchOutcome {
        let start = Instant::now();
        let mut out = FetchOutcome {
            requested: url.clone(),
            final_url: url.clone(),
            status: None,
            class: FetchClass::NetworkError,
            redirects: Vec::new(),
            content_type: None,
            x_robots_tag: None,
            header_canonical: None,
            body: None,
            size: 0,
            elapsed_ms: 0,
            retry_after: None,
            error: None,
        };
        let mut current = url.clone();
        let mut seen = vec![urls::hash(url)];

        loop {
            if let Err(e) = self.policy.check_url(&current) {
                out.class = FetchClass::PolicyBlocked;
                out.error = Some(e.to_string());
                break;
            }
            let resp = match self.client.get(current.clone()).send().await {
                Ok(r) => r,
                Err(e) => {
                    out.class = FetchClass::NetworkError;
                    out.error = Some(describe(&e));
                    break;
                }
            };
            let status = resp.status().as_u16();
            out.status = Some(status);
            out.final_url = current.clone();

            if resp.status().is_redirection() {
                let next = resp
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|loc| urls::resolve(&current, loc));
                let Some(next) = next else {
                    out.class = FetchClass::ClientError;
                    out.error = Some("redirect without a valid Location header".into());
                    break;
                };
                out.redirects.push(RedirectHop {
                    url: next.to_string(),
                    status,
                });
                let h = urls::hash(&next);
                if seen.contains(&h) {
                    out.class = FetchClass::RedirectLoop;
                    break;
                }
                if out.redirects.len() > MAX_REDIRECTS {
                    out.class = FetchClass::TooManyRedirects;
                    break;
                }
                if let Some(scope) = scope
                    && !scope.contains(&next)
                {
                    out.final_url = next;
                    out.class = FetchClass::RedirectOffsite;
                    break;
                }
                seen.push(h);
                current = next;
                continue;
            }

            let headers = resp.headers().clone();
            out.content_type = header_str(&headers, header::CONTENT_TYPE);
            out.x_robots_tag = header_str(&headers, "x-robots-tag");
            out.header_canonical = canonical_from_link_header(&headers);
            out.retry_after = headers
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let is_html = out
                .content_type
                .as_deref()
                .is_some_and(|ct| ct.contains("text/html") || ct.contains("application/xhtml"));

            out.class = classify(status, &headers, is_html);
            let want_body = any_type || out.class == FetchClass::Html;
            if want_body {
                match read_capped(resp, keep).await {
                    Ok((body, size)) => {
                        out.body = Some(body);
                        out.size = size;
                    }
                    Err(e) => {
                        out.class = FetchClass::NetworkError;
                        out.error = Some(e);
                    }
                }
            } else {
                out.size = resp.content_length().unwrap_or(0) as usize;
            }
            break;
        }
        out.elapsed_ms = start.elapsed().as_millis().min(u32::MAX as u128) as u32;
        out
    }
}

fn classify(status: u16, headers: &HeaderMap, is_html: bool) -> FetchClass {
    let challenged = headers
        .get("cf-mitigated")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"challenge"));
    match status {
        429 => FetchClass::RateLimited,
        401 | 403 => FetchClass::Blocked,
        _ if challenged => FetchClass::Blocked,
        400..=499 => FetchClass::ClientError,
        500..=599 => FetchClass::ServerError,
        200..=299 if is_html => FetchClass::Html,
        _ => FetchClass::NonHtml,
    }
}

async fn read_capped(resp: reqwest::Response, keep: usize) -> Result<(Vec<u8>, usize), String> {
    let mut stream = resp.bytes_stream();
    let mut body = Vec::with_capacity(keep.min(64 * 1024));
    let mut size = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| describe(&e))?;
        size += chunk.len();
        if body.len() < keep {
            let take = (keep - body.len()).min(chunk.len());
            body.extend_from_slice(&chunk[..take]);
        }
        if size >= MAX_RESPONSE_BYTES {
            break;
        }
    }
    Ok((body, size))
}

fn header_str(headers: &HeaderMap, name: impl header::AsHeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
}

/// Extract the canonical URL from `Link: <https://…>; rel="canonical"`.
fn canonical_from_link_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::LINK)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.split(',').find_map(|part| {
                let (target, params) = part.split_once(';')?;
                let rel_canonical = params.split(';').any(|p| {
                    p.trim()
                        .replace('"', "")
                        .eq_ignore_ascii_case("rel=canonical")
                });
                rel_canonical.then(|| {
                    target
                        .trim()
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_string()
                })
            })
        })
}

fn describe(e: &reqwest::Error) -> String {
    let kind = if e.is_timeout() {
        "timed out"
    } else if e.is_connect() {
        "could not connect"
    } else {
        "request failed"
    };
    // The source chain carries the resolver's policy message, if any.
    let mut detail = String::new();
    let mut src: Option<&dyn std::error::Error> = std::error::Error::source(e);
    while let Some(s) = src {
        detail = s.to_string();
        src = s.source();
    }
    if detail.is_empty() {
        kind.to_string()
    } else {
        format!("{kind}: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn parses_link_canonical() {
        let mut h = HeaderMap::new();
        h.insert(
            header::LINK,
            HeaderValue::from_static(
                r#"<https://x.com/a.css>; rel="preload", <https://x.com/page>; rel="canonical""#,
            ),
        );
        assert_eq!(
            canonical_from_link_header(&h).as_deref(),
            Some("https://x.com/page")
        );
    }

    #[test]
    fn classifies_status() {
        let h = HeaderMap::new();
        assert_eq!(classify(200, &h, true), FetchClass::Html);
        assert_eq!(classify(200, &h, false), FetchClass::NonHtml);
        assert_eq!(classify(404, &h, true), FetchClass::ClientError);
        assert_eq!(classify(403, &h, true), FetchClass::Blocked);
        assert_eq!(classify(429, &h, true), FetchClass::RateLimited);
        assert_eq!(classify(503, &h, true), FetchClass::ServerError);
    }
}
