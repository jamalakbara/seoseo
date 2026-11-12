//! Engine-specific AI requests and citation provenance. Endpoints and credentials
//! are supplied by the operator, never by project/tracker input.
use crate::{
    ProviderError, USER_AGENT,
    ai_visibility::{AiReportedSource, ModelGateway, sources},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiCitation {
    pub url: String,
    pub title: Option<String>,
}
pub struct AiEngineAnswer {
    pub text: String,
    pub engine: &'static str,
    pub grounding: &'static str,
    pub citations: Vec<AiCitation>,
}
#[derive(Clone)]
pub struct NativeEngine {
    pub id: String,
    pub kind: &'static str,
    model: String,
    endpoint: url::Url,
    key: Option<String>,
    ollama: Option<ModelGateway>,
}
impl NativeEngine {
    pub fn new(
        kind: &'static str,
        model: &str,
        base: &str,
        key: Option<String>,
    ) -> Result<Self, ProviderError> {
        if model.is_empty()
            || model.len() > 120
            || !model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
        {
            return Err(ProviderError::NotConfigured(
                "Invalid configured engine model identifier".into(),
            ));
        }
        // Reuse the strict operator-URL checks; the client never follows redirects/proxies.
        let checked = ModelGateway::new(base, key.clone(), vec![model.into()])?;
        let endpoint = match kind {
            "ollama" => url::Url::parse(checked.identity()).expect("validated"),
            "anthropic" => url::Url::parse(&format!("{}/messages", base.trim_end_matches('/')))
                .map_err(|_| ProviderError::NotConfigured("Invalid Anthropic URL".into()))?,
            "gemini" => url::Url::parse(&format!(
                "{}/models/{model}:generateContent",
                base.trim_end_matches('/')
            ))
            .map_err(|_| ProviderError::NotConfigured("Invalid Gemini URL".into()))?,
            _ => return Err(ProviderError::NotConfigured("Unsupported AI engine".into())),
        };
        if kind != "ollama" && key.as_deref().is_none_or(str::is_empty) {
            return Err(ProviderError::NotConfigured(format!(
                "Configure the {kind} API key"
            )));
        }
        Ok(Self {
            id: format!("{kind}/{model}"),
            kind,
            model: model.into(),
            endpoint,
            key,
            ollama: (kind == "ollama").then_some(checked),
        })
    }
    async fn answer(
        &self,
        prompt: &str,
        country: &str,
        language: &str,
    ) -> Result<AiEngineAnswer, ProviderError> {
        if let Some(gateway) = &self.ollama {
            return Ok(AiEngineAnswer {
                text: gateway
                    .answer(&self.model, prompt, country, language)
                    .await?,
                engine: "ollama",
                grounding: "none",
                citations: vec![],
            });
        }
        crate::http::pace("ai_native", Duration::from_secs(1)).await;
        let context = format!(
            "Answer for country {country} in language {language}. Disclose uncertainty and do not invent facts or citations. {}",
            if self.kind == "anthropic" {
                "Use web search for current facts and cite sources."
            } else {
                "You have no web search tools; do not claim live web verification."
            }
        );
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| {
                ProviderError::Unavailable("Engine client initialization failed".into())
            })?;
        let request = if self.kind == "anthropic" {
            client.post(self.endpoint.clone()).header("x-api-key",self.key.as_deref().unwrap_or("")).header("anthropic-version","2023-06-01")
                .json(&json!({"model":self.model,"max_tokens":2000,"system":context,"messages":[{"role":"user","content":prompt}],"tools":[{"type":"web_search_20250305","name":"web_search","max_uses":2}]}))
        } else {
            client.post(self.endpoint.clone()).header("x-goog-api-key",self.key.as_deref().unwrap_or(""))
                .json(&json!({"systemInstruction":{"parts":[{"text":context}]},"contents":[{"role":"user","parts":[{"text":prompt}]}],"generationConfig":{"maxOutputTokens":2000}}))
        };
        let mut response = request.send().await.map_err(|_| {
            ProviderError::Unavailable("AI engine request failed or timed out".into())
        })?;
        match response.status().as_u16() {
            200 => {}
            429 => return Err(ProviderError::RateLimited),
            401 | 403 => {
                return Err(ProviderError::Blocked(
                    "AI engine refused access; check operator credentials and permissions".into(),
                ));
            }
            status => {
                return Err(ProviderError::Unavailable(format!(
                    "AI engine returned HTTP {status}"
                )));
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ProviderError::Unavailable("AI engine response interrupted".into()))?
        {
            if bytes.len() + chunk.len() > 256 * 1024 {
                return Err(ProviderError::BadResponse(
                    "AI engine response too large".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        parse(
            self.kind,
            &serde_json::from_slice(&bytes)
                .map_err(|_| ProviderError::BadResponse("Invalid AI engine JSON".into()))?,
        )
    }
}

#[derive(Clone)]
pub struct EngineGateway {
    compatible: Option<ModelGateway>,
    native: Vec<NativeEngine>,
    models: Vec<String>,
    identity: String,
}
impl EngineGateway {
    pub fn new(
        compatible: Option<ModelGateway>,
        native: Vec<NativeEngine>,
    ) -> Result<Self, ProviderError> {
        let mut models = compatible.as_ref().map_or(vec![], |c| c.models().to_vec());
        models.extend(native.iter().map(|e| e.id.clone()));
        if models.is_empty()
            || models.len() > 20
            || models
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != models.len()
        {
            return Err(ProviderError::NotConfigured(
                "Configure 1-20 unique approved AI model/engine IDs".into(),
            ));
        }
        let identity = if native.is_empty() {
            compatible
                .as_ref()
                .expect("nonempty")
                .identity()
                .to_string()
        } else {
            json!({"compatible":compatible.as_ref().map(ModelGateway::identity),"engines":native.iter().map(|e|(&e.id,e.endpoint.as_str())).collect::<Vec<_>>()}).to_string()
        };
        Ok(Self {
            compatible,
            native,
            models,
            identity,
        })
    }
    pub fn models(&self) -> &[String] {
        &self.models
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn engine(&self, model: &str) -> &'static str {
        self.native
            .iter()
            .find(|e| e.id == model)
            .map_or("compatible", |e| e.kind)
    }
    pub async fn answer(
        &self,
        model: &str,
        prompt: &str,
        country: &str,
        language: &str,
    ) -> Result<AiEngineAnswer, ProviderError> {
        if let Some(e) = self.native.iter().find(|e| e.id == model) {
            return e.answer(prompt, country, language).await;
        }
        let gateway = self.compatible.as_ref().ok_or_else(|| {
            ProviderError::NotConfigured("The requested model is not available".into())
        })?;
        Ok(AiEngineAnswer {
            text: gateway.answer(model, prompt, country, language).await?,
            engine: "compatible",
            grounding: "none",
            citations: vec![],
        })
    }
}

fn citation(url: &str, title: Option<&str>) -> Option<AiCitation> {
    let parsed = url::Url::parse(url).ok()?;
    if url.len() > 2048 || crawler::UrlPolicy::default().check_url(&parsed).is_err() {
        return None;
    }
    Some(AiCitation {
        url: parsed.to_string(),
        title: title.map(|t| t.chars().take(200).collect()),
    })
}
fn parse(kind: &'static str, v: &Value) -> Result<AiEngineAnswer, ProviderError> {
    let mut citations = Vec::new();
    let text = if kind == "anthropic" {
        if v["stop_reason"] != "end_turn" {
            return Err(ProviderError::BadResponse(
                "Anthropic answer did not finish normally".into(),
            ));
        }
        let blocks = v["content"]
            .as_array()
            .ok_or_else(|| ProviderError::BadResponse("Anthropic returned no content".into()))?;
        let mut parts = Vec::new();
        for block in blocks {
            if block["type"] == "web_search_tool_result"
                && block["content"]["type"] == "web_search_tool_result_error"
            {
                return Err(ProviderError::Unavailable(
                    "Anthropic web search reported an error".into(),
                ));
            }
            if block["type"] == "text" {
                if let Some(t) = block["text"].as_str() {
                    parts.push(t);
                }
                for c in block["citations"].as_array().into_iter().flatten() {
                    if let Some(c) = c["url"]
                        .as_str()
                        .and_then(|u| citation(u, c["title"].as_str()))
                    {
                        citations.push(c);
                    }
                }
            }
        }
        parts.join("\n")
    } else {
        let candidate = &v["candidates"][0];
        if !candidate["groundingMetadata"].is_null() {
            return Err(ProviderError::BadResponse("Unexpected Gemini grounding metadata; stored comparisons do not request Google Search grounding".into()));
        }
        if candidate["finishReason"] != "STOP" {
            return Err(ProviderError::BadResponse(
                "Gemini answer was blocked, truncated or incomplete".into(),
            ));
        }
        candidate["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["thought"] != true)
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };
    if text.trim().is_empty() || text.chars().count() > 20000 {
        return Err(ProviderError::BadResponse(
            "AI engine returned an empty or oversized answer".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    citations.retain(|c| seen.insert(c.url.clone()));
    citations.truncate(20);
    let grounding = if kind == "gemini" {
        "none"
    } else if citations.is_empty() {
        "requested_no_citations"
    } else {
        "provider_citations"
    };
    Ok(AiEngineAnswer {
        text,
        engine: kind,
        grounding,
        citations,
    })
}

// Keep extraction available to downstream callers without treating text URLs as citations.
pub fn reported_sources(answer: &AiEngineAnswer) -> Vec<AiReportedSource> {
    sources(&answer.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_contracts_preserve_citation_provenance_and_fail_closed() {
        let a=parse("anthropic",&json!({"stop_reason":"end_turn","content":[{"type":"text","text":"Acme","citations":[{"url":"https://example.com/","title":"Source"}]}]})).unwrap();
        assert_eq!(a.citations.len(), 1);
        assert_eq!(a.grounding, "provider_citations");
        let g = parse(
            "gemini",
            &json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"Acme"}]}}]}),
        )
        .unwrap();
        assert_eq!(g.engine, "gemini");
        assert!(g.citations.is_empty());
        assert_eq!(g.grounding, "none");
        assert!(
            parse(
                "gemini",
                &json!({"candidates":[{"finishReason":"STOP","groundingMetadata":{}}]})
            )
            .is_err()
        );
        assert!(parse("anthropic", &json!({"stop_reason":"pause_turn"})).is_err());
        assert!(
            parse(
                "gemini",
                &json!({"candidates":[{"finishReason":"MAX_TOKENS"}]})
            )
            .is_err()
        );
        assert!(citation("http://127.0.0.1/", None).is_none());
    }
}
