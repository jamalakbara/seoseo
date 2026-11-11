//! Explicitly enabled direct Bing results. No stealth flags, CAPTCHA solving or proxy rotation.
use crate::{ProviderError, SerpItem, SerpPage, SerpProvider, SerpQuery};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

pub struct Chromium {
    endpoint: String,
}
impl Chromium {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').into(),
        }
    }
}

#[derive(Deserialize)]
struct Response {
    items: Vec<SerpItem>,
    related: Vec<String>,
    features: Vec<String>,
    questions: Vec<String>,
}

#[async_trait]
impl SerpProvider for Chromium {
    fn id(&self) -> &'static str {
        "serp_chromium"
    }
    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError> {
        let fetcher = crawler::Fetcher::new(crawler::FetcherConfig {
            user_agent: crate::USER_AGENT.into(),
            ..Default::default()
        })
        .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        let url = url::Url::parse("https://www.bing.com/search").expect("fixed URL");
        let robots = crawler::robots::Robots::fetch(&fetcher, &url, crawler::ROBOTS_TOKEN).await;
        if !matches!(robots.status, Some(200..=299) | Some(404)) {
            return Err(ProviderError::Blocked(
                "could not safely read Bing robots.txt".into(),
            ));
        }
        if !robots.allowed(&url) {
            return Err(ProviderError::Blocked(
                "Bing robots.txt disallows search scraping".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(100))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        let response = client.post(format!("{}/render", self.endpoint))
            .json(&serde_json::json!({"mode":"serp", "keyword":q.keyword, "country":q.market.country, "language":q.market.language}))
            .send().await.map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        if response.status().as_u16() == 429 {
            return Err(ProviderError::RateLimited);
        }
        if !response.status().is_success() {
            return Err(ProviderError::Blocked(
                "browser search unavailable or challenged; no bypass attempted".into(),
            ));
        }
        let r: Response = response
            .json()
            .await
            .map_err(|e| ProviderError::BadResponse(e.to_string()))?;
        Ok(SerpPage {
            provider: self.id(),
            exact: true,
            items: r.items,
            related: r.related,
            features: r.features,
            questions: r.questions,
        })
    }
}
