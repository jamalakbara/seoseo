//! Public markup fingerprints. They describe observed hints, not a verified stack.
use lol_html::{HtmlRewriter, Settings, element};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DetectedTechnology {
    pub name: String,
    pub category: String,
    pub signals: Vec<String>,
}

pub fn detect(bytes: &[u8]) -> Vec<DetectedTechnology> {
    let found = Rc::new(RefCell::new(BTreeMap::<String, DetectedTechnology>::new()));
    let add = |map: &mut BTreeMap<String, DetectedTechnology>,
               name: &str,
               category: &str,
               signal: &str| {
        let entry = map
            .entry(name.into())
            .or_insert_with(|| DetectedTechnology {
                name: name.into(),
                category: category.into(),
                signals: vec![],
            });
        if entry.signals.len() < 4 && !entry.signals.iter().any(|s| s == signal) {
            entry.signals.push(signal.into());
        }
    };
    let generators = found.clone();
    let assets = found.clone();
    let mut parser = HtmlRewriter::new(
        Settings::new()
            .append_element_content_handler(element!("meta[name]", move |el| {
                if el
                    .get_attribute("name")
                    .is_some_and(|s| s.eq_ignore_ascii_case("generator"))
                {
                    let generator = el
                        .get_attribute("content")
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase();
                    for (prefix, name, category) in [
                        ("wordpress", "WordPress", "CMS"),
                        ("shopify", "Shopify", "Commerce"),
                        ("nuxt", "Nuxt", "Framework"),
                        ("next.js", "Next.js", "Framework"),
                    ] {
                        if generator
                            .strip_prefix(prefix)
                            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '/']))
                        {
                            add(
                                &mut generators.borrow_mut(),
                                name,
                                category,
                                "Declared meta generator",
                            );
                        }
                    }
                }
                Ok(())
            }))
            .append_element_content_handler(element!(
                "script[src],link[href],img[src]",
                move |el| {
                    let raw = el
                        .get_attribute("src")
                        .or_else(|| el.get_attribute("href"))
                        .unwrap_or_default();
                    let base = url::Url::parse("https://relative.invalid/").unwrap();
                    if let Ok(url) = base.join(&raw) {
                        if !matches!(url.scheme(), "http" | "https") {
                            return Ok(());
                        }
                        let path = url.path();
                        let mut map = assets.borrow_mut();
                        if path.contains("/wp-content/") {
                            add(&mut map, "WordPress", "CMS", "Resource path /wp-content/");
                        }
                        if path.contains("/_next/static/") {
                            add(
                                &mut map,
                                "Next.js",
                                "Framework",
                                "Resource path /_next/static/",
                            );
                        }
                        if path.contains("/_nuxt/") {
                            add(&mut map, "Nuxt", "Framework", "Resource path /_nuxt/");
                        }
                        if url.host_str() == Some("cdn.shopify.com") {
                            add(
                                &mut map,
                                "Shopify",
                                "Commerce",
                                "Resource host cdn.shopify.com",
                            );
                        }
                    }
                    Ok(())
                }
            )),
        |_: &[u8]| {},
    );
    if parser.write(bytes).is_err() || parser.end().is_err() {
        return vec![];
    }
    found.borrow().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspects_structured_markup_not_article_text_or_lookalike_hosts() {
        let results=detect(br#"<p>WordPress /wp-content/ and Nuxt are discussed.</p><script src="https://cdn.shopify.com.evil.example/a.js"></script>"#);
        assert!(results.is_empty());
        let results=detect(br#"<meta name="generator" content="WordPress 6"><script src="/wp-content/theme/a.js"></script><script src="/_next/static/a.js"></script><link href="/_nuxt/main.css"><img src="https://cdn.shopify.com/a.jpg">"#);
        assert_eq!(results.len(), 4);
        assert_eq!(
            results
                .iter()
                .find(|t| t.name == "WordPress")
                .unwrap()
                .signals
                .len(),
            2
        );
    }
}
