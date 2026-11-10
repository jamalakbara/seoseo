//! Site-audit crawler building blocks. Pure, testable pieces with no database access:
//! URL policy (SSRF guard), fetcher, robots.txt, sitemaps, HTML extraction and issue rules.
//! The `jobs` crate orchestrates them into a durable, resumable audit.

pub mod extract;
pub mod fetch;
pub mod policy;
pub mod robots;
pub mod rules;
pub mod sitemap;
pub mod urls;

pub use extract::PageFacts;
pub use fetch::{FetchClass, FetchOutcome, Fetcher, FetcherConfig};
pub use policy::UrlPolicy;

/// Default crawler user agent. Sites can allow it by name in robots.txt / WAF rules.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; SeoSeoBot/0.1; +https://github.com/seoseo/seoseo)";
/// Token matched against robots.txt groups.
pub const ROBOTS_TOKEN: &str = "SeoSeoBot";
