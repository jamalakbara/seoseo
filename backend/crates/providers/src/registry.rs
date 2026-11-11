//! Builds the set of usable providers for one organization from its saved settings and the
//! operator's environment.

use crate::catalog::{CATALOG, ProviderInfo};
use crate::serp::{Brave, Searxng, Serper};
use crate::suggest::{BingSuggest, GoogleSuggest};
use crate::{ProviderError, SerpPage, SerpProvider, SerpQuery, SuggestProvider};
use std::sync::Arc;

/// Operator-level settings (environment).
#[derive(Debug, Clone, Default)]
pub struct EnvCfg {
    pub searxng_url: Option<String>,
    pub renderer_url: Option<String>,
}

/// One organization's saved setting for a provider (secret already decrypted).
#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    pub id: String,
    pub enabled: Option<bool>,
    pub secret: Option<String>,
}

/// Why a provider can or cannot be used right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    pub enabled: bool,
    /// Enabled and fully configured.
    pub ready: bool,
    /// What is missing, phrased as an instruction.
    pub missing: Option<String>,
}

pub fn availability(
    info: &ProviderInfo,
    env: &EnvCfg,
    state: Option<&ProviderState>,
) -> Availability {
    let enabled = state
        .and_then(|s| s.enabled)
        .unwrap_or(info.default_enabled);
    let missing = if info.needs_key && state.and_then(|s| s.secret.as_ref()).is_none() {
        Some("add an API key".to_string())
    } else if info.needs_env == Some("SEARXNG_URL") && env.searxng_url.is_none() {
        Some(
            "start the stack with `--profile serp` (sets SEARXNG_URL) or point SEARXNG_URL at \
             your own instance"
                .to_string(),
        )
    } else if info.needs_env == Some("RENDERER_URL") && env.renderer_url.is_none() {
        Some("start the renderer profile and set RENDERER_URL".into())
    } else {
        None
    };
    Availability {
        enabled,
        ready: enabled && missing.is_none(),
        missing,
    }
}

#[derive(Clone, Default)]
pub struct Registry {
    pub suggest: Vec<Arc<dyn SuggestProvider>>,
    /// In preference order: exact Google results first, then other indexes.
    pub serp: Vec<Arc<dyn SerpProvider>>,
    pub wikipedia: bool,
}

impl Registry {
    pub fn build(env: &EnvCfg, states: &[ProviderState]) -> Self {
        let state = |id: &str| states.iter().find(|s| s.id == id);
        let ready = |id: &str| {
            CATALOG
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|info| availability(info, env, state(id)).ready)
        };
        let secret = |id: &str| state(id).and_then(|s| s.secret.clone());

        let mut reg = Registry::default();
        if ready("suggest_google") {
            reg.suggest.push(Arc::new(GoogleSuggest::default()));
        }
        if ready("suggest_bing") {
            reg.suggest.push(Arc::new(BingSuggest::default()));
        }
        if ready("serp_serper")
            && let Some(k) = secret("serp_serper")
        {
            reg.serp.push(Arc::new(Serper::new(k)));
        }
        if ready("serp_brave")
            && let Some(k) = secret("serp_brave")
        {
            reg.serp.push(Arc::new(Brave::new(k)));
        }
        if ready("serp_searxng")
            && let Some(u) = &env.searxng_url
        {
            reg.serp.push(Arc::new(Searxng::new(u)));
        }
        if ready("serp_chromium")
            && let Some(u) = &env.renderer_url
        {
            reg.serp.push(Arc::new(crate::chromium::Chromium::new(u)));
        }
        reg.wikipedia = ready("wikipedia_pageviews");
        reg
    }

    pub fn has_serp(&self) -> bool {
        !self.serp.is_empty()
    }

    /// Try each SERP provider in order; return the first success, or the last error.
    pub async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError> {
        let mut last = ProviderError::NotConfigured(
            "no search results provider is set up; open Settings → Data providers".into(),
        );
        for p in &self.serp {
            match p.search(q).await {
                Ok(page) => return Ok(page),
                Err(e) => {
                    tracing::warn!(provider = p.id(), error = %e, "serp provider failed");
                    last = e;
                }
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: &str, enabled: Option<bool>, secret: Option<&str>) -> ProviderState {
        ProviderState {
            id: id.into(),
            enabled,
            secret: secret.map(String::from),
        }
    }

    #[test]
    fn zero_config_gives_free_suggest_and_no_serp() {
        let r = Registry::build(&EnvCfg::default(), &[]);
        assert_eq!(r.suggest.len(), 2);
        assert!(!r.has_serp());
        assert!(r.wikipedia);
    }

    #[test]
    fn keys_and_env_enable_serp_in_preference_order() {
        let env = EnvCfg {
            searxng_url: Some("http://searxng:8080".into()),
            ..Default::default()
        };
        let r = Registry::build(
            &env,
            &[
                st("serp_serper", None, Some("k1")),
                st("serp_brave", Some(false), Some("k2")),
                st("suggest_bing", Some(false), None),
            ],
        );
        let ids: Vec<_> = r.serp.iter().map(|p| p.id()).collect();
        assert_eq!(ids, vec!["serp_serper", "serp_searxng"]);
        assert_eq!(r.suggest.len(), 1);
    }

    #[test]
    fn availability_explains_what_is_missing() {
        let info = crate::catalog::info("serp_brave").unwrap();
        let a = availability(info, &EnvCfg::default(), None);
        assert!(a.enabled && !a.ready);
        assert_eq!(a.missing.as_deref(), Some("add an API key"));
        let info = crate::catalog::info("serp_searxng").unwrap();
        assert!(
            availability(info, &EnvCfg::default(), None)
                .missing
                .is_some()
        );
    }
}
