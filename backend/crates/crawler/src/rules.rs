//! Site-audit issue catalog and rules. Thresholds match OpenSEO so results are comparable.
//!
//! Page rules look at one fetched page; site rules look at the whole crawl (duplicates,
//! orphans, broken internal links). Both return [`Finding`]s keyed by issue id.

use crate::extract::PageFacts;
use crate::fetch::FetchClass;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

pub const TITLE_MIN: usize = 10;
pub const TITLE_MAX: usize = 60;
pub const META_DESC_MIN: usize = 70;
pub const META_DESC_MAX: usize = 160;
pub const THIN_CONTENT_WORDS: u32 = 150;
pub const DEEP_PAGE_DEPTH: i32 = 5;
pub const SLOW_RESPONSE_MS: u32 = 1500;
pub const DUPLICATE_SAMPLE: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IssueType {
    pub id: &'static str,
    pub severity: Severity,
    pub category: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
    pub how_to_fix: &'static str,
}

macro_rules! issue {
    ($id:literal, $sev:ident, $cat:literal, $title:literal, $why:literal, $fix:literal) => {
        IssueType {
            id: $id,
            severity: Severity::$sev,
            category: $cat,
            title: $title,
            explanation: $why,
            how_to_fix: $fix,
        }
    };
}

/// Every issue the audit can report. The API serves this list to the UI and agents.
pub static CATALOG: &[IssueType] = &[
    issue!(
        "invalid-hreflang",
        Warning,
        "international",
        "Invalid hreflang",
        "A language code or alternate URL is invalid or duplicated.",
        "Use valid language-region codes and absolute HTTP(S) URLs, one target per language."
    ),
    issue!(
        "hreflang-missing-self",
        Warning,
        "international",
        "Hreflang omits this page",
        "The alternate set does not reference this page.",
        "Include a self-referencing alternate."
    ),
    issue!(
        "hreflang-target-error",
        Warning,
        "international",
        "Hreflang target is not indexable",
        "A crawled alternate redirects or is not indexable.",
        "Point alternates to canonical, indexable pages."
    ),
    issue!(
        "hreflang-missing-return",
        Warning,
        "international",
        "Missing reciprocal hreflang",
        "A crawled alternate does not link back to this page.",
        "Include reciprocal alternate links on every language version."
    ),
    issue!(
        "soft-404",
        Warning,
        "content",
        "Possible soft 404",
        "A short successful page has a not-found title or heading. This is a heuristic.",
        "Return HTTP 404/410 for missing content or restore the page."
    ),
    issue!(
        "url-hygiene",
        Info,
        "crawlability",
        "URL can be simplified",
        "The URL is long, contains uppercase or underscores in its path, or has many query parameters.",
        "Prefer short descriptive paths; avoid changing established URLs without redirects."
    ),
    issue!(
        "broken-asset",
        Warning,
        "performance",
        "Broken page resource",
        "A checked same-origin image, stylesheet or script returned 4xx/5xx.",
        "Restore the resource or update its URL."
    ),
    issue!(
        "large-asset",
        Info,
        "performance",
        "Large page resource",
        "A checked resource exceeds 1 MiB.",
        "Compress images and split or minify scripts and styles."
    ),
    issue!(
        "render-failed",
        Warning,
        "rendering",
        "JavaScript render failed",
        "The browser could not render this page. Static HTML was audited instead.",
        "Check the renderer configuration and page errors."
    ),
    issue!(
        "lighthouse-failed",
        Warning,
        "performance",
        "Lighthouse audit failed",
        "No lab metrics were available for this page.",
        "Check renderer availability and the page response."
    ),
    issue!(
        "blocked-page",
        Critical,
        "crawlability",
        "Crawler was blocked",
        "The server answered with an access denial or bot challenge (401/403) instead of the page, so it could not be audited. Search engines may hit the same wall.",
        "Allow the SeoSeoBot user agent in your firewall or bot protection, then run the audit again."
    ),
    issue!(
        "server-error",
        Critical,
        "crawlability",
        "Server error (5xx)",
        "The page returned a 5xx error. Visitors and search engines cannot load it.",
        "Check server logs for this URL and fix the failing route or upstream."
    ),
    issue!(
        "broken-internal-link",
        Critical,
        "links",
        "Broken internal link",
        "A page on your site links to an internal URL that returns an error.",
        "Update or remove the link, or restore the target page / add a redirect."
    ),
    issue!(
        "missing-title",
        Critical,
        "content",
        "Missing title tag",
        "The page has no <title>. Titles are the main headline in search results.",
        "Add a unique, descriptive <title> of roughly 10–60 characters."
    ),
    issue!(
        "redirect-loop",
        Critical,
        "crawlability",
        "Redirect loop",
        "Following this URL's redirects leads back to a URL already visited, so it never loads.",
        "Fix the redirect rules so the URL resolves to a final 200 page."
    ),
    issue!(
        "javascript-rendering-suspected",
        Warning,
        "rendering",
        "Content may require JavaScript",
        "The HTML contains an app container with almost no readable text, headings or links. Content probably loads in the browser, and content checks may be unreliable.",
        "Serve important content and links in the initial HTML (server-side rendering or prerendering). Enable JavaScript rendering in the audit settings."
    ),
    issue!(
        "rate-limited-page",
        Warning,
        "crawlability",
        "Rate limited (429)",
        "The server asked the crawler to slow down (HTTP 429) for this page.",
        "Allow a higher request rate for SeoSeoBot, or lower the crawl speed and run the audit again."
    ),
    issue!(
        "broken-page",
        Warning,
        "crawlability",
        "Page returns an error (4xx)",
        "The URL returns a client error such as 404 or 410.",
        "Restore the page, redirect it to the closest replacement, or remove links and sitemap entries pointing to it."
    ),
    issue!(
        "redirect-chain",
        Warning,
        "crawlability",
        "Redirect chain",
        "The URL takes more than one redirect hop to reach its destination, which wastes crawl budget and slows users down.",
        "Point the first URL (and any links to it) straight at the final destination."
    ),
    issue!(
        "too-many-redirects",
        Warning,
        "crawlability",
        "Too many redirects",
        "The URL redirected more than 5 times without reaching a page.",
        "Collapse the redirects into a single hop to the final URL."
    ),
    issue!(
        "duplicate-title",
        Warning,
        "content",
        "Duplicate title",
        "Several indexable pages share the same title, which makes them hard to tell apart in search results.",
        "Give every page a unique title that describes its specific content."
    ),
    issue!(
        "duplicate-meta-description",
        Warning,
        "content",
        "Duplicate meta description",
        "Several indexable pages share the same meta description.",
        "Write a unique meta description for each page."
    ),
    issue!(
        "duplicate-content",
        Warning,
        "content",
        "Duplicate page content",
        "Several indexable pages have identical visible text.",
        "Consolidate the pages, or point duplicates at the main version with a canonical tag."
    ),
    issue!(
        "missing-meta-description",
        Warning,
        "content",
        "Missing meta description",
        "Without a meta description, search engines pick a snippet from the page that may not sell the click.",
        "Add a meta description of roughly 70–160 characters summarising the page."
    ),
    issue!(
        "canonical-conflict",
        Warning,
        "indexability",
        "Conflicting canonical signals",
        "The HTML canonical tag and the HTTP Link header point to different URLs.",
        "Make both signals point to the same preferred URL, or keep only one."
    ),
    issue!(
        "thin-content",
        Warning,
        "content",
        "Thin content",
        "The page has fewer than 150 words of visible text.",
        "Expand the page with useful content, merge it into a stronger page, or noindex it if it is a utility page."
    ),
    issue!(
        "images-missing-alt",
        Warning,
        "accessibility",
        "Images missing alt text",
        "Some images have no alt attribute. Alt text helps screen readers and image search.",
        "Add descriptive alt text; use alt=\"\" for purely decorative images."
    ),
    issue!(
        "orphan-page",
        Warning,
        "links",
        "Orphan page",
        "The page is in your sitemap but no crawled page links to it.",
        "Link to it from relevant pages, or remove it from the sitemap if it should not exist."
    ),
    issue!(
        "no-outgoing-links",
        Warning,
        "links",
        "Page has no outgoing links",
        "The page links to no other page on your site, which is a dead end for users and crawlers.",
        "Add navigation or contextual links to related pages."
    ),
    issue!(
        "missing-h1",
        Warning,
        "content",
        "Missing H1 heading",
        "The page has no <h1>, the main on-page heading.",
        "Add one <h1> that states the page topic."
    ),
    issue!(
        "mixed-content",
        Warning,
        "security",
        "Insecure resources on HTTPS page",
        "The HTTPS page loads scripts, images or styles over plain HTTP. Browsers block or warn about these.",
        "Load every resource over HTTPS."
    ),
    issue!(
        "invalid-structured-data",
        Warning,
        "structured-data",
        "Invalid JSON-LD",
        "A JSON-LD block on the page is not valid JSON, so search engines ignore it.",
        "Fix the JSON syntax; validate with a structured data testing tool."
    ),
    issue!(
        "redirected-internal-link",
        Info,
        "links",
        "Internal link to a redirect",
        "Pages link to an internal URL that redirects. Each hop adds latency.",
        "Update the links to point at the final URL."
    ),
    issue!(
        "multiple-h1",
        Info,
        "content",
        "Multiple H1 headings",
        "The page has more than one <h1>.",
        "Keep a single <h1> and use <h2>–<h6> for sub-sections."
    ),
    issue!(
        "multiple-titles",
        Info,
        "content",
        "Multiple title tags",
        "The page has more than one <title>; only the first is used.",
        "Remove the extra <title> tags."
    ),
    issue!(
        "missing-lang",
        Info,
        "accessibility",
        "Missing language attribute",
        "The <html> element has no lang attribute.",
        "Add lang, for example <html lang=\"en\">."
    ),
    issue!(
        "missing-viewport",
        Info,
        "mobile",
        "Missing viewport tag",
        "Without a viewport meta tag, mobile browsers render the page zoomed out.",
        "Add <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">."
    ),
    issue!(
        "title-too-long",
        Info,
        "content",
        "Title too long",
        "Titles over about 60 characters are usually cut off in search results.",
        "Shorten the title and put the most important words first."
    ),
    issue!(
        "title-too-short",
        Info,
        "content",
        "Title too short",
        "Titles under 10 characters rarely describe the page well.",
        "Write a more descriptive title."
    ),
    issue!(
        "meta-description-too-long",
        Info,
        "content",
        "Meta description too long",
        "Descriptions over about 160 characters are usually truncated.",
        "Shorten the description."
    ),
    issue!(
        "meta-description-too-short",
        Info,
        "content",
        "Meta description too short",
        "Descriptions under about 70 characters waste space in search results.",
        "Expand the description with what the page offers."
    ),
    issue!(
        "heading-order-skip",
        Info,
        "accessibility",
        "Heading levels skip",
        "Headings jump more than one level (for example H2 to H4).",
        "Use heading levels in order so the outline makes sense."
    ),
    issue!(
        "slow-response",
        Info,
        "performance",
        "Slow server response",
        "The server took more than 1.5 seconds to respond.",
        "Investigate server performance: caching, database queries, hosting capacity."
    ),
    issue!(
        "noindex-page",
        Info,
        "indexability",
        "Page is noindex",
        "The page asks search engines not to index it (meta robots or X-Robots-Tag).",
        "Confirm this is intended; remove noindex from pages that should rank."
    ),
    issue!(
        "canonicalized-page",
        Info,
        "indexability",
        "Canonicalized to another URL",
        "The page's canonical points to a different URL, so this URL will not be indexed itself.",
        "Confirm the canonical target is right; link to the canonical URL directly."
    ),
    issue!(
        "deep-page",
        Info,
        "links",
        "Page is deep in the site structure",
        "The page is 5 or more clicks from the start URL.",
        "Link to it from higher-level pages so users and crawlers reach it faster."
    ),
    issue!(
        "sitemap-non-indexable",
        Info,
        "indexability",
        "Non-indexable URL in sitemap",
        "The sitemap lists a URL that is an error, a redirect, noindex or canonicalized elsewhere.",
        "List only final, indexable, canonical URLs in the sitemap."
    ),
    issue!(
        "robots-disallowed",
        Info,
        "crawlability",
        "Blocked by robots.txt",
        "A discovered URL is disallowed for crawlers by robots.txt, so it was not audited.",
        "If the page should be indexed, allow it in robots.txt."
    ),
];

pub fn issue_type(id: &str) -> Option<&'static IssueType> {
    CATALOG.iter().find(|t| t.id == id)
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub issue: &'static str,
    pub details: Value,
}

impl Finding {
    pub fn new(issue: &'static str, details: Value) -> Self {
        debug_assert!(issue_type(issue).is_some(), "unknown issue {issue}");
        Self { issue, details }
    }
    pub fn severity(&self) -> Severity {
        issue_type(self.issue).map_or(Severity::Info, |t| t.severity)
    }
}

/// What the page rules need about one fetched URL.
pub struct PageInput<'a> {
    pub url: &'a str,
    pub class: FetchClass,
    pub status: Option<u16>,
    pub redirect_hops: usize,
    pub elapsed_ms: u32,
    pub depth: i32,
    pub x_robots_tag: Option<&'a str>,
    pub header_canonical: Option<&'a str>,
    /// Resolved absolute canonical (HTML), if any.
    pub canonical_abs: Option<&'a str>,
    pub header_canonical_abs: Option<&'a str>,
    /// Number of links to in-scope pages.
    pub internal_links: usize,
    pub facts: Option<&'a PageFacts>,
}

pub fn is_noindex(meta_robots: Option<&str>, x_robots: Option<&str>) -> bool {
    [meta_robots, x_robots].into_iter().flatten().any(|v| {
        v.to_ascii_lowercase()
            .split([',', ' '])
            .any(|d| d.trim() == "noindex" || d.trim() == "none")
    })
}

/// Indexable = HTML 200, not noindex, canonical (if any) points to itself.
pub fn is_indexable(p: &PageInput) -> bool {
    p.class == FetchClass::Html
        && !is_noindex(
            p.facts.and_then(|f| f.meta_robots.as_deref()),
            p.x_robots_tag,
        )
        && p.canonical_abs.is_none_or(|c| same_url(c, p.url))
}

fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

pub fn page_rules(p: &PageInput) -> Vec<Finding> {
    let mut out = Vec::new();
    let status = json!({ "status": p.status });
    match p.class {
        FetchClass::Blocked => out.push(Finding::new("blocked-page", status.clone())),
        FetchClass::ServerError => out.push(Finding::new("server-error", status.clone())),
        FetchClass::ClientError => out.push(Finding::new("broken-page", status.clone())),
        FetchClass::RateLimited => out.push(Finding::new("rate-limited-page", status.clone())),
        FetchClass::RedirectLoop => out.push(Finding::new(
            "redirect-loop",
            json!({ "hops": p.redirect_hops }),
        )),
        FetchClass::TooManyRedirects => out.push(Finding::new(
            "too-many-redirects",
            json!({ "hops": p.redirect_hops }),
        )),
        _ => {}
    }
    if p.redirect_hops > 1
        && !matches!(
            p.class,
            FetchClass::RedirectLoop | FetchClass::TooManyRedirects
        )
    {
        out.push(Finding::new(
            "redirect-chain",
            json!({ "hops": p.redirect_hops }),
        ));
    }
    if p.status.is_some() && p.elapsed_ms > SLOW_RESPONSE_MS {
        out.push(Finding::new("slow-response", json!({ "ms": p.elapsed_ms })));
    }
    if p.class == FetchClass::Html && p.depth >= DEEP_PAGE_DEPTH {
        out.push(Finding::new("deep-page", json!({ "depth": p.depth })));
    }

    let Some(f) = p.facts else { return out };
    if p.class != FetchClass::Html {
        return out;
    }

    out.extend(crate::extended::page_rules(p));
    let noindex = is_noindex(f.meta_robots.as_deref(), p.x_robots_tag);
    if noindex {
        out.push(Finding::new(
            "noindex-page",
            json!({ "meta_robots": f.meta_robots, "x_robots_tag": p.x_robots_tag }),
        ));
    }
    if let Some(c) = p.canonical_abs
        && !same_url(c, p.url)
    {
        out.push(Finding::new(
            "canonicalized-page",
            json!({ "canonical": c }),
        ));
    }
    if let (Some(html), Some(header)) = (p.canonical_abs, p.header_canonical_abs)
        && !same_url(html, header)
    {
        out.push(Finding::new(
            "canonical-conflict",
            json!({ "html": html, "header": header }),
        ));
    }

    // An empty SPA shell: content checks would be noise, so report that instead.
    let shell = f.has_app_root && f.word_count < 50 && f.links.is_empty() && f.headings.is_empty();
    if shell {
        out.push(Finding::new(
            "javascript-rendering-suspected",
            json!({ "words": f.word_count }),
        ));
        return out;
    }

    match f.title.as_deref() {
        None | Some("") => out.push(Finding::new("missing-title", json!({}))),
        Some(t) => {
            let n = t.chars().count();
            if n > TITLE_MAX {
                out.push(Finding::new(
                    "title-too-long",
                    json!({ "length": n, "title": t }),
                ));
            } else if n < TITLE_MIN {
                out.push(Finding::new(
                    "title-too-short",
                    json!({ "length": n, "title": t }),
                ));
            }
        }
    }
    if f.title_count > 1 {
        out.push(Finding::new(
            "multiple-titles",
            json!({ "count": f.title_count }),
        ));
    }
    match f.meta_description.as_deref().map(str::trim) {
        None | Some("") => out.push(Finding::new("missing-meta-description", json!({}))),
        Some(d) => {
            let n = d.chars().count();
            if n > META_DESC_MAX {
                out.push(Finding::new(
                    "meta-description-too-long",
                    json!({ "length": n }),
                ));
            } else if n < META_DESC_MIN {
                out.push(Finding::new(
                    "meta-description-too-short",
                    json!({ "length": n }),
                ));
            }
        }
    }
    match f.h1s.len() {
        0 => out.push(Finding::new("missing-h1", json!({}))),
        1 => {}
        n => out.push(Finding::new(
            "multiple-h1",
            json!({ "count": n, "h1s": f.h1s.iter().take(5).collect::<Vec<_>>() }),
        )),
    }
    if let Some((from, to)) = f
        .headings
        .windows(2)
        .find(|w| w[1] > w[0] + 1)
        .map(|w| (w[0], w[1]))
    {
        out.push(Finding::new(
            "heading-order-skip",
            json!({ "from": format!("h{from}"), "to": format!("h{to}") }),
        ));
    }
    if !noindex && f.word_count < THIN_CONTENT_WORDS {
        out.push(Finding::new(
            "thin-content",
            json!({ "words": f.word_count }),
        ));
    }
    let missing_alt = f.images.iter().filter(|i| !i.has_alt).count();
    if missing_alt > 0 {
        let sample: Vec<&str> = f
            .images
            .iter()
            .filter(|i| !i.has_alt)
            .take(5)
            .map(|i| i.src.as_str())
            .collect();
        out.push(Finding::new(
            "images-missing-alt",
            json!({ "count": missing_alt, "total": f.images.len(), "sample": sample }),
        ));
    }
    if p.internal_links == 0 {
        out.push(Finding::new("no-outgoing-links", json!({})));
    }
    if f.lang.is_none() {
        out.push(Finding::new("missing-lang", json!({})));
    }
    if !f.has_viewport {
        out.push(Finding::new("missing-viewport", json!({})));
    }
    if p.url.starts_with("https://") && !f.insecure_resources.is_empty() {
        out.push(Finding::new("mixed-content", json!({ "count": f.insecure_resources.len(), "sample": f.insecure_resources.iter().take(5).collect::<Vec<_>>() })));
    }
    let invalid_ld = f
        .json_ld
        .iter()
        .filter(|b| serde_json::from_str::<Value>(b.trim()).is_err())
        .count();
    if invalid_ld > 0 {
        out.push(Finding::new(
            "invalid-structured-data",
            json!({ "invalid_blocks": invalid_ld }),
        ));
    }
    out
}

/// Whole-site summary of one page for cross-page rules.
pub struct SitePage {
    pub key: usize,
    pub url_hash: String,
    pub url: String,
    pub class: FetchClass,
    pub status: Option<u16>,
    pub indexable: bool,
    pub redirected: bool,
    pub in_sitemap: bool,
    pub is_start: bool,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub text_hash: Option<String>,
}

/// A site finding attached to the page with `key`.
pub struct SiteFinding {
    pub key: usize,
    pub finding: Finding,
}

/// Cross-page rules. `links` are (from page key, to url hash) for in-scope links.
pub fn site_rules(pages: &[SitePage], links: &[(usize, String)]) -> Vec<SiteFinding> {
    let mut out = Vec::new();
    let by_hash: HashMap<&str, &SitePage> =
        pages.iter().map(|p| (p.url_hash.as_str(), p)).collect();

    // Duplicates among indexable pages.
    let dup = |field: fn(&SitePage) -> Option<&str>,
               issue: &'static str,
               out: &mut Vec<SiteFinding>| {
        let mut groups: HashMap<String, Vec<&SitePage>> = HashMap::new();
        for p in pages.iter().filter(|p| p.indexable) {
            if let Some(v) = field(p).map(str::trim).filter(|v| !v.is_empty()) {
                groups.entry(v.to_lowercase()).or_default().push(p);
            }
        }
        for group in groups.values().filter(|g| g.len() > 1) {
            let sample: Vec<&str> = group
                .iter()
                .take(DUPLICATE_SAMPLE)
                .map(|p| p.url.as_str())
                .collect();
            for p in group {
                out.push(SiteFinding {
                    key: p.key,
                    finding: Finding::new(issue, json!({ "count": group.len(), "sample": sample })),
                });
            }
        }
    };
    dup(|p| p.title.as_deref(), "duplicate-title", &mut out);
    dup(
        |p| p.meta_description.as_deref(),
        "duplicate-meta-description",
        &mut out,
    );
    dup(|p| p.text_hash.as_deref(), "duplicate-content", &mut out);

    // Link targets: broken and redirected internal links, attributed to the linking page.
    let mut reported: HashSet<(usize, &str, &'static str)> = HashSet::new();
    let mut linked: HashSet<&str> = HashSet::new();
    for (from, to) in links {
        linked.insert(to.as_str());
        let Some(target) = by_hash.get(to.as_str()) else {
            continue;
        };
        let issue = match target.class {
            FetchClass::ClientError | FetchClass::ServerError => Some("broken-internal-link"),
            _ if target.redirected => Some("redirected-internal-link"),
            _ => None,
        };
        if let Some(issue) = issue
            && reported.insert((*from, to.as_str(), issue))
        {
            out.push(SiteFinding {
                key: *from,
                finding: Finding::new(
                    issue,
                    json!({ "target": target.url, "status": target.status }),
                ),
            });
        }
    }

    for p in pages {
        if p.in_sitemap
            && !p.is_start
            && !linked.contains(p.url_hash.as_str())
            && p.class == FetchClass::Html
        {
            out.push(SiteFinding {
                key: p.key,
                finding: Finding::new("orphan-page", json!({})),
            });
        }
        if p.in_sitemap && !p.indexable {
            out.push(SiteFinding {
                key: p.key,
                finding: Finding::new(
                    "sitemap-non-indexable",
                    json!({ "status": p.status, "redirected": p.redirected }),
                ),
            });
        }
    }
    out
}

/// 0–100 score: start at 100 and subtract per affected page, weighted by severity.
pub fn health_score(pages_ok: usize, findings: &[(Severity, usize)]) -> i32 {
    if pages_ok == 0 {
        return 0;
    }
    let mut penalty = 0f64;
    for (sev, affected) in findings {
        let share = (*affected as f64 / pages_ok as f64).min(1.0);
        penalty += share
            * match sev {
                Severity::Critical => 30.0,
                Severity::Warning => 8.0,
                Severity::Info => 1.0,
            };
    }
    (100.0 - penalty).clamp(0.0, 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract;

    fn input<'a>(url: &'a str, facts: &'a PageFacts) -> PageInput<'a> {
        PageInput {
            url,
            class: FetchClass::Html,
            status: Some(200),
            redirect_hops: 0,
            elapsed_ms: 100,
            depth: 1,
            x_robots_tag: None,
            header_canonical: None,
            canonical_abs: facts.canonical.as_deref(),
            header_canonical_abs: None,
            internal_links: facts.links.len(),
            facts: Some(facts),
        }
    }

    fn ids(f: &[Finding]) -> Vec<&str> {
        let mut v: Vec<&str> = f.iter().map(|f| f.issue).collect();
        v.sort();
        v
    }

    #[test]
    fn catalog_ids_unique() {
        let mut seen = HashSet::new();
        assert!(CATALOG.iter().all(|t| seen.insert(t.id)));
    }

    #[test]
    fn bad_page_findings() {
        let f = extract(br#"<html><head><title>Hi</title></head><body><h2>a</h2><h4>b</h4><img src="x.png"></body></html>"#);
        let found = page_rules(&input("https://ex.com/", &f));
        assert_eq!(
            ids(&found),
            vec![
                "heading-order-skip",
                "images-missing-alt",
                "missing-h1",
                "missing-lang",
                "missing-meta-description",
                "missing-viewport",
                "no-outgoing-links",
                "thin-content",
                "title-too-short",
            ]
        );
    }

    #[test]
    fn extended_rules_use_evidence_and_skip_normal_pages() {
        let f = extract(br#"<title>404 Page not found</title><h1>Page not found</h1><link rel="alternate" hreflang="en_US" href="/relative"><link rel="alternate" hreflang="de" href="https://ex.com/de">"#);
        let found = page_rules(&input("https://ex.com/Bad_URL", &f));
        for id in [
            "soft-404",
            "url-hygiene",
            "invalid-hreflang",
            "hreflang-missing-self",
        ] {
            assert!(ids(&found).contains(&id));
        }
        let f = extract(br#"<title>How to fix 404 responses</title><h1>Troubleshooting</h1><link rel="alternate" hreflang="en" href="https://ex.com/help">"#);
        let found = page_rules(&input("https://ex.com/help", &f));
        assert!(!ids(&found).contains(&"soft-404"));
        assert!(!ids(&found).contains(&"hreflang-missing-self"));
    }

    #[test]
    fn good_page_is_clean() {
        let words = "word ".repeat(200);
        let html = format!(
            r#"<html lang="en"><head><title>A perfectly reasonable page title</title>
<meta name="viewport" content="width=device-width"><meta name="description" content="{}">
<link rel="canonical" href="https://ex.com/page"></head>
<body><h1>Topic</h1><h2>Sub</h2><p>{words}</p><a href="/other">Other</a><img src="a.png" alt="A"></body></html>"#,
            "d".repeat(100)
        );
        let f = extract(html.as_bytes());
        assert!(page_rules(&input("https://ex.com/page", &f)).is_empty());
    }

    #[test]
    fn spa_shell_short_circuits_content_checks() {
        let f = extract(br#"<html><head><title>App</title></head><body><div id="root"></div><script src="/app.js"></script></body></html>"#);
        assert_eq!(
            ids(&page_rules(&input("https://ex.com/", &f))),
            vec!["javascript-rendering-suspected"]
        );
    }

    #[test]
    fn noindex_and_canonical() {
        assert!(is_noindex(Some("noindex,follow"), None));
        assert!(is_noindex(None, Some("none")));
        assert!(!is_noindex(Some("index, follow"), None));
        let f = extract(br#"<html><head><meta name="robots" content="noindex"><link rel="canonical" href="https://ex.com/other"></head></html>"#);
        let findings = page_rules(&input("https://ex.com/page", &f));
        let found = ids(&findings);
        assert!(found.contains(&"noindex-page") && found.contains(&"canonicalized-page"));
        assert!(!found.contains(&"thin-content"));
    }

    #[test]
    fn site_rules_find_duplicates_orphans_and_broken_links() {
        let page =
            |key: usize, url: &str, class: FetchClass, title: &str, sitemap: bool| SitePage {
                key,
                url_hash: format!("h{key}"),
                url: url.into(),
                class,
                status: Some(if class == FetchClass::Html { 200 } else { 404 }),
                indexable: class == FetchClass::Html,
                redirected: false,
                in_sitemap: sitemap,
                is_start: key == 0,
                title: Some(title.into()),
                meta_description: None,
                text_hash: None,
            };
        let pages = vec![
            page(0, "https://ex.com/", FetchClass::Html, "Same", true),
            page(1, "https://ex.com/a", FetchClass::Html, "Same", true),
            page(
                2,
                "https://ex.com/gone",
                FetchClass::ClientError,
                "x",
                false,
            ),
            page(3, "https://ex.com/orphan", FetchClass::Html, "Unique", true),
        ];
        let links = vec![
            (0, "h1".to_string()),
            (0, "h2".to_string()),
            (1, "h2".to_string()),
        ];
        let found: Vec<(usize, &str)> = site_rules(&pages, &links)
            .iter()
            .map(|s| (s.key, s.finding.issue))
            .collect();
        assert!(found.contains(&(0, "duplicate-title")) && found.contains(&(1, "duplicate-title")));
        assert!(
            found.contains(&(0, "broken-internal-link"))
                && found.contains(&(1, "broken-internal-link"))
        );
        assert!(found.contains(&(3, "orphan-page")));
        assert!(!found.iter().any(|(k, i)| *k == 0 && *i == "orphan-page"));
    }

    #[test]
    fn score_bounds() {
        assert_eq!(health_score(10, &[]), 100);
        assert_eq!(health_score(10, &[(Severity::Critical, 10)]), 70);
        assert_eq!(health_score(0, &[]), 0);
    }
}
