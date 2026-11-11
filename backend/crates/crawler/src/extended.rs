//! Conservative extended checks. Uncrawled alternates are unknown, not broken.
use crate::rules::{Finding, PageInput};
use serde_json::json;
use std::collections::HashSet;
use url::Url;

fn language(code: &str) -> bool {
    if code.eq_ignore_ascii_case("x-default") {
        return true;
    }
    let parts: Vec<_> = code.split('-').collect();
    (1..=2).contains(&parts.len())
        && (2..=3).contains(&parts[0].len())
        && parts[0].bytes().all(|c| c.is_ascii_alphabetic())
        && (parts.len() == 1
            || (parts[1].len() == 2 && parts[1].bytes().all(|c| c.is_ascii_alphabetic())))
}

pub fn page_rules(p: &PageInput) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(f) = p.facts else { return out };
    if let Ok(url) = Url::parse(p.url) {
        let path = url.path();
        if p.url.len() > 200
            || path.contains('_')
            || path.bytes().any(|c| c.is_ascii_uppercase())
            || url.query_pairs().count() > 4
        {
            out.push(Finding::new("url-hygiene", json!({"url": p.url})));
        }
    }
    let error_heading = f.title.iter().chain(f.h1s.iter()).any(|s| {
        let s = s.trim().to_lowercase();
        s == "404"
            || s.starts_with("404 ")
            || s.starts_with("404:")
            || s.contains("page not found")
            || s.contains("page doesn't exist")
    });
    if p.status == Some(200) && f.word_count < 150 && error_heading {
        out.push(Finding::new(
            "soft-404",
            json!({"title": f.title, "h1": f.h1s, "confidence": "heuristic"}),
        ));
    }
    if !f.hreflang.is_empty() {
        let mut codes = HashSet::new();
        let mut self_ref = false;
        for (code, target) in &f.hreflang {
            let parsed = Url::parse(target).ok().filter(|u| {
                matches!(u.scheme(), "http" | "https")
                    && u.username().is_empty()
                    && u.password().is_none()
            });
            if !language(code) || parsed.is_none() || !codes.insert(code.to_lowercase()) {
                out.push(Finding::new(
                    "invalid-hreflang",
                    json!({"language": code, "target": target}),
                ));
            }
            self_ref |= target.trim_end_matches('/') == p.url.trim_end_matches('/');
        }
        if !self_ref {
            out.push(Finding::new("hreflang-missing-self", json!({})));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hreflang_syntax() {
        for s in ["en", "id-ID", "eng", "x-default"] {
            assert!(language(s), "{s}");
        }
        for s in ["", "english", "en_US", "en-123", "1d"] {
            assert!(!language(s), "{s}");
        }
    }
}
