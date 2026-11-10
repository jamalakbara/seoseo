//! XML sitemaps: urlset + sitemapindex, gzip, nested up to a depth/document/byte budget.

use crate::fetch::Fetcher;
use crate::urls::{self, Scope};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashSet;
use std::io::Read;
use url::Url;

pub const MAX_SITEMAP_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_SITEMAP_DEPTH: usize = 3;
pub const MAX_SITEMAP_DOCS: usize = 300;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub urls: Vec<String>,
    pub sitemaps: Vec<String>,
}

/// Parse one sitemap document (either kind). Unknown elements are ignored.
pub fn parse(xml: &[u8]) -> Parsed {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut out = Parsed::default();
    let mut stack: Vec<String> = Vec::new();
    let mut loc = String::new();
    let mut buf = Vec::new();
    let in_loc = |stack: &[String]| stack.last().map(String::as_str) == Some("loc");
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                stack.push(e.local_name().as_ref().to_string());
                loc.clear();
            }
            Ok(Event::End(_)) => {
                if in_loc(&stack) {
                    let parent = stack.iter().rev().nth(1).map(String::as_str);
                    let value = crate::extract::decode_entities(loc.trim());
                    match parent {
                        Some("url") => out.urls.push(value),
                        Some("sitemap") => out.sitemaps.push(value),
                        _ => {}
                    }
                    loc.clear();
                }
                stack.pop();
            }
            // quick-xml reports entity references (`&amp;`) as separate events.
            Ok(Event::Text(t)) if in_loc(&stack) => loc.push_str(&t.xml10_content()),
            Ok(Event::CData(t)) if in_loc(&stack) => loc.push_str(&t.xml10_content()),
            Ok(Event::GeneralRef(r)) if in_loc(&stack) => {
                loc.push('&');
                loc.push_str(&r.xml10_content());
                loc.push(';');
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn maybe_gunzip(bytes: Vec<u8>) -> Vec<u8> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        let mut dec = flate2::read::GzDecoder::new(bytes.as_slice()).take(MAX_SITEMAP_BYTES as u64);
        if dec.read_to_end(&mut out).is_ok() {
            return out;
        }
    }
    bytes
}

#[derive(Debug, Default)]
pub struct Discovery {
    /// In-scope page URLs, normalized and de-duplicated, in discovery order.
    pub urls: Vec<Url>,
    pub documents: usize,
    pub errors: Vec<String>,
}

/// Walk sitemaps breadth-first from `roots`, keeping in-scope URLs up to `limit`.
pub async fn discover(
    fetcher: &Fetcher,
    roots: Vec<Url>,
    scope: &Scope,
    limit: usize,
) -> Discovery {
    let mut d = Discovery::default();
    let mut seen_docs = HashSet::new();
    let mut seen_urls = HashSet::new();
    let mut level: Vec<Url> = roots;
    for _depth in 0..MAX_SITEMAP_DEPTH {
        let mut next = Vec::new();
        for doc in level {
            if d.documents >= MAX_SITEMAP_DOCS || d.urls.len() >= limit {
                return d;
            }
            if !seen_docs.insert(urls::hash(&doc)) {
                continue;
            }
            d.documents += 1;
            let bytes = match fetcher.get_bytes(&doc, MAX_SITEMAP_BYTES).await {
                Ok((status, b)) if (200..300).contains(&status) => maybe_gunzip(b),
                Ok((status, _)) => {
                    d.errors.push(format!("{doc} returned HTTP {status}"));
                    continue;
                }
                Err(e) => {
                    d.errors.push(format!("{doc}: {e}"));
                    continue;
                }
            };
            let parsed = parse(&bytes);
            for u in parsed.urls {
                let Some(u) = Url::parse(&u).ok().and_then(|u| urls::normalize(&u)) else {
                    continue;
                };
                if scope.contains(&u) && seen_urls.insert(urls::hash(&u)) {
                    d.urls.push(u);
                    if d.urls.len() >= limit {
                        return d;
                    }
                }
            }
            next.extend(parsed.sitemaps.iter().filter_map(|s| Url::parse(s).ok()));
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urlset_and_index() {
        let xml = br#"<?xml version="1.0"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://ex.com/a?x=1&amp;y=2</loc><lastmod>2026-01-01</lastmod></url>
  <url><loc><![CDATA[https://ex.com/b]]></loc></url>
</urlset>"#;
        let p = parse(xml);
        assert_eq!(p.urls, vec!["https://ex.com/a?x=1&y=2", "https://ex.com/b"]);
        let idx =
            br#"<sitemapindex><sitemap><loc>https://ex.com/s1.xml</loc></sitemap></sitemapindex>"#;
        assert_eq!(parse(idx).sitemaps, vec!["https://ex.com/s1.xml"]);
    }
}
