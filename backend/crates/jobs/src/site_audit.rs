//! `site_audit` job: probe → discover → crawl → analyze.
//!
//! All crawl state lives in Postgres (frontier, pages, links, issues), so a worker crash or
//! restart resumes where it stopped: pages are keyed by URL hash and inserted at most once,
//! and site-wide rules are recomputed from scratch in the analyze phase.

use crate::JobCtx;
use crawler::fetch::{FetchClass, FetchOutcome};
use crawler::robots::Robots;
use crawler::rules::{self, PageInput, Severity, SitePage};
use crawler::urls::{self, Scope};
use crawler::{Fetcher, FetcherConfig, UrlPolicy, extract};
use db::audits::{self, FrontierInsert, NewIssue, NewLink, NewPage};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use url::Url;
use uuid::Uuid;

pub const KIND: &str = "site_audit";
/// Hard stop so a tarpit site cannot hold a worker forever.
const MAX_CRAWL_TIME: Duration = Duration::from_secs(2 * 60 * 60);
/// Consecutive 429s before we give up politely.
const MAX_RATE_LIMITED: u32 = 3;

/// User-tunable audit settings (validated by the domain service).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub max_pages: u32,
    pub concurrency: u32,
    /// Minimum delay between requests per worker slot.
    pub delay_ms: u64,
    pub respect_robots: bool,
    #[serde(default)]
    pub render_js: bool,
    #[serde(default)]
    pub lighthouse: bool,
    #[serde(skip)]
    pub renderer_url: Option<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_pages: 500,
            concurrency: 4,
            delay_ms: 250,
            respect_robots: true,
            render_js: false,
            lighthouse: false,
            renderer_url: None,
            exclude_paths: Vec::new(),
        }
    }
}

/// Operator-level crawler settings (environment).
#[derive(Debug, Clone, Default)]
pub struct CrawlSettings {
    pub user_agent: Option<String>,
    pub allow_private: bool,
    pub renderer_url: Option<String>,
}

#[derive(Deserialize)]
struct Payload {
    audit_id: Uuid,
}

pub fn register(registry: crate::Registry, settings: CrawlSettings) -> crate::Registry {
    registry.register(KIND, move |ctx: JobCtx| {
        let settings = settings.clone();
        async move { run(ctx, settings).await }
    })
}

async fn run(ctx: JobCtx, settings: CrawlSettings) -> anyhow::Result<()> {
    let payload: Payload = serde_json::from_value(ctx.job.payload.clone())?;
    let audit_id = payload.audit_id;
    let result = run_audit(&ctx, &settings, audit_id).await;
    if let Err(e) = &result {
        // Only the final attempt marks the audit failed; earlier attempts are retried.
        if ctx.job.attempts >= ctx.job.max_attempts {
            let _ = audits::fail(&ctx.pool, audit_id, &format!("{e:#}")).await;
        }
    }
    result
}

async fn run_audit(ctx: &JobCtx, settings: &CrawlSettings, audit_id: Uuid) -> anyhow::Result<()> {
    let pool = &ctx.pool;
    let audit = audits::get_any(pool, audit_id).await?;
    if matches!(audit.status.as_str(), "completed" | "failed" | "cancelled") {
        return Ok(());
    }
    if !scheduled_site_current(ctx, audit_id).await? {
        return Ok(());
    }
    let mut cfg: AuditConfig = serde_json::from_value(audit.config.clone()).unwrap_or_default();
    cfg.renderer_url = settings.renderer_url.clone();
    let fetcher = Fetcher::new(FetcherConfig {
        user_agent: settings
            .user_agent
            .clone()
            .unwrap_or_else(|| crawler::DEFAULT_USER_AGENT.into()),
        policy: UrlPolicy {
            allow_private: settings.allow_private,
        },
        timeout: Duration::from_secs(20),
    })?;

    // ---- probe: resolve the start URL (http→https, apex→www) to define the crawl scope.
    let mut start = Url::parse(&audit.start_url)?;
    if audit.phase == "queued" || audit.phase == "probe" {
        audits::set_phase(pool, audit_id, "probe").await?;
        ctx.progress(json!({ "phase": "probe" })).await?;
        let probe = fetcher.fetch_page(&start, None).await;
        match probe.class {
            FetchClass::Html => {}
            FetchClass::PolicyBlocked | FetchClass::NetworkError => {
                let reason = probe
                    .error
                    .unwrap_or_else(|| "could not reach the site".into());
                audits::fail(
                    pool,
                    audit_id,
                    &format!("Could not start the audit: {reason}"),
                )
                .await?;
                return Ok(());
            }
            _ => {} // Error pages are still audited (they become issues).
        }
        if let Some(final_url) = urls::normalize(&probe.final_url)
            && final_url != start
            && probe.class != FetchClass::RedirectOffsite
        {
            start = final_url;
            audits::set_start_url(pool, audit_id, start.as_str()).await?;
        }
        if ctx.job.payload["scheduled"].as_bool() == Some(true) {
            let expected = ctx.job.payload["expected_domain"].as_str().unwrap_or("");
            let root = Url::parse(&format!("https://{expected}/"))?;
            if !Scope::new(&root).contains(&start) {
                audits::fail(
                    pool,
                    audit_id,
                    "Scheduled crawl stopped: start URL redirected outside the reviewed website",
                )
                .await?;
                return Ok(());
            }
        }
        audits::frontier_add(
            pool,
            audit_id,
            &[FrontierInsert {
                url_hash: &urls::hash(&start),
                url: start.as_str(),
                depth: 0,
                source: "start",
                in_sitemap: false,
            }],
        )
        .await?;
        audits::set_phase(pool, audit_id, "discover").await?;
    }
    let scope = Scope::new(&start);

    // robots.txt is needed by both discovery and crawl.
    let robots = Robots::fetch(&fetcher, &start, crawler::ROBOTS_TOKEN).await;

    // ---- discover: sitemaps (robots-listed, else /sitemap.xml).
    let phase = audits::get_any(pool, audit_id).await?.phase;
    if phase == "discover" {
        ctx.progress(json!({ "phase": "discover" })).await?;
        let mut roots: Vec<Url> = robots
            .sitemaps
            .iter()
            .filter_map(|s| Url::parse(s).ok())
            .collect();
        if roots.is_empty()
            && let Ok(u) = start.join("/sitemap.xml")
        {
            roots.push(u);
        }
        let limit = (cfg.max_pages as usize).saturating_mul(3);
        let found = crawler::sitemap::discover(&fetcher, roots, &scope, limit).await;
        let hashes: Vec<String> = found.urls.iter().map(urls::hash).collect();
        let rows: Vec<FrontierInsert> = found
            .urls
            .iter()
            .zip(&hashes)
            .filter(|(u, _)| !excluded(&cfg, u))
            .map(|(u, h)| FrontierInsert {
                url_hash: h,
                url: u.as_str(),
                depth: 1,
                source: "sitemap",
                in_sitemap: true,
            })
            .collect();
        audits::frontier_add(pool, audit_id, &rows).await?;
        tracing::info!(audit = %audit_id, sitemap_urls = rows.len(), docs = found.documents, "discovery done");
        audits::set_phase(pool, audit_id, "crawl").await?;
    }

    // ---- crawl
    let phase = audits::get_any(pool, audit_id).await?.phase;
    if phase == "crawl" {
        let stop = crawl(ctx, &fetcher, &robots, &scope, &cfg, audit_id).await?;
        if stop == Stop::Cancelled {
            return Ok(());
        }
        if let Some(reason) = stop.reason() {
            audits::set_stop_reason(pool, audit_id, reason).await?;
        }
        audits::frontier_skip_pending(pool, audit_id).await?;
        audits::set_phase(pool, audit_id, "analyze").await?;
    }

    // ---- analyze: graph metrics, site-wide rules, score.
    ctx.progress(json!({ "phase": "analyze" })).await?;
    analyze(pool, audit_id, &start).await?;
    Ok(())
}

fn excluded(cfg: &AuditConfig, url: &Url) -> bool {
    cfg.exclude_paths
        .iter()
        .any(|p| !p.is_empty() && url.path().starts_with(p.as_str()))
}

/// A queued recurring crawl must not outlive its website authorization. In-flight
/// requests may finish; check again before claiming every further crawl batch.
async fn scheduled_site_current(ctx: &JobCtx, audit_id: Uuid) -> anyhow::Result<bool> {
    if ctx.job.payload["scheduled"].as_bool() != Some(true) {
        return Ok(true);
    }
    let expected = ctx.job.payload["expected_domain"].as_str().unwrap_or("");
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND organization_id=$2 AND domain::text=$3 AND archived_at IS NULL)")
        .bind(ctx.job.project_id).bind(ctx.job.organization_id).bind(expected).fetch_one(&ctx.pool).await?;
    if !current {
        audits::fail(
            &ctx.pool,
            audit_id,
            "Scheduled crawl stopped: website changed or project archived",
        )
        .await?;
    }
    Ok(current)
}

#[derive(Debug, PartialEq, Eq)]
enum Stop {
    Finished,
    Budget,
    TimeLimit,
    RateLimited,
    Cancelled,
}

impl Stop {
    fn reason(&self) -> Option<&'static str> {
        match self {
            Stop::Finished | Stop::Cancelled => None,
            Stop::Budget => Some("page_limit"),
            Stop::TimeLimit => Some("time_limit"),
            Stop::RateLimited => Some("rate_limited"),
        }
    }
}

async fn crawl(
    ctx: &JobCtx,
    fetcher: &Fetcher,
    robots: &Robots,
    scope: &Scope,
    cfg: &AuditConfig,
    audit_id: Uuid,
) -> anyhow::Result<Stop> {
    let pool = &ctx.pool;
    let started = Instant::now();
    let concurrency = cfg.concurrency.clamp(1, 8) as usize;
    let robots_delay = robots
        .crawl_delay_secs
        .map(|s| (s.min(10.0) * 1000.0) as u64)
        .unwrap_or(0);
    let delay = Duration::from_millis(cfg.delay_ms.max(robots_delay));
    let mut rate_limited_streak = 0u32;

    loop {
        if !scheduled_site_current(ctx, audit_id).await? {
            return Ok(Stop::Cancelled);
        }
        if audits::status(pool, audit_id).await? == "cancelled" {
            return Ok(Stop::Cancelled);
        }
        if started.elapsed() > MAX_CRAWL_TIME {
            return Ok(Stop::TimeLimit);
        }
        let (crawled, _) = audits::refresh_progress(pool, audit_id).await?;
        let remaining = cfg.max_pages as i64 - crawled as i64;
        if remaining <= 0 {
            let pending = audits::frontier_next(pool, audit_id, 1).await?;
            return Ok(if pending.is_empty() {
                Stop::Finished
            } else {
                Stop::Budget
            });
        }
        let batch =
            audits::frontier_next(pool, audit_id, remaining.min(concurrency as i64 * 4)).await?;
        if batch.is_empty() {
            return Ok(Stop::Finished);
        }
        ctx.progress(json!({ "phase": "crawl", "crawled": crawled }))
            .await?;

        // Robots-disallowed URLs are recorded, not fetched.
        let mut to_fetch = Vec::new();
        for row in batch {
            let url = Url::parse(&row.url)?;
            if cfg.respect_robots && !robots.allowed(&url) {
                audits::frontier_mark(pool, audit_id, &row.url_hash, "blocked").await?;
                let issue = NewIssue {
                    page_id: None,
                    page_url: Some(row.url.clone()),
                    issue_type: "robots-disallowed".into(),
                    severity: Severity::Info.as_str().into(),
                    details: json!({}),
                };
                audits::replace_issues(pool, audit_id, &[], &[issue]).await?;
            } else {
                to_fetch.push((row, url));
            }
        }

        let results: Vec<_> = futures::stream::iter(to_fetch.into_iter().enumerate())
            .map(|(i, (row, url))| async move {
                // Stagger the first requests of a batch, then keep `delay` per slot.
                tokio::time::sleep(delay * ((i % concurrency) as u32)).await;
                let out = fetcher.fetch_page(&url, Some(scope)).await;
                tokio::time::sleep(delay).await;
                (row, url, out)
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        for (row, url, out) in results {
            if out.class == FetchClass::RateLimited {
                rate_limited_streak += 1;
            } else {
                rate_limited_streak = 0;
            }
            store_page(pool, audit_id, scope, cfg, &row, &url, out, fetcher, robots).await?;
        }
        if rate_limited_streak >= MAX_RATE_LIMITED {
            return Ok(Stop::RateLimited);
        }
        if rate_limited_streak > 0 {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn store_page(
    pool: &sqlx::PgPool,
    audit_id: Uuid,
    scope: &Scope,
    cfg: &AuditConfig,
    row: &audits::FrontierRow,
    url: &Url,
    out: FetchOutcome,
    fetcher: &Fetcher,
    robots: &Robots,
) -> anyhow::Result<()> {
    let page_id = common::new_id();
    // A redirecting URL is not a page of its own: its destination is crawled separately,
    // so skip content checks and links here to avoid double counting.
    let redirected = !out.redirects.is_empty();
    let mut browser_findings = Vec::new();
    let mut browser_result = None;
    let mut rendered = None;
    if !redirected && out.class == FetchClass::Html && (cfg.render_js || cfg.lighthouse) {
        match crate::renderer::render(cfg.renderer_url.as_deref(), &out.final_url, cfg.lighthouse)
            .await
        {
            Ok(result) => {
                if cfg.render_js {
                    rendered = Some(result.html.into_bytes());
                }
                if let Some(error) = &result.lighthouse_error {
                    browser_findings.push(rules::Finding::new(
                        "lighthouse-failed",
                        json!({"reason": error}),
                    ));
                }
                browser_result =
                    Some(json!({"rendered": cfg.render_js, "lighthouse": result.lighthouse}));
            }
            Err(e) => {
                if cfg.render_js && cfg.lighthouse {
                    browser_findings.push(rules::Finding::new(
                        "lighthouse-failed",
                        json!({"reason": e.to_string()}),
                    ));
                }
                browser_findings.push(rules::Finding::new(
                    if cfg.render_js {
                        "render-failed"
                    } else {
                        "lighthouse-failed"
                    },
                    json!({"reason": e.to_string()}),
                ));
            }
        }
    }
    let body = rendered.as_ref().or(out.body.as_ref());
    let facts = match (&out.class, body) {
        _ if redirected => None,
        (FetchClass::Html, Some(body)) => {
            let body = body.clone();
            Some(tokio::task::spawn_blocking(move || extract::extract(&body)).await?)
        }
        _ => None,
    };

    // Resolve links against <base href> (if any) or the final URL.
    let base = facts
        .as_ref()
        .and_then(|f| f.base_href.as_deref())
        .and_then(|b| out.final_url.join(b).ok())
        .unwrap_or_else(|| out.final_url.clone());
    let mut internal: Vec<NewLink> = Vec::new();
    let mut external = 0i32;
    let mut new_frontier: Vec<(String, Url)> = Vec::new();
    if let Some(f) = &facts {
        let mut seen = std::collections::HashSet::new();
        for link in &f.links {
            let Some(target) = urls::resolve(&base, &link.href) else {
                continue;
            };
            if !scope.contains(&target) {
                external += 1;
                continue;
            }
            let h = urls::hash(&target);
            if !seen.insert(h.clone()) {
                continue;
            }
            if !urls::looks_like_asset(&target) && !link.nofollow && !excluded(cfg, &target) {
                new_frontier.push((h.clone(), target.clone()));
            }
            internal.push(NewLink {
                to_url_hash: h,
                to_url: target.to_string(),
                anchor: Some(link.text.clone()).filter(|t| !t.is_empty()),
                nofollow: link.nofollow,
            });
        }
    }

    if let Some(f) = &facts {
        for (_, raw) in &f.hreflang {
            if let Ok(target) = Url::parse(raw)
                && let Some(target) = urls::normalize(&target)
                && scope.contains(&target)
                && !excluded(cfg, &target)
            {
                new_frontier.push((urls::hash(&target), target));
            }
        }
    }

    let canonical_abs = facts
        .as_ref()
        .and_then(|f| f.canonical.as_deref())
        .and_then(|c| base.join(c).ok())
        .and_then(|u| urls::normalize(&u))
        .map(|u| u.to_string());
    let header_canonical_abs = out
        .header_canonical
        .as_deref()
        .and_then(|c| out.final_url.join(c).ok())
        .and_then(|u| urls::normalize(&u))
        .map(|u| u.to_string());
    let page_url = urls::normalize(&out.final_url)
        .unwrap_or_else(|| url.clone())
        .to_string();
    let input = PageInput {
        url: &page_url,
        class: out.class,
        status: out.status,
        redirect_hops: out.redirects.len(),
        elapsed_ms: out.elapsed_ms,
        depth: 0, // click depth is computed from the link graph in `analyze`
        x_robots_tag: out.x_robots_tag.as_deref(),
        header_canonical: out.header_canonical.as_deref(),
        canonical_abs: canonical_abs.as_deref(),
        header_canonical_abs: header_canonical_abs.as_deref(),
        internal_links: internal.len(),
        facts: facts.as_ref(),
    };
    let indexable = !redirected && rules::is_indexable(&input);
    let mut findings = rules::page_rules(&input);
    findings.extend(browser_findings);
    if let Some(f) = &facts {
        let mut checked = std::collections::HashSet::new();
        // Bound extra requests; apply the same DNS policy and robots restrictions as pages.
        for raw in &f.assets {
            let Some(asset) = urls::resolve(&base, raw) else {
                continue;
            };
            if asset.origin() != url.origin() || !checked.insert(asset.to_string()) {
                continue;
            }
            if checked.len() > 20 {
                break;
            }
            if cfg.respect_robots && !robots.allowed(&asset) {
                continue;
            }
            tokio::time::sleep(Duration::from_millis(
                cfg.delay_ms
                    .max((robots.crawl_delay_secs.unwrap_or(0.0) * 1000.0) as u64),
            ))
            .await;
            let result = fetcher.fetch_page(&asset, Some(scope)).await;
            if matches!(result.class, FetchClass::Blocked | FetchClass::RateLimited) {
                break;
            }
            if matches!(
                result.class,
                FetchClass::ClientError | FetchClass::ServerError
            ) {
                findings.push(rules::Finding::new(
                    "broken-asset",
                    json!({"url": asset, "status": result.status}),
                ));
            } else if result.size > 1024 * 1024 {
                findings.push(rules::Finding::new(
                    "large-asset",
                    json!({"url": asset, "bytes": result.size}),
                ));
            }
        }
    }

    let f = facts.as_ref();
    let page = NewPage {
        browser_result,
        id: page_id,
        url: url.to_string(),
        url_hash: row.url_hash.clone(),
        final_url: (out.final_url != *url).then(|| out.final_url.to_string()),
        status_code: out.status.map(i32::from),
        fetch_class: out.class.as_str().into(),
        content_type: out.content_type.clone(),
        response_time_ms: out.elapsed_ms as i32,
        size_bytes: out.size.min(i32::MAX as usize) as i32,
        redirect_chain: serde_json::to_value(&out.redirects)?,
        depth: row.depth,
        in_sitemap: row.in_sitemap,
        title: f.and_then(|f| f.title.clone()),
        meta_description: f.and_then(|f| f.meta_description.clone()),
        meta_robots: f.and_then(|f| f.meta_robots.clone()),
        x_robots_tag: out.x_robots_tag.clone(),
        canonical_url: canonical_abs.clone(),
        header_canonical_url: header_canonical_abs.clone(),
        is_indexable: indexable,
        lang: f.and_then(|f| f.lang.clone()),
        has_viewport: f.is_some_and(|f| f.has_viewport),
        h1: f.and_then(|f| f.h1s.first().cloned()),
        h1_count: f.map_or(0, |f| f.h1s.len() as i32),
        word_count: f.map_or(0, |f| f.word_count as i32),
        images_total: f.map_or(0, |f| f.images.len() as i32),
        images_missing_alt: f.map_or(0, |f| f.images.iter().filter(|i| !i.has_alt).count() as i32),
        internal_links: internal.len() as i32,
        external_links: external,
        structured_data_count: f.map_or(0, |f| f.json_ld.len() as i32),
        og_title: f.and_then(|f| f.og_title.clone()),
        og_description: f.and_then(|f| f.og_description.clone()),
        og_image: f.and_then(|f| f.og_image.clone()),
        hreflang: serde_json::to_value(f.map(|f| f.hreflang.clone()).unwrap_or_default())?,
        content_hash: f.and_then(|f| f.text_hash.clone()),
        error: out.error.clone(),
    };
    let issues: Vec<NewIssue> = findings
        .iter()
        .map(|fd| NewIssue {
            page_id: Some(page_id),
            page_url: Some(url.to_string()),
            issue_type: fd.issue.into(),
            severity: fd.severity().as_str().into(),
            details: fd.details.clone(),
        })
        .collect();

    audits::insert_page(pool, audit_id, &page, &internal, &issues).await?;

    // A redirect's in-scope destination is a page in its own right.
    if !out.redirects.is_empty()
        && out.class != FetchClass::RedirectOffsite
        && let Some(dest) = urls::normalize(&out.final_url)
        && scope.contains(&dest)
    {
        new_frontier.push((urls::hash(&dest), dest));
    }

    // Cap the frontier so link-heavy sites cannot grow it without bound.
    if !new_frontier.is_empty()
        && audits::frontier_count(pool, audit_id).await? < (cfg.max_pages as i64) * 5
    {
        let rows: Vec<FrontierInsert> = new_frontier
            .iter()
            .map(|(h, u)| FrontierInsert {
                url_hash: h,
                url: u.as_str(),
                depth: row.depth + 1,
                source: "link",
                in_sitemap: false,
            })
            .collect();
        audits::frontier_add(pool, audit_id, &rows).await?;
    }
    audits::frontier_mark(pool, audit_id, &row.url_hash, "done").await?;
    Ok(())
}

/// Site rules whose results are recomputed wholesale on every analyze run.
const SITE_ISSUES: &[&str] = &[
    "hreflang-target-error",
    "hreflang-missing-return",
    "duplicate-title",
    "duplicate-meta-description",
    "duplicate-content",
    "broken-internal-link",
    "redirected-internal-link",
    "orphan-page",
    "sitemap-non-indexable",
    "deep-page",
];

async fn analyze(pool: &sqlx::PgPool, audit_id: Uuid, start: &Url) -> anyhow::Result<()> {
    let pages = audits::page_summaries(pool, audit_id).await?;
    let edges = audits::link_edges(pool, audit_id).await?;
    let start_hash = urls::hash(start);

    let key_of: HashMap<Uuid, usize> = pages.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let by_hash: HashMap<&str, usize> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.url_hash.as_str(), i))
        .collect();

    // Inlinks (distinct source pages) and BFS click depth from the start page.
    let mut inlinks = vec![0i32; pages.len()];
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); pages.len()];
    let mut link_pairs: Vec<(usize, String)> = Vec::with_capacity(edges.len());
    for (from, to_hash) in &edges {
        let Some(&from_key) = key_of.get(from) else {
            continue;
        };
        link_pairs.push((from_key, to_hash.clone()));
        if let Some(&to_key) = by_hash.get(to_hash.as_str())
            && to_key != from_key
        {
            inlinks[to_key] += 1;
            adjacency[from_key].push(to_key);
        }
    }
    let mut depth = vec![-1i32; pages.len()];
    if let Some(&s) = by_hash.get(start_hash.as_str()) {
        depth[s] = 0;
        let mut queue = VecDeque::from([s]);
        while let Some(n) = queue.pop_front() {
            for &m in &adjacency[n] {
                if depth[m] < 0 {
                    depth[m] = depth[n] + 1;
                    queue.push_back(m);
                }
            }
        }
    }
    let metrics: Vec<(Uuid, i32, i32)> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, inlinks[i], depth[i]))
        .collect();
    audits::update_graph_metrics(pool, audit_id, &metrics).await?;

    let class_of = |s: &str| match s {
        "html" => FetchClass::Html,
        "client_error" => FetchClass::ClientError,
        "server_error" => FetchClass::ServerError,
        "blocked" => FetchClass::Blocked,
        "rate_limited" => FetchClass::RateLimited,
        "redirect_loop" => FetchClass::RedirectLoop,
        "too_many_redirects" => FetchClass::TooManyRedirects,
        "redirect_offsite" => FetchClass::RedirectOffsite,
        "policy_blocked" => FetchClass::PolicyBlocked,
        "network_error" => FetchClass::NetworkError,
        _ => FetchClass::NonHtml,
    };
    let site_pages: Vec<SitePage> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| SitePage {
            key: i,
            url_hash: p.url_hash.clone(),
            url: p.url.clone(),
            class: class_of(&p.fetch_class),
            status: p.status_code.map(|s| s as u16),
            indexable: p.is_indexable,
            redirected: p.redirected,
            in_sitemap: p.in_sitemap,
            is_start: p.url_hash == start_hash,
            title: p.title.clone(),
            meta_description: p.meta_description.clone(),
            text_hash: p.content_hash.clone(),
        })
        .collect();

    let mut issues: Vec<NewIssue> = rules::site_rules(&site_pages, &link_pairs)
        .into_iter()
        .map(|sf| NewIssue {
            page_id: Some(pages[sf.key].id),
            page_url: Some(pages[sf.key].url.clone()),
            issue_type: sf.finding.issue.into(),
            severity: sf.finding.severity().as_str().into(),
            details: sf.finding.details,
        })
        .collect();
    for (i, p) in pages.iter().enumerate() {
        if p.fetch_class == "html" && depth[i] >= rules::DEEP_PAGE_DEPTH {
            issues.push(NewIssue {
                page_id: Some(p.id),
                page_url: Some(p.url.clone()),
                issue_type: "deep-page".into(),
                severity: Severity::Info.as_str().into(),
                details: json!({ "depth": depth[i] }),
            });
        }
    }
    let by_url: HashMap<&str, &audits::PageSummary> =
        pages.iter().map(|p| (p.url.as_str(), p)).collect();
    for p in &pages {
        let alternates: Vec<(String, String)> =
            serde_json::from_value(p.hreflang.clone()).unwrap_or_default();
        for (_, target) in &alternates {
            let Some(other) = by_url.get(target.as_str()) else {
                continue;
            };
            let other_alts: Vec<(String, String)> =
                serde_json::from_value(other.hreflang.clone()).unwrap_or_default();
            let issue = if !other.is_indexable || other.redirected {
                Some("hreflang-target-error")
            } else if !other_alts.iter().any(|(_, u)| u == &p.url) {
                Some("hreflang-missing-return")
            } else {
                None
            };
            if let Some(issue) = issue {
                issues.push(NewIssue {
                    page_id: Some(p.id),
                    page_url: Some(p.url.clone()),
                    issue_type: issue.into(),
                    severity: "warning".into(),
                    details: json!({"target": target}),
                });
            }
        }
    }
    audits::replace_issues(pool, audit_id, SITE_ISSUES, &issues).await?;

    // Summary counts and health score.
    let counts = audits::issue_counts(pool, audit_id).await?;
    let mut by_sev: HashMap<&str, i64> = HashMap::new();
    let mut by_type = serde_json::Map::new();
    let mut weighted = Vec::new();
    for c in &counts {
        *by_sev.entry(c.severity.as_str()).or_default() += c.pages;
        by_type.insert(c.issue_type.clone(), json!(c.pages));
        let sev = match c.severity.as_str() {
            "critical" => Severity::Critical,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        };
        weighted.push((sev, c.pages as usize));
    }
    let score = rules::health_score(pages.len(), &weighted);
    let summary = json!({
        "critical": by_sev.get("critical").copied().unwrap_or(0),
        "warning": by_sev.get("warning").copied().unwrap_or(0),
        "info": by_sev.get("info").copied().unwrap_or(0),
        "by_type": by_type,
    });
    audits::refresh_progress(pool, audit_id).await?;
    audits::finish(pool, audit_id, score, summary, None).await?;
    tracing::info!(audit = %audit_id, pages = pages.len(), score, "audit completed");
    Ok(())
}
