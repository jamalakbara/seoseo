//! Internal browser API. Operator URL only; never accept an endpoint from a user.
use serde::Deserialize;
use std::time::Duration;
use url::Url;

#[derive(Deserialize)]
pub struct RenderResult {
    pub html: String,
    pub lighthouse: Option<serde_json::Value>,
    pub lighthouse_error: Option<String>,
}

pub async fn render(
    endpoint: Option<&str>,
    url: &Url,
    lighthouse: bool,
) -> anyhow::Result<RenderResult> {
    let endpoint = endpoint.ok_or_else(|| anyhow::anyhow!("RENDERER_URL is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(100))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    // The service serializes Chromium runs; concurrent audits wait briefly for their turn.
    for attempt in 0..12 {
        let response = client
            .post(format!("{}/render", endpoint.trim_end_matches('/')))
            .json(&serde_json::json!({"url": url, "lighthouse": lighthouse}))
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 11 {
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }
        anyhow::ensure!(
            response.status().is_success(),
            "renderer failed: {}",
            response.text().await?
        );
        return Ok(response.json().await?);
    }
    anyhow::bail!("renderer is busy")
}
