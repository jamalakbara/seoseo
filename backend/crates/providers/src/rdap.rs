//! Registry registration dates, discovered from the IANA DNS bootstrap (RFC 9224).
//! Never follows entity links, registrar referrals or redirects; no contact data is retained.
use crate::{ProviderError, http};
use chrono::{DateTime, Utc};
use crawler::{Fetcher, FetcherConfig, UrlPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use url::Url;
use utoipa::ToSchema;

pub const BOOTSTRAP_URL: &str = "https://data.iana.org/rdap/dns.json";
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RdapStatus {
    Found,
    NotFound,
    Unsupported,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RdapRecord {
    pub domain: String,
    pub status: RdapStatus,
    /// Registry's registration event, not the date the website first went online.
    pub registered_at: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub warning: Option<String>,
}

fn fetcher() -> Result<Fetcher, ProviderError> {
    Fetcher::new(FetcherConfig {
        user_agent: crate::USER_AGENT.into(),
        policy: UrlPolicy::default(),
        timeout: Duration::from_secs(15),
    })
    .map_err(|_| ProviderError::Unavailable("could not initialize RDAP client".into()))
}

async fn get(fetcher: &Fetcher, url: &Url) -> Result<(u16, Vec<u8>), ProviderError> {
    // The crawler client filters DNS at connect time, disables proxies and redirects,
    // and enforces a streaming size cap. HTTPS registry URLs come only from IANA.
    http::pace("rdap", Duration::from_secs(1)).await;
    let (status, bytes) = fetcher.get_exact_bytes(url, MAX_BYTES).await.map_err(|_| {
        ProviderError::Unavailable("RDAP request failed or exceeded the response limit".into())
    })?;
    match status {
        200 | 404 => Ok((status, bytes)),
        429 => Err(ProviderError::RateLimited),
        401 | 403 => Err(ProviderError::Blocked("registry denied RDAP access".into())),
        300..=399 => Err(ProviderError::Unavailable(
            "RDAP redirected the request; redirects are not followed".into(),
        )),
        _ => Err(ProviderError::Unavailable(format!(
            "RDAP returned HTTP {status}"
        ))),
    }
}

pub async fn bootstrap() -> Result<Value, ProviderError> {
    let (status, bytes) = get(&fetcher()?, &Url::parse(BOOTSTRAP_URL).expect("fixed URL")).await?;
    if status != 200 {
        return Err(ProviderError::Unavailable(
            "IANA RDAP bootstrap is unavailable".into(),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(http::bad_json)?;
    services(&value)?;
    Ok(value)
}

fn services(value: &Value) -> Result<&Vec<Value>, ProviderError> {
    value
        .get("services")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("IANA bootstrap has no services array".into()))
}

/// Longest label-boundary suffix match. Only public HTTPS endpoints without
/// credentials, unusual ports or query/fragment components are eligible.
fn endpoint(value: &Value, domain: &str) -> Result<Option<Url>, ProviderError> {
    let mut matches = Vec::new();
    for service in services(value)? {
        let (Some(suffixes), Some(urls)) = (
            service.get(0).and_then(Value::as_array),
            service.get(1).and_then(Value::as_array),
        ) else {
            continue;
        };
        for suffix in suffixes.iter().filter_map(Value::as_str) {
            if domain == suffix
                || domain
                    .strip_suffix(suffix)
                    .is_some_and(|prefix| prefix.ends_with('.'))
            {
                matches.push((suffix.len(), urls));
            }
        }
    }
    let Some(longest) = matches.iter().map(|(len, _)| *len).max() else {
        return Ok(None);
    };
    for (_, urls) in matches.into_iter().filter(|(len, _)| *len == longest) {
        for raw in urls.iter().filter_map(Value::as_str) {
            let Ok(mut base) = Url::parse(raw) else {
                continue;
            };
            if base.scheme() != "https"
                || base.port_or_known_default() != Some(443)
                || base.query().is_some()
                || base.fragment().is_some()
                || UrlPolicy::default().check_url(&base).is_err()
            {
                continue;
            }
            if !base.path().ends_with('/') {
                base.set_path(&format!("{}/", base.path()));
            }
            let Ok(url) = base.join(&format!("domain/{domain}")) else {
                continue;
            };
            return Ok(Some(url));
        }
    }
    Ok(None)
}

fn parse(
    domain: &str,
    source_url: &Url,
    value: &Value,
    now: DateTime<Utc>,
) -> Result<RdapRecord, ProviderError> {
    if value.get("objectClassName").and_then(Value::as_str) != Some("domain")
        || !value
            .get("ldhName")
            .and_then(Value::as_str)
            .is_some_and(|name| name.trim_end_matches('.').eq_ignore_ascii_case(domain))
    {
        return Err(ProviderError::BadResponse(
            "RDAP response does not describe the requested domain".into(),
        ));
    }
    let registered_at = value
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| event.get("eventAction").and_then(Value::as_str) == Some("registration"))
        .filter_map(|event| event.get("eventDate").and_then(Value::as_str))
        .filter_map(|date| {
            DateTime::parse_from_rfc3339(date)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        })
        .filter(|date| *date <= now)
        .min();
    Ok(RdapRecord { domain: domain.into(), status: RdapStatus::Found, registered_at,
        source_url: Some(source_url.to_string()), checked_at: now,
        warning: registered_at.is_none().then(|| "The registry did not provide a valid past registration event; domain age is unknown.".into()),
    })
}

pub async fn lookup(domain: &str, bootstrap: &Value) -> Result<RdapRecord, ProviderError> {
    // Domain is a validated ASCII registrable name from the domain service, never a URL.
    if domain.len() > 253
        || !domain.contains('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
    {
        return Err(ProviderError::BadResponse("invalid RDAP domain".into()));
    }
    let Some(url) = endpoint(bootstrap, domain)? else {
        return Ok(RdapRecord {
            domain: domain.into(),
            status: RdapStatus::Unsupported,
            registered_at: None,
            source_url: None,
            checked_at: Utc::now(),
            warning: Some(
                "IANA lists no supported public HTTPS RDAP service for this domain.".into(),
            ),
        });
    };
    let (status, bytes) = get(&fetcher()?, &url).await?;
    if status == 404 {
        return Ok(RdapRecord { domain: domain.into(), status: RdapStatus::NotFound, registered_at: None,
            source_url: Some(url.to_string()), checked_at: Utc::now(),
            warning: Some("The registry returned no domain record (404); this does not establish availability or a registration date.".into()) });
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(http::bad_json)?;
    parse(domain, &url, &value, Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn bounded_http_stops_on_blocks_redirects_and_large_bodies() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let fetcher = Fetcher::new(FetcherConfig {
            user_agent: crate::USER_AGENT.into(),
            policy: UrlPolicy {
                allow_private: true,
            },
            timeout: Duration::from_secs(3),
        })
        .unwrap();
        for status in [200, 404, 403, 429, 302] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                assert!(socket.read(&mut request).await.unwrap() > 0);
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Length: 2\r\nLocation: http://127.0.0.1:1/\r\nConnection: close\r\n\r\n{{}}"
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            });
            let result = get(
                &fetcher,
                &Url::parse(&format!("http://{address}/")).unwrap(),
            )
            .await;
            match status {
                200 | 404 => assert_eq!(result.unwrap().0, status),
                403 => assert!(matches!(result, Err(ProviderError::Blocked(_)))),
                429 => assert!(matches!(result, Err(ProviderError::RateLimited))),
                302 => assert!(
                    matches!(result, Err(ProviderError::Unavailable(message)) if message.contains("redirect"))
                ),
                _ => unreachable!(),
            }
            task.await.unwrap();
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_BYTES + 1
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            let _ = socket.write_all(&vec![b'x'; MAX_BYTES + 1]).await;
        });
        assert!(
            get(
                &fetcher,
                &Url::parse(&format!("http://{address}/")).unwrap()
            )
            .await
            .is_err()
        );
        task.await.unwrap();
    }

    #[test]
    fn authoritative_longest_suffix_and_safe_endpoints() {
        let data = json!({"services":[[["com"],["https://rdap.example.com/v1/"]],[["test.com"],["https://special.example.com/base"]]]});
        assert_eq!(
            endpoint(&data, "one.test.com").unwrap().unwrap().as_str(),
            "https://special.example.com/base/domain/one.test.com"
        );
        assert!(endpoint(&data, "one.notcom").unwrap().is_none());
        for base in [
            "http://rdap.example.com/",
            "https://localhost/",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://u:p@rdap.example.com/",
            "https://rdap.example.com:8443/",
            "https://rdap.example.com/?x=1",
            "https://rdap.example.com/#x",
        ] {
            assert!(
                endpoint(&json!({"services":[[["com"],[base]]]}), "example.com")
                    .unwrap()
                    .is_none(),
                "{base}"
            );
        }
        assert!(endpoint(&json!({}), "example.com").is_err());
    }

    #[test]
    fn registration_events_only_and_no_contacts_retained() {
        let now = DateTime::parse_from_rfc3339("2026-11-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let url = Url::parse("https://rdap.example.com/domain/example.com").unwrap();
        let mut value = json!({"objectClassName":"domain","ldhName":"EXAMPLE.COM.","events":[
            {"eventAction":"last changed","eventDate":"1990-01-01T00:00:00Z"},
            {"eventAction":"registration","eventDate":"2000-01-02T03:00:00+03:00"},
            {"eventAction":"registration","eventDate":"2030-01-01T00:00:00Z"},
            {"eventAction":"registration","eventDate":"invalid"}],"entities":[{"contact":"private@example.com"}]});
        let r = parse("example.com", &url, &value, now).unwrap();
        assert_eq!(
            r.registered_at.unwrap().to_rfc3339(),
            "2000-01-02T00:00:00+00:00"
        );
        assert!(
            !serde_json::to_string(&r)
                .unwrap()
                .contains("private@example.com")
        );
        value["events"] =
            json!([{"eventAction":"last changed","eventDate":"2000-01-01T00:00:00Z"}]);
        assert!(
            parse("example.com", &url, &value, now)
                .unwrap()
                .registered_at
                .is_none()
        );
        value["ldhName"] = json!("other.com");
        assert!(parse("example.com", &url, &value, now).is_err());
        value["objectClassName"] = json!("entity");
        assert!(parse("example.com", &url, &value, now).is_err());
    }
}
