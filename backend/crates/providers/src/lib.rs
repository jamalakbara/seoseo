//! Data providers: thin adapters around free public data sources and optional
//! bring-your-own-key APIs. No database access here; services and jobs call these.
//!
//! Rules every adapter follows:
//! - honest User-Agent, fixed hosts (no user-supplied URLs), polite pacing;
//! - a block or CAPTCHA is reported as [`ProviderError::Blocked`] and never worked around;
//! - unknown data stays unknown (`None`), never a made-up number.

pub mod catalog;
pub mod commoncrawl;
mod http;
pub mod rdap;
pub mod registry;
pub mod serp;
pub mod suggest;
pub mod wikipedia;

pub use registry::{EnvCfg, ProviderState, Registry};
pub use serp::{SerpItem, SerpPage, SerpProvider, SerpQuery};
pub use suggest::SuggestProvider;

/// Identifies us to the services we call. Not a browser string on purpose.
pub const USER_AGENT: &str = "SeoSeo/0.1 (+https://github.com/seoseo/seoseo; self-hosted SEO tool)";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("rate limited by the provider; try again later")]
    RateLimited,
    /// Access denied, CAPTCHA or bot check. We stop instead of working around it.
    #[error("the provider refused the request: {0}")]
    Blocked(String),
    #[error("the provider is unreachable: {0}")]
    Unavailable(String),
    #[error("unexpected response from the provider: {0}")]
    BadResponse(String),
    #[error("{0}")]
    NotConfigured(String),
}

/// A search market: ISO 3166-1 alpha-2 country + ISO 639 language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Market {
    pub country: String,
    pub language: String,
}

impl Market {
    pub fn new(country: &str, language: &str) -> Self {
        Self {
            country: country.to_ascii_uppercase(),
            language: language.to_ascii_lowercase(),
        }
    }

    /// Primary language subtag (`pt-br` → `pt`).
    pub fn lang(&self) -> &str {
        self.language.split('-').next().unwrap_or("en")
    }

    /// `en-US` style locale.
    pub fn locale(&self) -> String {
        format!("{}-{}", self.lang(), self.country)
    }
}

pub mod chromium;
