//! Bounded OpenAI-compatible SSE transport for assistant JSON actions.
use crate::{
    ProviderError, USER_AGENT,
    ai_visibility::{ModelGateway, assistant_messages},
};
use serde_json::{Value, json};
use std::{future::Future, time::Duration};

fn bad(message: &str) -> ProviderError {
    ProviderError::BadResponse(message.into())
}

impl ModelGateway {
    pub async fn answer_json_stream<F, Fut>(
        &self,
        model: &str,
        prompt: &str,
        country: &str,
        language: &str,
        mut update: F,
    ) -> Result<String, ProviderError>
    where
        F: FnMut(String) -> Fut + Send,
        Fut: Future<Output = Result<(), ProviderError>> + Send,
    {
        if !self.models.iter().any(|m| m == model) {
            return Err(ProviderError::NotConfigured(
                "This model is no longer approved".into(),
            ));
        }
        crate::http::pace("ai_visibility", Duration::from_secs(1)).await;
        let context = format!(
            "Answer for country {country} in language {language}. Be factual and disclose uncertainty. You have no browsing tools; do not claim live web verification or invent citations."
        );
        let messages = assistant_messages(&context, prompt)?;
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| bad("Model client initialization failed"))?;
        let mut request=client.post(self.endpoint.clone()).json(&json!({"model":model,"messages":messages,"temperature":0,"max_tokens":2000,"stream":true,"response_format":{"type":"json_object"}}));
        if let Some(key) = self.key.as_deref().filter(|k| !k.is_empty()) {
            request = request.bearer_auth(key);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| ProviderError::Unavailable("Model stream failed or timed out".into()))?;
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
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s.starts_with("text/event-stream"))
        {
            return Err(bad(
                "Assistant requires a gateway supporting streamed chat completions",
            ));
        }
        let mut decoder = Decoder::default();
        while let Some(chunk) = response.chunk().await.map_err(|_| {
            ProviderError::Unavailable("Model stream was interrupted or timed out".into())
        })? {
            if let Some(text) = decoder.feed(&chunk)? {
                update(text).await?;
            }
            if decoder.done {
                break;
            }
        }
        decoder.finish()
    }
}

#[derive(Default)]
struct Decoder {
    pending: Vec<u8>,
    data: Vec<u8>,
    wire: usize,
    text: String,
    done: bool,
    stopped: bool,
}
impl Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Result<Option<String>, ProviderError> {
        self.wire += bytes.len();
        if self.wire > 1024 * 1024 {
            return Err(bad("Model stream exceeded 1 MiB"));
        }
        self.pending.extend_from_slice(bytes);
        let mut changed = false;
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if self.data.is_empty() {
                    continue;
                }
                let data = std::mem::take(&mut self.data);
                let raw = std::str::from_utf8(&data)
                    .map_err(|_| bad("Invalid UTF-8 in model stream"))?
                    .trim();
                if raw == "[DONE]" {
                    self.done = true;
                    break;
                }
                let value: Value =
                    serde_json::from_str(raw).map_err(|_| bad("Invalid JSON in model stream"))?;
                let choice = &value["choices"][0];
                if choice.is_null() {
                    if value.get("usage").is_some() {
                        continue;
                    }
                    return Err(bad("Model stream has no answer choice"));
                }
                if choice["delta"].get("tool_calls").is_some()
                    || choice["delta"].get("refusal").is_some_and(|v| !v.is_null())
                {
                    return Err(bad(
                        "Model stream refused or returned unsupported tool calls",
                    ));
                }
                if let Some(reason) = choice["finish_reason"].as_str() {
                    if reason != "stop" {
                        return Err(bad("Model stream was truncated or did not finish normally"));
                    }
                    self.stopped = true;
                }
                if let Some(delta) = choice["delta"]["content"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                {
                    if self.text.len() + delta.len() > 80000 {
                        return Err(bad("Model action exceeded 20000 characters"));
                    }
                    self.text.push_str(delta);
                    if self.text.chars().count() > 20000 {
                        return Err(bad("Model action exceeded 20000 characters"));
                    }
                    changed = true;
                }
            } else if let Some(data) = line.strip_prefix(b"data:") {
                let data = data.strip_prefix(b" ").unwrap_or(data);
                if !self.data.is_empty() {
                    self.data.push(b'\n');
                }
                self.data.extend_from_slice(data);
            }
            if self.data.len() > 128 * 1024 {
                return Err(bad("Model stream event exceeded 128 KiB"));
            }
        }
        if self.pending.len() > 128 * 1024 {
            return Err(bad("Model stream line exceeded 128 KiB"));
        }
        Ok(changed.then(|| self.text.clone()))
    }
    fn finish(self) -> Result<String, ProviderError> {
        if !self.done || !self.stopped || self.text.trim().is_empty() {
            Err(bad(
                "Model stream ended before a complete answer; send a new message to try again",
            ))
        } else {
            Ok(self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(text: &str) -> String {
        format!(
            "data: {}\r\n\r\n",
            json!({"choices":[{"delta":{"content":text},"finish_reason":null}]})
        )
    }
    #[test]
    fn arbitrary_byte_boundaries_preserve_unicode_and_require_terminal_markers() {
        let raw = format!(
            ": heartbeat\n\n{}{}data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n",
            event("{\"type\":\"final\",\"answer\":\""),
            event("文🙂\"}")
        );
        for width in [1, 2, 3, 7, 31, 4096] {
            let mut d = Decoder::default();
            for bytes in raw.as_bytes().chunks(width) {
                d.feed(bytes).unwrap();
            }
            assert_eq!(
                d.finish().unwrap(),
                "{\"type\":\"final\",\"answer\":\"文🙂\"}"
            );
        }
        let mut d = Decoder::default();
        d.feed(event("partial").as_bytes()).unwrap();
        assert!(d.finish().is_err());
        let mut d = Decoder::default();
        assert!(
            d.feed(b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n")
                .is_err()
        );
        let mut d = Decoder::default();
        assert!(d.feed(&vec![b'x'; 128 * 1024 + 1]).is_err());
    }
}
