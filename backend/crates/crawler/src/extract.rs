//! Streaming HTML extraction with lol_html: one pass, bounded memory, no DOM.

use lol_html::html_content::{Element, TextChunk};
use lol_html::{HtmlRewriter, Settings, doc_text, element};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::rc::Rc;

pub const MAX_LINKS: usize = 1000;
pub const MAX_IMAGES: usize = 1000;
const MAX_CAPTURE: usize = 300;
const MAX_JSON_LD: usize = 100 * 1024;
const MAX_VISIBLE_TEXT: usize = 512 * 1024;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Link {
    pub href: String,
    pub text: String,
    pub nofollow: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Image {
    pub src: String,
    /// `alt` attribute present (an empty alt is valid for decorative images).
    pub has_alt: bool,
}

/// Everything the issue rules need from one HTML document.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PageFacts {
    pub title: Option<String>,
    pub title_count: u32,
    pub meta_description: Option<String>,
    pub meta_robots: Option<String>,
    pub canonical: Option<String>,
    pub lang: Option<String>,
    pub has_viewport: bool,
    pub base_href: Option<String>,
    pub meta_refresh: Option<String>,
    /// Heading levels (1–6) in document order.
    pub headings: Vec<u8>,
    pub h1s: Vec<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub hreflang: Vec<(String, String)>,
    pub json_ld: Vec<String>,
    /// http:// resources referenced by scripts, images, iframes and stylesheets.
    pub insecure_resources: Vec<String>,
    pub script_count: u32,
    /// An empty SPA mount point such as `<div id="root">` / `#app` / `#__next`.
    pub has_app_root: bool,
    pub word_count: u32,
    /// Hash of normalized visible text, for duplicate-content detection.
    pub text_hash: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Cap {
    Skip,
    Title,
    Heading(usize),
    Anchor(usize),
    JsonLd(usize),
}

#[derive(Default)]
struct State {
    facts: PageFacts,
    stack: Vec<Cap>,
    visible: String,
    anchor_text: Vec<String>,
    heading_text: Vec<String>,
    title_buf: String,
}

fn push_capped(buf: &mut String, s: &str, max: usize) {
    if buf.len() < max {
        let room = max - buf.len();
        let mut end = s.len().min(room);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        buf.push_str(&s[..end]);
    }
}

fn attr(el: &Element, name: &str) -> Option<String> {
    el.get_attribute(name).map(|v| decode_entities(v.trim()))
}

/// Track an element until its end tag; void or unclosed elements simply never pop.
fn track(el: &mut Element, state: &Rc<RefCell<State>>, cap: Cap) {
    if let Some(handlers) = el.end_tag_handlers() {
        state.borrow_mut().stack.push(cap);
        let st = state.clone();
        handlers.push(Box::new(move |_end| {
            let mut s = st.borrow_mut();
            if let Some(pos) = s.stack.iter().rposition(|c| *c == cap) {
                s.stack.truncate(pos);
            }
            Ok(())
        }));
    }
}

pub fn extract(html: &[u8]) -> PageFacts {
    let state = Rc::new(RefCell::new(State::default()));
    let s = |st: &Rc<RefCell<State>>| st.clone();

    let st_html = s(&state);
    let st_title = s(&state);
    let st_meta = s(&state);
    let st_link = s(&state);
    let st_base = s(&state);
    let st_h = s(&state);
    let st_a = s(&state);
    let st_img = s(&state);
    let st_script = s(&state);
    let st_skip = s(&state);
    let st_res = s(&state);
    let st_root = s(&state);
    let st_text = s(&state);

    let settings = Settings::new()
        .append_element_content_handler(element!("html", move |el| {
            st_html.borrow_mut().facts.lang = attr(el, "lang").filter(|l| !l.is_empty());
            Ok(())
        }))
        .append_element_content_handler(element!("title", move |el| {
            {
                let mut s = st_title.borrow_mut();
                s.facts.title_count += 1;
            }
            // Only the first <title> counts, like browsers.
            let first = st_title.borrow().facts.title_count == 1;
            track(el, &st_title, if first { Cap::Title } else { Cap::Skip });
            Ok(())
        }))
        .append_element_content_handler(element!("meta", move |el| {
            let mut s = st_meta.borrow_mut();
            let name = attr(el, "name").unwrap_or_default().to_ascii_lowercase();
            let property = attr(el, "property")
                .unwrap_or_default()
                .to_ascii_lowercase();
            let http_equiv = attr(el, "http-equiv")
                .unwrap_or_default()
                .to_ascii_lowercase();
            let content = attr(el, "content");
            match (name.as_str(), property.as_str()) {
                ("description", _) if s.facts.meta_description.is_none() => {
                    s.facts.meta_description = content
                }
                ("robots", _) => {
                    let merged = [s.facts.meta_robots.take(), content]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    s.facts.meta_robots = (!merged.is_empty()).then(|| merged.join(", "));
                }
                ("seoseobot", _) => {
                    let merged = [s.facts.meta_robots.take(), content]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    s.facts.meta_robots = (!merged.is_empty()).then(|| merged.join(", "));
                }
                ("viewport", _) => s.facts.has_viewport = true,
                (_, "og:title") => s.facts.og_title = content,
                (_, "og:description") => s.facts.og_description = content,
                (_, "og:image") => s.facts.og_image = content,
                _ if http_equiv == "refresh" => s.facts.meta_refresh = content,
                _ => {}
            }
            Ok(())
        }))
        .append_element_content_handler(element!("link[rel][href]", move |el| {
            let mut s = st_link.borrow_mut();
            let rel = attr(el, "rel").unwrap_or_default().to_ascii_lowercase();
            let href = attr(el, "href").unwrap_or_default();
            let rels: Vec<&str> = rel.split_ascii_whitespace().collect();
            if rels.contains(&"canonical") && s.facts.canonical.is_none() {
                s.facts.canonical = Some(href.clone());
            }
            if rels.contains(&"alternate")
                && let Some(lang) = attr(el, "hreflang")
            {
                s.facts.hreflang.push((lang, href.clone()));
            }
            if rels.contains(&"stylesheet") && href.starts_with("http://") {
                s.facts.insecure_resources.push(href);
            }
            Ok(())
        }))
        .append_element_content_handler(element!("base[href]", move |el| {
            let mut s = st_base.borrow_mut();
            if s.facts.base_href.is_none() {
                s.facts.base_href = attr(el, "href");
            }
            Ok(())
        }))
        .append_element_content_handler(element!("h1, h2, h3, h4, h5, h6", move |el| {
            let level = el.tag_name().as_bytes()[1] - b'0';
            let idx = {
                let mut s = st_h.borrow_mut();
                s.facts.headings.push(level);
                if level == 1 {
                    s.heading_text.push(String::new());
                    Some(s.heading_text.len() - 1)
                } else {
                    None
                }
            };
            if let Some(i) = idx {
                track(el, &st_h, Cap::Heading(i));
            }
            Ok(())
        }))
        .append_element_content_handler(element!("a[href]", move |el| {
            let idx = {
                let mut s = st_a.borrow_mut();
                if s.facts.links.len() >= MAX_LINKS {
                    return Ok(());
                }
                let rel = attr(el, "rel").unwrap_or_default().to_ascii_lowercase();
                s.facts.links.push(Link {
                    href: attr(el, "href").unwrap_or_default(),
                    text: String::new(),
                    nofollow: rel.split_ascii_whitespace().any(|r| r == "nofollow"),
                });
                s.anchor_text.push(String::new());
                s.facts.links.len() - 1
            };
            track(el, &st_a, Cap::Anchor(idx));
            Ok(())
        }))
        .append_element_content_handler(element!("img", move |el| {
            let mut s = st_img.borrow_mut();
            let src = attr(el, "src")
                .or_else(|| attr(el, "data-src"))
                .unwrap_or_default();
            if src.starts_with("http://") {
                s.facts.insecure_resources.push(src.clone());
            }
            if s.facts.images.len() < MAX_IMAGES {
                s.facts.images.push(Image {
                    src,
                    has_alt: el.has_attribute("alt"),
                });
            }
            Ok(())
        }))
        .append_element_content_handler(element!("script", move |el| {
            let ty = attr(el, "type").unwrap_or_default().to_ascii_lowercase();
            let src = attr(el, "src");
            let cap = {
                let mut s = st_script.borrow_mut();
                s.facts.script_count += 1;
                if let Some(src) = src.filter(|s| s.starts_with("http://")) {
                    s.facts.insecure_resources.push(src);
                }
                if ty == "application/ld+json" {
                    s.facts.json_ld.push(String::new());
                    Cap::JsonLd(s.facts.json_ld.len() - 1)
                } else {
                    Cap::Skip
                }
            };
            track(el, &st_script, cap);
            Ok(())
        }))
        .append_element_content_handler(element!(
            "style, noscript, template, svg, select, option",
            move |el| {
                track(el, &st_skip, Cap::Skip);
                Ok(())
            }
        ))
        .append_element_content_handler(element!(
            "iframe[src], source[src], video[src], audio[src]",
            move |el| {
                if let Some(src) = attr(el, "src").filter(|s| s.starts_with("http://")) {
                    st_res.borrow_mut().facts.insecure_resources.push(src);
                }
                Ok(())
            }
        ))
        .append_element_content_handler(element!(
            "#root, #app, #__next, #__nuxt, #___gatsby, [data-reactroot]",
            move |_el| {
                st_root.borrow_mut().facts.has_app_root = true;
                Ok(())
            }
        ))
        .append_document_content_handler(doc_text!(move |t: &mut TextChunk| {
            let mut s = st_text.borrow_mut();
            let text = t.as_str();
            if text.is_empty() {
                return Ok(());
            }
            let stack = s.stack.clone();
            let mut visible = true;
            for cap in &stack {
                match *cap {
                    Cap::Skip => visible = false,
                    Cap::Title => {
                        visible = false;
                        push_capped(&mut s.title_buf, text, MAX_CAPTURE * 4);
                    }
                    Cap::JsonLd(i) => {
                        visible = false;
                        if let Some(buf) = s.facts.json_ld.get_mut(i) {
                            push_capped(buf, text, MAX_JSON_LD);
                        }
                    }
                    Cap::Heading(i) => {
                        if let Some(buf) = s.heading_text.get_mut(i) {
                            push_capped(buf, text, MAX_CAPTURE);
                        }
                    }
                    Cap::Anchor(i) => {
                        if let Some(buf) = s.anchor_text.get_mut(i) {
                            push_capped(buf, text, MAX_CAPTURE);
                        }
                    }
                }
            }
            if visible {
                push_capped(&mut s.visible, text, MAX_VISIBLE_TEXT);
                s.visible.push(' ');
            }
            Ok(())
        }));

    let mut rewriter = HtmlRewriter::new(settings, |_: &[u8]| {});
    // Malformed markup is not fatal: keep whatever was extracted so far.
    let _ = rewriter.write(html);
    let _ = rewriter.end();

    let mut s = Rc::try_unwrap(state)
        .map(RefCell::into_inner)
        .unwrap_or_else(|rc| std::mem::take(&mut *rc.borrow_mut()));

    let title = collapse(&decode_entities(&s.title_buf));
    s.facts.title = (s.facts.title_count > 0).then_some(title);
    s.facts.h1s = s
        .heading_text
        .iter()
        .map(|h| collapse(&decode_entities(h)))
        .collect();
    for (link, text) in s.facts.links.iter_mut().zip(s.anchor_text.iter()) {
        link.text = collapse(&decode_entities(text));
    }
    let visible = collapse(&decode_entities(&s.visible));
    s.facts.word_count = visible
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count() as u32;
    if !visible.is_empty() {
        let digest = Sha256::digest(visible.to_lowercase().as_bytes());
        s.facts.text_hash = Some(hex::encode(&digest[..16]));
    }
    s.facts
}

/// Collapse runs of whitespace and trim.
pub fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decode the HTML entities that matter for titles, descriptions and URLs.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest[1..].find(';').map(|i| i + 1).filter(|&i| i <= 10);
        let decoded = end.and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "hellip" => Some('…'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<!doctype html><html lang="en"><head>
<title>Hello &amp; Welcome</title>
<meta name="description" content="A page about &quot;things&quot;">
<meta name="robots" content="noindex, follow">
<meta name="viewport" content="width=device-width">
<link rel="canonical" href="https://ex.com/a?x=1&amp;y=2">
<link rel="alternate" hreflang="de" href="https://ex.com/de/">
<meta property="og:title" content="OG">
<script type="application/ld+json">{"@type":"Organization"}</script>
<script>var hidden = "not words";</script>
<style>.x{}</style>
</head><body>
<h1>Main <em>Heading</em></h1><h3>Skipped level</h3>
<p>Some visible body text here.</p>
<a href="/about" rel="nofollow">About <b>us</b></a>
<img src="/a.png"><img src="http://cdn.ex.com/b.png" alt="">
<div id="root"></div>
</body></html>"#;

    #[test]
    fn extracts_facts() {
        let f = extract(PAGE.as_bytes());
        assert_eq!(f.title.as_deref(), Some("Hello & Welcome"));
        assert_eq!(
            f.meta_description.as_deref(),
            Some(r#"A page about "things""#)
        );
        assert_eq!(f.meta_robots.as_deref(), Some("noindex, follow"));
        assert_eq!(f.canonical.as_deref(), Some("https://ex.com/a?x=1&y=2"));
        assert_eq!(f.lang.as_deref(), Some("en"));
        assert!(f.has_viewport);
        assert_eq!(f.headings, vec![1, 3]);
        assert_eq!(f.h1s, vec!["Main Heading"]);
        assert_eq!(f.links.len(), 1);
        assert_eq!(f.links[0].text, "About us");
        assert!(f.links[0].nofollow);
        assert_eq!(f.images.len(), 2);
        assert!(!f.images[0].has_alt && f.images[1].has_alt);
        assert_eq!(f.insecure_resources, vec!["http://cdn.ex.com/b.png"]);
        assert_eq!(f.json_ld, vec![r#"{"@type":"Organization"}"#]);
        assert_eq!(f.hreflang, vec![("de".into(), "https://ex.com/de/".into())]);
        assert!(f.has_app_root);
        // Main Heading Skipped level Some visible body text here. About us
        assert_eq!(f.word_count, 11);
        assert!(f.text_hash.is_some());
    }

    #[test]
    fn missing_title_is_none() {
        let f = extract(b"<html><body><p>x</p></body></html>");
        assert!(f.title.is_none());
        assert_eq!(f.title_count, 0);
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            decode_entities("a &amp; b &#39;c&#x27; &bogus; &"),
            "a & b 'c' &bogus; &"
        );
    }
}
