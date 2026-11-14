//! The list of data providers the product knows about. Everything needed works without a
//! paid key; bring-your-own-key entries only upgrade quality.

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Explicit local-grid and Google AI Overview collection, separate from organic search.
    Observations,
    /// Keyword ideas from autocomplete.
    Suggest,
    /// Search result pages.
    Serp,
    /// Demand signals (pageviews).
    Demand,
    /// Domain authority.
    Authority,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub kind: ProviderKind,
    pub name: &'static str,
    pub description: &'static str,
    /// Usable without paying anyone.
    pub free: bool,
    /// Needs an API key from the user.
    pub needs_key: bool,
    /// Where to get a key.
    pub key_url: Option<&'static str>,
    /// Needs something from the server operator (environment variable).
    pub needs_env: Option<&'static str>,
    pub default_enabled: bool,
    /// Honest note about data quality or terms.
    pub note: Option<&'static str>,
}

pub const CATALOG: &[ProviderInfo] = &[
    ProviderInfo {
        id: "serpapi_observations",
        kind: ProviderKind::Observations,
        name: "SerpApi (Maps and AI Overviews)",
        description: "Optional explicit Google Maps grid scans and Google AI Overview capture.",
        free: false,
        needs_key: true,
        key_url: Some("https://serpapi.com/"),
        needs_env: None,
        default_enabled: false,
        note: Some(
            "BYOK. Enable explicitly. Grid scans show a reviewed request budget; imported observations work without a key. Never used as an organic search fallback.",
        ),
    },
    ProviderInfo {
        id: "suggest_google",
        kind: ProviderKind::Suggest,
        name: "Google Autocomplete",
        description: "Keyword ideas from Google's public suggestion endpoint.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: None,
        default_enabled: true,
        note: Some("Unofficial public endpoint; requests are paced and cached for 30 days."),
    },
    ProviderInfo {
        id: "suggest_bing",
        kind: ProviderKind::Suggest,
        name: "Bing Autocomplete",
        description: "Keyword ideas from Bing's OpenSearch suggestion endpoint.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: None,
        default_enabled: true,
        note: None,
    },
    ProviderInfo {
        id: "wikipedia_pageviews",
        kind: ProviderKind::Demand,
        name: "Wikipedia pageviews",
        description: "Monthly article views for keywords that match a Wikipedia article; \
                      used as a measured demand signal and 12-month trend.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: None,
        default_enabled: true,
        note: Some("Article views are not search volume; estimates built on them are labelled."),
    },
    ProviderInfo {
        id: "serp_searxng",
        kind: ProviderKind::Serp,
        name: "SearXNG (self-hosted)",
        description: "Search results from your own SearXNG meta-search container.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: Some("SEARXNG_URL"),
        default_enabled: true,
        note: Some(
            "Positions are a blend of several engines, not exact Google rankings. \
             Start it with the `serp` compose profile.",
        ),
    },
    ProviderInfo {
        id: "serp_brave",
        kind: ProviderKind::Serp,
        name: "Brave Search API",
        description: "Search results from Brave's independent index (free plan available).",
        free: false,
        needs_key: true,
        key_url: Some("https://api-dashboard.search.brave.com/"),
        needs_env: None,
        default_enabled: true,
        note: Some("Brave's own index; positions differ from Google."),
    },
    ProviderInfo {
        id: "serp_serper",
        kind: ProviderKind::Serp,
        name: "Serper (Google results API)",
        description: "Exact Google results, People Also Ask and related searches.",
        free: false,
        needs_key: true,
        key_url: Some("https://serper.dev/"),
        needs_env: None,
        default_enabled: true,
        note: Some("Paid API with a free trial. Preferred over other SERP sources when set."),
    },
    ProviderInfo {
        id: "serp_chromium",
        kind: ProviderKind::Serp,
        name: "Chromium (Bing, opt-in)",
        description: "Direct Bing organic results using the isolated browser service.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: Some("RENDERER_URL"),
        default_enabled: false,
        note: Some(
            "Opt-in scraping uses your server IP, honors robots.txt and stops on blocks or CAPTCHA. Bing positions are not Google positions. At most one search per 10 seconds; blocked searches pause for an hour.",
        ),
    },
    ProviderInfo {
        id: "authority_commoncrawl",
        kind: ProviderKind::Authority,
        name: "Common Crawl web graph",
        description: "SeoSeo Rank (SR): domain authority computed from Common Crawl's public \
                      domain-level link graph. Powers keyword difficulty.",
        free: true,
        needs_key: false,
        key_url: None,
        needs_env: None,
        default_enabled: true,
        note: Some("Import the top domains once per release (about quarterly)."),
    },
];

pub fn info(id: &str) -> Option<&'static ProviderInfo> {
    CATALOG.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_kind_has_a_free_default() {
        let mut ids: Vec<_> = CATALOG.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        for kind in [
            ProviderKind::Suggest,
            ProviderKind::Serp,
            ProviderKind::Demand,
            ProviderKind::Authority,
        ] {
            assert!(
                CATALOG
                    .iter()
                    .any(|p| p.kind == kind && p.free && p.default_enabled),
                "{kind:?} needs a free default provider"
            );
        }
    }
}
