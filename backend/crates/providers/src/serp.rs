//! Search results providers. SearXNG (self-hosted, free) is the default; Brave and Serper are
//! optional bring-your-own-key APIs. None of these scrape a search engine's HTML.

use crate::http::{bad_json, check, client, net_err, pace, text, url};
use crate::{Market, ProviderError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use utoipa::ToSchema;

const MAX_BODY: usize = 4 * 1024 * 1024;
pub const MAX_ITEMS: usize = 20;

#[derive(Debug, Clone)]
pub struct SerpQuery {
    pub keyword: String,
    pub market: Market,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SerpItem {
    pub position: i32,
    pub url: String,
    pub title: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerpPage {
    pub provider: &'static str,
    /// True when positions are a real search engine's ranking (not a meta-search blend).
    pub exact: bool,
    pub items: Vec<SerpItem>,
    /// Detected SERP features, e.g. `answer_box`, `people_also_ask`.
    pub features: Vec<String>,
    pub related: Vec<String>,
    pub questions: Vec<String>,
}

#[async_trait]
pub trait SerpProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError>;
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
}

fn non_empty(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        _ => false,
    }
}

fn strings(v: &Value, key: &str, field: Option<&str>) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| match field {
                    Some(f) => s(x, f),
                    None => x.as_str().map(|t| t.trim().to_string()),
                })
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn items<'a>(
    rows: impl Iterator<Item = &'a Value>,
    url_key: &str,
    snippet_key: &str,
) -> Vec<SerpItem> {
    let mut out: Vec<SerpItem> = Vec::new();
    for r in rows {
        let Some(u) = s(r, url_key).filter(|u| u.starts_with("http")) else {
            continue;
        };
        if out.iter().any(|i| i.url == u) {
            continue;
        }
        out.push(SerpItem {
            position: out.len() as i32 + 1,
            title: s(r, "title").unwrap_or_else(|| u.clone()),
            snippet: s(r, snippet_key),
            url: u,
        });
        if out.len() >= MAX_ITEMS {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- SearXNG

pub struct Searxng {
    base: String,
}

impl Searxng {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
        }
    }
}

pub fn parse_searxng(body: &str) -> Result<SerpPage, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let rows = v
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("missing results".into()))?;
    let items = items(rows.iter(), "url", "content");
    if items.is_empty() {
        let dead = v
            .get("unresponsive_engines")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.get(0).and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if !dead.is_empty() {
            return Err(ProviderError::Unavailable(format!(
                "SearXNG engines did not respond ({dead})"
            )));
        }
    }
    let mut features = Vec::new();
    if non_empty(&v, "answers") {
        features.push("answer_box".to_string());
    }
    if non_empty(&v, "infoboxes") {
        features.push("knowledge_panel".to_string());
    }
    let related = strings(&v, "suggestions", None);
    if !related.is_empty() {
        features.push("related_searches".to_string());
    }
    Ok(SerpPage {
        provider: "serp_searxng",
        exact: false,
        items,
        features,
        related,
        questions: Vec::new(),
    })
}

#[async_trait]
impl SerpProvider for Searxng {
    fn id(&self) -> &'static str {
        "serp_searxng"
    }

    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError> {
        let u = url(
            &format!("{}/search", self.base),
            &[
                ("q", q.keyword.as_str()),
                ("format", "json"),
                ("categories", "general"),
                ("language", &q.market.locale()),
                ("safesearch", "0"),
                ("pageno", "1"),
            ],
        )?;
        pace(self.id(), Duration::from_millis(1500)).await;
        let res = client().get(u).send().await.map_err(net_err)?;
        if res.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::NotConfigured(
                "SearXNG refused the JSON format; add `json` to search.formats in its settings.yml"
                    .into(),
            ));
        }
        parse_searxng(&text(check(res)?, MAX_BODY).await?)
    }
}

// ---------------------------------------------------------------- Brave Search API

pub struct Brave {
    key: String,
}

impl Brave {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}

pub fn parse_brave(body: &str) -> Result<SerpPage, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let rows = v
        .pointer("/web/results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut features = Vec::new();
    let questions = v
        .get("faq")
        .map(|f| strings(f, "results", Some("question")))
        .unwrap_or_default();
    if !questions.is_empty() {
        features.push("people_also_ask".to_string());
    }
    for (key, name) in [
        ("infobox", "knowledge_panel"),
        ("videos", "videos"),
        ("news", "top_stories"),
        ("locations", "local_pack"),
        ("discussions", "discussions"),
    ] {
        if v.get(key).is_some_and(|x| !x.is_null()) {
            features.push(name.to_string());
        }
    }
    Ok(SerpPage {
        provider: "serp_brave",
        exact: false,
        items: items(rows.iter(), "url", "description"),
        features,
        related: Vec::new(),
        questions,
    })
}

#[async_trait]
impl SerpProvider for Brave {
    fn id(&self) -> &'static str {
        "serp_brave"
    }

    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError> {
        let u = url(
            "https://api.search.brave.com/res/v1/web/search",
            &[
                ("q", q.keyword.as_str()),
                ("country", &q.market.country),
                ("search_lang", q.market.lang()),
                ("count", "20"),
            ],
        )?;
        // The free plan allows one request per second.
        pace(self.id(), Duration::from_millis(1100)).await;
        let res = client()
            .get(u)
            .header("X-Subscription-Token", &self.key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(net_err)?;
        parse_brave(&text(check(res)?, MAX_BODY).await?)
    }
}

// ---------------------------------------------------------------- Serper (Google results API)

pub struct Serper {
    key: String,
}

impl Serper {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}

pub fn parse_serper(body: &str) -> Result<SerpPage, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let rows = v
        .get("organic")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("missing organic results".into()))?;
    let questions = strings(&v, "peopleAlsoAsk", Some("question"));
    let related = strings(&v, "relatedSearches", Some("query"));
    let mut features = Vec::new();
    for (key, name) in [
        ("answerBox", "answer_box"),
        ("knowledgeGraph", "knowledge_panel"),
        ("peopleAlsoAsk", "people_also_ask"),
        ("relatedSearches", "related_searches"),
        ("topStories", "top_stories"),
        ("videos", "videos"),
        ("images", "images"),
        ("places", "local_pack"),
        ("shopping", "shopping"),
    ] {
        if non_empty(&v, key) {
            features.push(name.to_string());
        }
    }
    Ok(SerpPage {
        provider: "serp_serper",
        exact: true,
        items: items(rows.iter(), "link", "snippet"),
        features,
        related,
        questions,
    })
}

#[async_trait]
impl SerpProvider for Serper {
    fn id(&self) -> &'static str {
        "serp_serper"
    }

    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError> {
        pace(self.id(), Duration::from_millis(250)).await;
        let res = client()
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &self.key)
            .json(&serde_json::json!({
                "q": q.keyword,
                "gl": q.market.country.to_ascii_lowercase(),
                "hl": q.market.lang(),
                "num": 20,
            }))
            .send()
            .await
            .map_err(net_err)?;
        parse_serper(&text(check(res)?, MAX_BODY).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_searxng() {
        let body = r#"{"query":"x","results":[
            {"url":"https://a.com/1","title":"A","content":"about a","engine":"bing"},
            {"url":"https://a.com/1","title":"A dup"},
            {"url":"ftp://skip","title":"no"},
            {"url":"https://b.org/","title":"B"}],
            "answers":["42"],"infoboxes":[],"suggestions":["x vs y"],"unresponsive_engines":[]}"#;
        let p = parse_searxng(body).unwrap();
        assert!(!p.exact);
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.items[1].position, 2);
        assert_eq!(p.items[0].snippet.as_deref(), Some("about a"));
        assert_eq!(p.features, vec!["answer_box", "related_searches"]);
        assert_eq!(p.related, vec!["x vs y"]);
    }

    #[test]
    fn searxng_with_dead_engines_is_unavailable() {
        let body =
            r#"{"results":[],"unresponsive_engines":[["google","CAPTCHA"],["bing","timeout"]]}"#;
        assert!(matches!(
            parse_searxng(body),
            Err(ProviderError::Unavailable(m)) if m.contains("google, bing")
        ));
        // No results and no dead engines is a valid empty SERP.
        assert!(parse_searxng(r#"{"results":[]}"#).unwrap().items.is_empty());
    }

    #[test]
    fn parses_serper_and_brave() {
        let serper = r#"{"organic":[{"title":"T","link":"https://x.com/","snippet":"s","position":1}],
            "peopleAlsoAsk":[{"question":"why?"}],"relatedSearches":[{"query":"x alt"}],
            "answerBox":{"answer":"a"},"knowledgeGraph":{}}"#;
        let p = parse_serper(serper).unwrap();
        assert!(p.exact);
        assert_eq!(p.items[0].url, "https://x.com/");
        assert_eq!(p.questions, vec!["why?"]);
        assert_eq!(
            p.features,
            vec!["answer_box", "people_also_ask", "related_searches"]
        );

        let brave = r#"{"web":{"results":[{"title":"T","url":"https://y.com/","description":"d"}]},
            "faq":{"results":[{"question":"how?"}]},"videos":{"results":[]}}"#;
        let p = parse_brave(brave).unwrap();
        assert_eq!(p.items[0].snippet.as_deref(), Some("d"));
        assert_eq!(p.features, vec!["people_also_ask", "videos"]);
    }
}
