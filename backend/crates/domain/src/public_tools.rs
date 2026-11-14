//! Anonymous, bounded tools. Submitted content is never persisted or rendered as HTML.
use common::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use url::Url;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicToolInput {
    /// keywords | serp-preview | backlinks | authority | domain-age | robots | sitemap |
    /// meta | schema | redirects | hreflang
    pub tool: String,
    /// Seed, HTML, robots.txt, XML, or JSON-LD according to the tool.
    #[serde(default)]
    pub content: String,
    /// Public URL for resolution, target matching, robots testing or redirect checks.
    pub url: Option<String>,
}
fn invalid(message: &str) -> AppError {
    AppError::Validation(message.into())
}
fn public_url(raw: Option<&str>) -> AppResult<Url> {
    let url = Url::parse(raw.ok_or_else(|| invalid("enter a complete public URL"))?)
        .map_err(|_| invalid("enter a complete public URL"))?;
    crawler::UrlPolicy::default()
        .check_url(&url)
        .map_err(|e| invalid(&e.to_string()))?;
    if url.as_str().len() > 2048 {
        return Err(invalid("URL exceeds 2048 bytes"));
    }
    Ok(url)
}
pub async fn run(pool: &PgPool, v: PublicToolInput) -> AppResult<Value> {
    if v.content.len() > 256 * 1024 {
        return Err(invalid("maximum input size is 256 KiB"));
    }
    let network = matches!(v.tool.as_str(), "domain-age" | "redirects");
    if !db::extras::public_budget(pool, network).await? {
        return Err(AppError::Unavailable(
            "public tool request budget reached; try again in one minute".into(),
        ));
    }
    match v.tool.as_str() {
        "domain-age" => {
            let url = public_url(v.url.as_deref())?;
            let domain = crate::competitive::normalize_domain(url.host_str().unwrap_or_default())?;
            Ok(
                serde_json::to_value(crate::domain_age::public_lookup(pool, &domain).await?)
                    .map_err(AppError::internal)?,
            )
        }
        "authority" => {
            let u = public_url(v.url.as_deref())?;
            let domain = crate::competitive::normalize_domain(u.host_str().unwrap_or_default())?;
            let rows = db::providers::domain_ranks(pool, std::slice::from_ref(&domain)).await?;
            Ok(
                json!({"domain":domain,"sr":rows.first().map(|r|r.sr),"release":rows.first().map(|r|&r.release),"source":"common_crawl_host_graph","note":"SeoSeo SR is a log-scaled harmonic rank from the operator's imported sample, not Moz DA or Ahrefs DR. Missing means unknown."}),
            )
        }
        "redirects" => {
            let u = public_url(v.url.as_deref())?;
            redirect_check(u).await
        }
        _ => analyze(&v),
    }
}

async fn redirect_check(mut current: Url) -> AppResult<Value> {
    use std::time::Duration;
    let policy = crawler::UrlPolicy::default();
    let client = crawler::fetch::guarded_client(policy, Duration::from_secs(8))
        .map_err(AppError::internal)?;
    let origin = current.origin();
    let mut hops = vec![];
    let mut seen = std::collections::HashSet::new();
    // Read robots directly, with no redirect chain to an unchecked origin.
    let robots_url = current.join("/robots.txt").map_err(AppError::internal)?;
    let mut response = client
        .get(robots_url)
        .send()
        .await
        .map_err(|_| invalid("robots policy unavailable"))?;
    let status = response.status().as_u16();
    if response
        .headers()
        .get("cf-mitigated")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"challenge"))
    {
        return Err(invalid("robots bot challenge; check stopped"));
    }
    if !(status == 404 || status == 410 || (200..300).contains(&status)) {
        return Err(invalid("robots policy unavailable; check stopped"));
    }
    let mut bytes = vec![];
    if status == 200 {
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| invalid("robots response interrupted"))?
        {
            if bytes.len() + chunk.len() > crawler::robots::MAX_ROBOTS_BYTES {
                return Err(invalid("robots file too large"));
            }
            bytes.extend_from_slice(&chunk);
        }
    }
    let robots = crawler::robots::Robots::parse(crawler::ROBOTS_TOKEN, &bytes, Some(status));
    for _ in 0..=5 {
        policy
            .check_url(&current)
            .map_err(|e| invalid(&e.to_string()))?;
        if current.origin() != origin {
            return Ok(
                json!({"hops":hops,"next_url":current,"stopped":"off_origin","source":"live_http"}),
            );
        }
        if !seen.insert(current.to_string()) {
            return Ok(json!({"hops":hops,"stopped":"redirect_loop","source":"live_http"}));
        }
        if !robots.allowed(&current) {
            return Ok(json!({"hops":hops,"stopped":"robots_disallowed","source":"live_http"}));
        }
        let delay = robots.crawl_delay_secs.unwrap_or(0.25).max(0.25);
        if !delay.is_finite() || delay > 2.0 {
            return Err(invalid(
                "robots crawl delay exceeds this interactive tool's budget",
            ));
        }
        tokio::time::sleep(Duration::from_secs_f32(delay)).await;
        let response = client
            .get(current.clone())
            .send()
            .await
            .map_err(|_| AppError::Unavailable("redirect check could not reach host".into()))?;
        let status = response.status().as_u16();
        let challenge = response
            .headers()
            .get("cf-mitigated")
            .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"challenge"));
        let next = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| current.join(v).ok());
        hops.push(json!({"url":current,"status":status,"location":next}));
        if challenge || matches!(status, 403 | 429) {
            return Ok(
                json!({"hops":hops,"stopped":"blocked_or_rate_limited","source":"live_http"}),
            );
        }
        if !(300..400).contains(&status) {
            return Ok(
                json!({"hops":hops,"final_url":current,"stopped":"complete","source":"live_http","note":"HTTP headers only; no response body or JavaScript. Every followed path passed submitted host robots rules."}),
            );
        }
        let Some(next) = next else {
            return Ok(json!({"hops":hops,"stopped":"missing_location","source":"live_http"}));
        };
        current = next;
    }
    Ok(json!({"hops":hops,"next_url":current,"stopped":"hop_limit","source":"live_http"}))
}
pub fn analyze(v: &PublicToolInput) -> AppResult<Value> {
    if v.content.len() > 256 * 1024 {
        return Err(invalid("maximum input size is 256 KiB"));
    }
    match v.tool.as_str() {
        "keywords" => {
            let seed = scoring::normalize_keyword(&v.content);
            if seed.is_empty() || seed.chars().count() > 120 {
                return Err(invalid("enter a seed of 1–120 characters"));
            }
            let ideas = [
                "{}",
                "best {}",
                "{} guide",
                "how to choose {}",
                "{} near me",
                "{} prices",
                "{} alternatives",
                "{} comparison",
                "free {}",
                "{} for beginners",
                "{} checklist",
                "what is {}",
            ]
            .map(|s| s.replace("{}", &seed));
            Ok(
                json!({"ideas":ideas,"source":"local_templates","volume":null,"note":"Brainstorming prompts, not measured demand or autocomplete."}),
            )
        }
        "robots" => {
            let url = public_url(v.url.as_deref())?;
            let r = crawler::robots::Robots::parse(
                crawler::ROBOTS_TOKEN,
                v.content.as_bytes(),
                Some(200),
            );
            Ok(
                json!({"allowed":r.allowed(&url),"agent":crawler::ROBOTS_TOKEN,"sitemaps":r.sitemaps,"crawl_delay_seconds":r.crawl_delay_secs,"source":"submitted_robots","note":"Tests submitted rules only. Invalid directives may be ignored; this is not an HTTP availability test."}),
            )
        }
        "sitemap" => {
            let mut reader = quick_xml::Reader::from_str(&v.content);
            reader.config_mut().check_end_names = true;
            let mut root = None;
            let mut depth = 0;
            let mut roots = 0;
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Start(e)) => {
                        if depth == 0 {
                            roots += 1;
                            root = Some(e.local_name().as_ref().to_string());
                        }
                        depth += 1;
                    }
                    Ok(quick_xml::events::Event::Empty(e)) => {
                        if depth == 0 {
                            roots += 1;
                            root = Some(e.local_name().as_ref().to_string());
                        }
                    }
                    Ok(quick_xml::events::Event::End(_)) => {
                        if depth == 0 {
                            return Err(invalid("unbalanced XML"));
                        }
                        depth -= 1;
                    }
                    Ok(quick_xml::events::Event::DocType(_)) => {
                        return Err(invalid("DOCTYPE is not supported"));
                    }
                    Ok(quick_xml::events::Event::Text(t))
                        if depth == 0 && !t.xml10_content().trim().is_empty() =>
                    {
                        return Err(invalid("text outside sitemap root"));
                    }
                    Ok(quick_xml::events::Event::Eof) => break,
                    Err(_) => return Err(invalid("malformed sitemap XML")),
                    _ => {}
                }
            }
            if depth != 0
                || roots != 1
                || !matches!(root.as_deref(), Some("urlset" | "sitemapindex"))
            {
                return Err(invalid(
                    "expected one complete urlset or sitemapindex XML root",
                ));
            }
            let p = crawler::sitemap::parse(v.content.as_bytes());
            let invalid_urls: Vec<_> = p
                .urls
                .iter()
                .chain(&p.sitemaps)
                .filter(|s| public_url(Some(s)).is_err())
                .take(100)
                .collect();
            Ok(
                json!({"root":root,"urls":p.urls.len(),"sitemaps":p.sitemaps.len(),"invalid_urls":invalid_urls,"source":"submitted_xml","note":"Checks syntax and public absolute locations; does not fetch locations or validate the full sitemap XSD."}),
            )
        }
        "schema" => {
            let data: Value =
                serde_json::from_str(&v.content).map_err(|_| invalid("enter valid JSON-LD"))?;
            let nodes: Vec<&Value> = match &data {
                Value::Array(a) => a.iter().collect(),
                Value::Object(o) => o
                    .get("@graph")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().collect())
                    .unwrap_or(vec![&data]),
                _ => return Err(invalid("JSON-LD must be an object or array")),
            };
            let typed = nodes.iter().filter(|n| n.get("@type").is_some()).count();
            Ok(
                json!({"nodes":nodes.len(),"typed_nodes":typed,"has_context":data.get("@context").is_some() || nodes.iter().any(|n|n.get("@context").is_some()),"source":"submitted_json_ld","note":"Syntax and basic shape check only. Not a Schema.org vocabulary validator or a Google rich-result eligibility test."}),
            )
        }
        "meta" | "serp-preview" | "backlinks" | "hreflang" => {
            let f = crawler::extract::extract(v.content.as_bytes());
            match v.tool.as_str() {
                "meta" | "serp-preview" => Ok(
                    json!({"title":f.title,"description":f.meta_description,"canonical":f.canonical,"robots":f.meta_robots,"title_characters":f.title.as_ref().map(|s|s.chars().count()),"description_characters":f.meta_description.as_ref().map(|s|s.chars().count()),"source":"submitted_html","note":"Search engines may rewrite snippets. Character counts are editorial hints, not ranking rules."}),
                ),
                "hreflang" => {
                    let base = public_url(v.url.as_deref())?;
                    let rows:Vec<_>=f.hreflang.iter().map(|(lang,href)|json!({"language":lang,"href":href,"resolves":base.join(href).ok().filter(|u|matches!(u.scheme(),"http"|"https")).map(|u|u.to_string())})).collect();
                    let mut seen = std::collections::HashSet::new();
                    let duplicate_languages: Vec<_> = f
                        .hreflang
                        .iter()
                        .filter(|(l, _)| !seen.insert(l.to_ascii_lowercase()))
                        .map(|(l, _)| l)
                        .collect();
                    Ok(
                        json!({"alternates":rows,"duplicate_languages":duplicate_languages,"source":"submitted_html","note":"Only checks this document; reciprocal links, target language and HTTP indexability are unverified."}),
                    )
                }
                _ => {
                    let target = public_url(v.url.as_deref())?;
                    let links: Vec<_> = f
                        .links
                        .iter()
                        .filter(|l| {
                            Url::parse(&l.href)
                                .ok()
                                .is_some_and(|u| u.host_str() == target.host_str())
                        })
                        .collect();
                    Ok(
                        json!({"matching_links":links,"source":"submitted_source_html","note":"Checks absolute links to the target host in one submitted source page. No site-wide backlink discovery or link freshness verification."}),
                    )
                }
            }
        }
        _ => Err(invalid("unknown public tool")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(tool: &str, content: &str) -> PublicToolInput {
        PublicToolInput {
            tool: tool.into(),
            content: content.into(),
            url: Some("https://example.com/private".into()),
        }
    }
    #[test]
    fn robots_and_meta_use_real_parsers() {
        assert_eq!(
            analyze(&input("robots", "User-agent: *\nDisallow: /private")).unwrap()["allowed"],
            false
        );
        assert_eq!(
            analyze(&input("meta", "<title>A &amp; B</title>")).unwrap()["title"],
            "A & B"
        );
    }
    #[test]
    fn rejects_partial_xml_and_private_targets() {
        assert!(analyze(&input("sitemap", "<urlset><url>")).is_err());
        assert!(analyze(&input("sitemap", "<urlset/>outside")).is_err());
        let mut v = input("robots", "");
        v.url = Some("http://127.0.0.1/".into());
        assert!(analyze(&v).is_err());
        assert_eq!(
            analyze(&input(
                "sitemap",
                "<urlset><url><loc>https://example.com/</loc></url></urlset>"
            ))
            .unwrap()["urls"],
            1
        );
    }
}
