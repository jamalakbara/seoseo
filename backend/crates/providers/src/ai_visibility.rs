//! Plain model-answer comparisons through one operator-controlled compatible gateway.
//! These are API answers, not consumer chat sessions or independently grounded sources.
use crate::{ProviderError, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use utoipa::ToSchema;

#[derive(Clone)]
pub struct ModelGateway {
    endpoint: url::Url,
    key: Option<String>,
    models: Vec<String>,
}

impl ModelGateway {
    pub fn new(
        base: &str,
        key: Option<String>,
        models: Vec<String>,
    ) -> Result<Self, ProviderError> {
        let mut url = url::Url::parse(base.trim()).map_err(|_| {
            ProviderError::NotConfigured("Configure a valid OPENAI_COMPAT_BASE_URL".into())
        })?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ProviderError::NotConfigured("Model gateway must be an operator-controlled HTTP(S) URL without credentials, query or fragment".into()));
        }
        if models.is_empty()
            || models.len() > 10
            || models
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != models.len()
            || models
                .iter()
                .any(|m| m.trim().is_empty() || m.len() > 120 || m.chars().any(char::is_control))
        {
            return Err(ProviderError::NotConfigured(
                "Configure 1-10 unique model identifiers in AI_VISIBILITY_MODELS or LLM_MODEL"
                    .into(),
            ));
        }
        url.set_path(&format!(
            "{}/chat/completions",
            url.path().trim_end_matches('/')
        ));
        Ok(Self {
            endpoint: url,
            key,
            models,
        })
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }
    pub fn identity(&self) -> &str {
        self.endpoint.as_str()
    }

    pub async fn answer(
        &self,
        model: &str,
        prompt: &str,
        country: &str,
        language: &str,
    ) -> Result<String, ProviderError> {
        if !self.models.iter().any(|m| m == model) {
            return Err(ProviderError::NotConfigured(
                "This model is no longer approved by the operator".into(),
            ));
        }
        crate::http::pace("ai_visibility", Duration::from_secs(1)).await;
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| ProviderError::Unavailable("Model client initialization failed".into()))?;
        let mut request=client.post(self.endpoint.clone()).json(&json!({"model":model,"temperature":0,"max_tokens":1500,"stream":false,
            "messages":[{"role":"system","content":format!("Answer the user's question for country {country} in language {language}. Be factual and disclose uncertainty. You have no browsing tools in this request; do not claim to have searched the live web. Include URLs only if you know them; do not invent citations.")},{"role":"user","content":prompt}]}));
        if let Some(key) = self.key.as_deref().filter(|k| !k.is_empty()) {
            request = request.bearer_auth(key);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| ProviderError::Unavailable("Model request failed or timed out".into()))?;
        match response.status().as_u16() {
            200 => {}
            429 => return Err(ProviderError::RateLimited),
            401 | 403 => {
                return Err(ProviderError::Blocked(
                    "Model gateway denied access; check operator credentials".into(),
                ));
            }
            status => {
                return Err(ProviderError::Unavailable(format!(
                    "Model gateway returned HTTP {status}; no retry attempted"
                )));
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ProviderError::Unavailable("Model response was interrupted".into()))?
        {
            if bytes.len() + chunk.len() > 128 * 1024 {
                return Err(ProviderError::BadResponse(
                    "Model response exceeded 128 KiB".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        parse_answer(
            &serde_json::from_slice(&bytes)
                .map_err(|_| ProviderError::BadResponse("Model returned invalid JSON".into()))?,
        )
    }
}

fn parse_answer(value: &serde_json::Value) -> Result<String, ProviderError> {
    let choice = &value["choices"][0];
    if choice["finish_reason"].as_str() != Some("stop") {
        return Err(ProviderError::BadResponse(
            "Model answer was truncated, refused or did not finish normally".into(),
        ));
    }
    let text = choice["message"]["content"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ProviderError::BadResponse("Model returned no text answer".into()))?;
    if text.chars().count() > 20000 {
        return Err(ProviderError::BadResponse(
            "Model answer exceeded 20000 characters".into(),
        ));
    }
    Ok(text.to_string())
}

/// Literal case-insensitive mention with Unicode alphanumeric boundaries, not sentiment,
/// endorsement or recommendation ranking. Aliases are explicit tracked terms.
pub fn mentioned(answer: &str, term: &str) -> bool {
    let answer = answer.to_lowercase();
    let term = term.to_lowercase();
    !term.is_empty()
        && answer.match_indices(&term).any(|(start, _)| {
            let end = start + term.len();
            !answer[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
                && !answer[end..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric)
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiReportedSource {
    pub url: String,
    pub domain: String,
}

pub fn sources(answer: &str) -> Vec<AiReportedSource> {
    let mut out = Vec::new();
    for word in answer.split_whitespace() {
        let Some(start) = word.find("https://").or_else(|| word.find("http://")) else {
            continue;
        };
        let raw = word[start..]
            .split(['<', '>', '"'])
            .next()
            .unwrap_or("")
            .trim_end_matches([')', ']', '}', ',', '.', ';', ':', '!', '?']);
        if raw.len() > 2048 {
            continue;
        }
        let Ok(mut url) = url::Url::parse(raw) else {
            continue;
        };
        if crawler::UrlPolicy::default().check_url(&url).is_err() {
            continue;
        }
        url.set_fragment(None);
        let Some(domain) = url.host_str().map(str::to_string) else {
            continue;
        };
        if !out.iter().any(|s: &AiReportedSource| s.url == url.as_str()) {
            out.push(AiReportedSource {
                url: url.to_string(),
                domain,
            });
        }
        if out.len() == 20 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_mentions_have_boundaries_and_sources_are_unverified_safe_text() {
        assert!(mentioned("Try ACME, and Café.", "acme"));
        assert!(mentioned("Try Café.", "café"));
        assert!(!mentioned("acmeology and notacme", "acme"));
        assert!(!mentioned("nothing", ""));
        let urls = sources(
            "[one](https://example.com/path). https://example.com/path http://127.0.0.1/ https://u:p@example.com/ file:///tmp",
        );
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/path");
    }
    #[test]
    fn truncated_empty_or_tool_answers_are_not_measurements() {
        for reason in ["length", "content_filter", "tool_calls", ""] {
            assert!(
                parse_answer(
                    &json!({"choices":[{"finish_reason":reason,"message":{"content":"Acme"}}]})
                )
                .is_err()
            );
        }
        assert!(
            parse_answer(&json!({"choices":[{"finish_reason":"stop","message":{"content":""}}]}))
                .is_err()
        );
        assert_eq!(
            parse_answer(
                &json!({"choices":[{"finish_reason":"stop","message":{"content":"answer"}}]})
            )
            .unwrap(),
            "answer"
        );
        assert!(
            ModelGateway::new(
                "https://user:password@example.com/v1",
                None,
                vec!["test".into()]
            )
            .is_err()
        );
    }
}
