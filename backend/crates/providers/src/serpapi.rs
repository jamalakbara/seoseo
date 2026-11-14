//! Optional BYOK Maps and Google AI Overview transport. Never follows provider links.
use crate::ProviderError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LocalResult {
    pub position: i16,
    pub business_id: String,
    pub name: String,
    pub address: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OverviewSource {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Overview {
    /// present, absent, or incomplete (a continuation was not resolved).
    pub status: String,
    pub text: Option<String>,
    pub sources: Vec<OverviewSource>,
}

pub fn public_link(raw: &str) -> Option<String> {
    if raw.len() > 2048 {
        return None;
    }
    let u = url::Url::parse(raw).ok()?;
    (matches!(u.scheme(), "http" | "https")
        && u.host_str().is_some()
        && u.username().is_empty()
        && u.password().is_none()
        && u.as_str().len() <= 2048)
        .then(|| u.to_string())
}

pub fn maps(v: &Value) -> Result<Vec<LocalResult>, ProviderError> {
    let rows = v
        .get("local_results")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("missing local_results array".into()))?;
    let mut out: Vec<LocalResult> = Vec::new();
    for r in rows.iter().take(20) {
        let position = r["position"].as_i64().filter(|n| (1..=20).contains(n));
        let id = r["place_id"].as_str().or_else(|| r["data_id"].as_str());
        let name = r["title"].as_str();
        let (Some(position), Some(id), Some(name)) = (position, id, name) else {
            return Err(ProviderError::BadResponse(
                "local result lacks position or identity".into(),
            ));
        };
        if id.is_empty()
            || id.len() > 160
            || name.is_empty()
            || name.len() > 500
            || out
                .iter()
                .any(|x| x.business_id == id || i64::from(x.position) == position)
        {
            return Err(ProviderError::BadResponse(
                "invalid or duplicate local result".into(),
            ));
        }
        out.push(LocalResult {
            position: position as i16,
            business_id: id.into(),
            name: name.into(),
            address: r["address"].as_str().map(|s| s.chars().take(500).collect()),
            website: r["website"].as_str().and_then(public_link),
        });
    }
    Ok(out)
}

pub fn overview(v: &Value) -> Result<Overview, ProviderError> {
    if v.get("error").is_some() {
        return Err(ProviderError::BadResponse(
            "AI Overview provider returned an error".into(),
        ));
    }
    let Some(a) = v.get("ai_overview") else {
        // A successful search envelope is required to assert observed absence.
        if v.pointer("/search_metadata/status").and_then(Value::as_str) != Some("Success") {
            return Err(ProviderError::BadResponse(
                "missing successful search metadata".into(),
            ));
        }
        return Ok(Overview {
            status: "absent".into(),
            text: None,
            sources: vec![],
        });
    };
    let mut snippets = Vec::new();
    fn walk(v: &Value, snippets: &mut Vec<String>, depth: usize) {
        if depth > 12 || snippets.len() >= 200 {
            return;
        }
        match v {
            Value::Object(o) => {
                if let Some(s) = o.get("snippet").and_then(Value::as_str) {
                    snippets.push(s.chars().take(2000).collect());
                }
                for key in ["text_blocks", "list", "items"] {
                    if let Some(x) = o.get(key) {
                        walk(x, snippets, depth + 1);
                    }
                }
            }
            Value::Array(rows) => {
                for x in rows.iter().take(200) {
                    walk(x, snippets, depth + 1);
                }
            }
            _ => {}
        }
    }
    walk(a, &mut snippets, 0);
    let text: String = snippets.join("\n").chars().take(20_000).collect();
    let sources = a["references"]
        .as_array()
        .into_iter()
        .flatten()
        .take(50)
        .filter_map(|r| {
            Some(OverviewSource {
                url: public_link(r["link"].as_str()?)?,
                title: r["title"]
                    .as_str()
                    .unwrap_or("Source")
                    .chars()
                    .take(300)
                    .collect(),
            })
        })
        .collect();
    Ok(Overview {
        status: if text.is_empty() {
            "incomplete"
        } else {
            "present"
        }
        .into(),
        text: (!text.is_empty()).then_some(text),
        sources,
    })
}

/// Endpoint is operator-controlled; callers cannot supply it. Errors never contain URLs/keys/bodies.
pub async fn request(
    endpoint: &str,
    key: &str,
    params: &[(&str, String)],
) -> Result<Value, ProviderError> {
    crate::http::pace("serpapi", std::time::Duration::from_millis(1000)).await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(25))
        .user_agent(crate::USER_AGENT)
        .build()
        .map_err(|_| ProviderError::Unavailable("could not create provider client".into()))?;
    let mut url = url::Url::parse(endpoint)
        .map_err(|_| ProviderError::NotConfigured("invalid operator endpoint".into()))?;
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params {
            query.append_pair(name, value);
        }
        query.append_pair("api_key", key);
    }
    let mut response = client.get(url).send().await.map_err(crate::http::net_err)?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            429 => ProviderError::RateLimited,
            401 | 403 => ProviderError::Blocked("provider access rejected; check your key".into()),
            _ => ProviderError::Unavailable("provider request failed".into()),
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(crate::http::net_err)? {
        if body.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(ProviderError::BadResponse(
                "provider response too large".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let v: Value = serde_json::from_slice(&body)
        .map_err(|_| ProviderError::BadResponse("provider returned invalid JSON".into()))?;
    if v.get("error").is_some() {
        return Err(ProviderError::Unavailable(
            "provider could not complete this search".into(),
        ));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn overview_preserves_unknown_and_nested_text() {
        assert!(overview(&json!({})).is_err());
        assert_eq!(
            overview(&json!({"search_metadata":{"status":"Success"}}))
                .unwrap()
                .status,
            "absent"
        );
        assert_eq!(
            overview(&json!({"ai_overview":{"page_token":"secret"}}))
                .unwrap()
                .status,
            "incomplete"
        );
        let a = overview(
            &json!({"ai_overview":{"text_blocks":[{"snippet":"A","list":[{"snippet":"B"}]}],
            "references":[{"title":"bad","link":"javascript:alert(1)"}]}}),
        )
        .unwrap();
        assert_eq!(a.text.as_deref(), Some("A\nB"));
        assert!(a.sources.is_empty());
    }
    #[test]
    fn maps_preserves_positions_and_rejects_duplicates() {
        assert!(public_link(&format!("https://example.com/{}", "x".repeat(2048))).is_none());
        let row = json!({"position":4,"place_id":"abc","title":"Cafe","website":"javascript:x"});
        let result = maps(&json!({"local_results":[row.clone()]})).unwrap();
        assert_eq!(result[0].position, 4);
        assert!(result[0].website.is_none());
        assert!(maps(&json!({"local_results":[row.clone(),row]})).is_err());
        assert!(maps(&json!({})).is_err());
    }
}
