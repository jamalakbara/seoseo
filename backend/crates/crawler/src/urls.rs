//! URL normalization and crawl scope.

use sha2::{Digest, Sha256};
use url::Url;

/// Canonical form used for de-duplication: lowercase host, no fragment, default port removed,
/// empty path becomes `/`. Query strings are kept (they can be distinct pages).
pub fn normalize(url: &Url) -> Option<Url> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    let mut u = url.clone();
    u.set_fragment(None);
    if u.port() == u.port_or_known_default() && u.port().is_some() {
        let _ = u.set_port(None);
    }
    if u.query() == Some("") {
        u.set_query(None);
    }
    if u.path().is_empty() {
        u.set_path("/");
    }
    Some(u)
}

/// Resolve `href` against `base` and normalize. Ignores `mailto:`, `javascript:`, `tel:` etc.
pub fn resolve(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    base.join(href).ok().and_then(|u| normalize(&u))
}

/// Stable short hash of a normalized URL (frontier / page key).
pub fn hash(url: &Url) -> String {
    let digest = Sha256::digest(url.as_str().as_bytes());
    hex::encode(&digest[..16])
}

/// Hosts that belong to the audited site: the start host and its `www.` twin.
#[derive(Debug, Clone)]
pub struct Scope {
    bare: String,
}

impl Scope {
    pub fn new(start: &Url) -> Self {
        let host = start.host_str().unwrap_or_default().to_ascii_lowercase();
        Self {
            bare: host.strip_prefix("www.").unwrap_or(&host).to_string(),
        }
    }

    pub fn contains(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        host == self.bare || host.strip_prefix("www.") == Some(self.bare.as_str())
    }
}

/// Paths that are almost never HTML pages; we skip fetching them during the crawl.
pub fn looks_like_asset(url: &Url) -> bool {
    const EXT: &[&str] = &[
        ".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif", ".svg", ".ico", ".css", ".js", ".mjs",
        ".map", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".mp4", ".webm", ".mp3", ".wav", ".zip",
        ".gz", ".tar", ".rar", ".7z", ".exe", ".dmg", ".pdf", ".doc", ".docx", ".xls", ".xlsx",
        ".ppt", ".pptx", ".json", ".xml", ".txt", ".csv",
    ];
    let path = url.path().to_ascii_lowercase();
    EXT.iter().any(|e| path.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        let u = Url::parse("HTTPS://Example.COM:443/a?#frag").unwrap();
        assert_eq!(normalize(&u).unwrap().as_str(), "https://example.com/a");
        let u = Url::parse("http://example.com").unwrap();
        assert_eq!(normalize(&u).unwrap().as_str(), "http://example.com/");
        assert!(normalize(&Url::parse("mailto:a@b.c").unwrap()).is_none());
    }

    #[test]
    fn resolves_and_scopes() {
        let base = Url::parse("https://example.com/blog/post").unwrap();
        assert_eq!(
            resolve(&base, "../about#x").unwrap().as_str(),
            "https://example.com/about"
        );
        assert!(resolve(&base, "javascript:void(0)").is_none());
        assert!(resolve(&base, "#top").is_none());
        let scope = Scope::new(&Url::parse("https://www.example.com/").unwrap());
        assert!(scope.contains(&Url::parse("https://example.com/x").unwrap()));
        assert!(scope.contains(&Url::parse("http://www.example.com/x").unwrap()));
        assert!(!scope.contains(&Url::parse("https://blog.example.com/").unwrap()));
        assert!(!scope.contains(&Url::parse("https://example.com.evil.io/").unwrap()));
    }
}
