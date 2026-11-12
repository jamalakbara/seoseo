use crate::{JobCtx, ProviderCtx};
use common::{AppError, AppResult, Config};
use db::{backlink_discovery as repo, backlinks::BacklinkEvidence};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use url::Url;

pub fn register(registry: crate::Registry, cfg: Config, pctx: ProviderCtx) -> crate::Registry {
    registry.register("backlink_discovery", move |ctx: JobCtx| {
        let cfg = cfg.clone();
        let pctx = pctx.clone();
        async move {
            let id = ctx.job.payload["discovery_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing discovery ID"))?
                .parse()?;
            let d = repo::any(&ctx.pool, id).await?;
            if matches!(d.status.as_str(), "completed" | "failed") {
                return Ok(());
            }
            repo::status(&ctx.pool, id, "running", None, None).await?;
            match collect(&ctx, &cfg, &pctx, &d).await {
                Ok(()) => {}
                Err(error) => {
                    repo::status(&ctx.pool, id, "failed", None, Some(&error.to_string())).await?
                }
            }
            Ok(())
        }
    })
}
async fn current(ctx: &JobCtx, d: &repo::BacklinkDiscovery) -> AppResult<()> {
    let p = db::projects::get(
        &ctx.pool,
        ctx.job.organization_id.ok_or(AppError::Unauthorized)?,
        d.project_id,
    )
    .await?;
    if p.domain.as_deref() != Some(&d.domain) || p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "Website changed or project archived during discovery".into(),
        ));
    }
    Ok(())
}
fn within(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

async fn collect(
    ctx: &JobCtx,
    cfg: &Config,
    pctx: &ProviderCtx,
    d: &repo::BacklinkDiscovery,
) -> AppResult<()> {
    let org = ctx.job.organization_id.ok_or(AppError::Unauthorized)?;
    current(ctx, d).await?;
    let previous = repo::previous(&ctx.pool, org, d.project_id, &d.domain).await?;
    let mut candidates: Vec<String> = previous
        .iter()
        .map(|r| r.source_url.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut seen: BTreeSet<String> = candidates.iter().cloned().collect();
    let mut provider = "explicit_sources".to_string();
    let mut proposed = d.config.source_urls.clone();
    if d.config.search {
        let registry = pctx.registry(&ctx.pool, org).await?;
        if !registry.has_serp() {
            return Err(AppError::Conflict("Configure free SearXNG or another SERP provider, or supply source URLs with search=false".into()));
        }
        let page = registry
            .search(&providers::SerpQuery {
                keyword: format!("\"{}\" -site:{}", d.domain, d.domain),
                market: providers::Market::new(&d.config.country, &d.config.language),
            })
            .await
            .map_err(crate::provider_ctx::provider_err)?;
        provider = page.provider.into();
        proposed.extend(page.items.into_iter().map(|i| i.url));
    }
    for raw in proposed {
        if let Ok(mut u) = Url::parse(&raw) {
            u.set_fragment(None);
            if crawler::UrlPolicy::default().check_url(&u).is_ok()
                && u.port().is_none()
                && u.as_str().len() <= 2048
                && u.host_str().is_some_and(|h| {
                    scoring::registrable_domain(h) != scoring::registrable_domain(&d.domain)
                })
                && seen.insert(u.to_string())
            {
                candidates.push(u.to_string());
            }
        }
    }
    // Prior sources are checked first; candidates beyond this cap retain prior evidence as unknown.
    candidates.truncate(40);
    let fetcher = crawler::Fetcher::new(crawler::FetcherConfig {
        user_agent: providers::USER_AGENT.into(),
        policy: crawler::UrlPolicy {
            allow_private: cfg.crawler_allow_private,
        },
        timeout: std::time::Duration::from_secs(10),
    })
    .map_err(AppError::internal)?;
    let mut robots = HashMap::<String, crawler::robots::Robots>::new();
    let mut stopped = BTreeSet::new();
    let mut definitive = BTreeSet::new();
    let mut found = BTreeMap::<(String, String), BacklinkEvidence>::new();
    let mut inconclusive = 0usize;
    for (index, raw) in candidates.iter().enumerate() {
        current(ctx, d).await?;
        ctx.progress(json!({"checked":index,"total":candidates.len()}))
            .await?;
        let Ok(url) = Url::parse(raw) else {
            inconclusive += 1;
            continue;
        };
        let origin = url.origin().ascii_serialization();
        if stopped.contains(&origin) {
            inconclusive += 1;
            continue;
        }
        if !robots.contains_key(&origin) {
            robots.insert(
                origin.clone(),
                crawler::robots::Robots::fetch(&fetcher, &url, crawler::ROBOTS_TOKEN).await,
            );
        }
        let policy = &robots[&origin];
        if !matches!(policy.status, Some(200) | Some(404) | Some(410))
            || !policy.allowed(&url)
            || policy.crawl_delay_secs.is_some_and(|delay| delay > 30.0)
        {
            inconclusive += 1;
            continue;
        }
        tokio::time::sleep(std::time::Duration::from_secs_f32(
            policy.crawl_delay_secs.unwrap_or(1.0).max(1.0),
        ))
        .await;
        let Ok((status, content_type, body)) = fetcher
            .get_exact_resource(&url, crawler::fetch::MAX_HTML_BYTES)
            .await
        else {
            stopped.insert(origin);
            inconclusive += 1;
            continue;
        };
        if matches!(status, 401 | 403 | 429) {
            stopped.insert(origin);
            inconclusive += 1;
            continue;
        }
        if matches!(status, 404 | 410) {
            definitive.insert(raw.clone());
            continue;
        }
        if status != 200
            || !content_type
                .as_deref()
                .is_some_and(|t| t.to_ascii_lowercase().contains("text/html"))
        {
            inconclusive += 1;
            continue;
        }
        let facts = crawler::extract::extract(&body);
        let title = facts.title.as_deref().unwrap_or("").to_ascii_lowercase();
        if facts.meta_refresh.is_some()
            || (facts.has_app_root && facts.word_count < 30)
            || [
                "just a moment",
                "verify you are human",
                "access denied",
                "captcha",
            ]
            .iter()
            .any(|t| title.contains(t))
        {
            stopped.insert(origin);
            inconclusive += 1;
            continue;
        }
        if facts.links.len() >= crawler::extract::MAX_LINKS {
            inconclusive += 1;
            continue;
        }
        definitive.insert(raw.clone());
        let base = facts
            .base_href
            .as_deref()
            .and_then(|b| url.join(b).ok())
            .unwrap_or_else(|| url.clone());
        for link in &facts.links {
            let Ok(mut target) = base.join(&link.href) else {
                continue;
            };
            target.set_fragment(None);
            if target.as_str().len() > 2048
                || target.port().is_some()
                || crawler::UrlPolicy::default().check_url(&target).is_err()
                || !target.host_str().is_some_and(|h| within(h, &d.domain))
            {
                continue;
            }
            let Some(source_domain) = url.host_str().and_then(scoring::registrable_domain) else {
                continue;
            };
            let (score, reasons) = spam(&facts, &link.text, &url);
            let nofollow = link.nofollow
                || facts
                    .meta_robots
                    .as_deref()
                    .is_some_and(|r| r.to_ascii_lowercase().contains("nofollow"));
            let key = (raw.clone(), target.to_string());
            if let Some(row) = found.get_mut(&key) {
                row.nofollow = Some(row.nofollow.unwrap_or(false) && nofollow);
                continue;
            }
            found.insert(
                key,
                BacklinkEvidence {
                    source_url: raw.clone(),
                    target_url: target.to_string(),
                    source_domain,
                    anchor: Some(link.text.chars().take(500).collect()),
                    nofollow: Some(nofollow),
                    state: "live".into(),
                    verification: "verified".into(),
                    verified_at: Some(chrono::Utc::now()),
                    spam_score: Some(score),
                    spam_reasons: json!(reasons),
                },
            );
            if found.len() > 1000 {
                return Err(AppError::Conflict(
                    "Discovery sample exceeds 1000 pairs; previous snapshot preserved".into(),
                ));
            }
        }
    }
    let mut retained = 0;
    for mut row in previous {
        let key = (row.source_url.clone(), row.target_url.clone());
        if !definitive.contains(&row.source_url) && !found.contains_key(&key) {
            row.verification = "unconfirmed".into();
            row.spam_score = None;
            row.spam_reasons = json!([]);
            found.insert(key, row);
            retained += 1;
        }
    }
    let mut rows: Vec<_> = found.into_values().collect();
    if rows.len() > 1000 {
        return Err(AppError::Conflict(
            "Discovery sample exceeds 1000 pairs; previous snapshot preserved".into(),
        ));
    }
    rows.sort_by(|a, b| (&a.source_url, &a.target_url).cmp(&(&b.source_url, &b.target_url)));
    current(ctx, d).await?;
    db::backlinks::import(&ctx.pool,org,d.project_id,db::backlinks::ImportSnapshot{expected_domain:&d.domain,dataset:"crawl_v1",links:rows,created_by:d.config.created_by,
        discovery_coverage:Some(json!({"provider":provider,"candidates":candidates.len(),"definitive_sources":definitive.len(),"inconclusive_sources":inconclusive,"retained_unconfirmed_pairs":retained,"method":"SERP/explicit-source bounded crawl; not Common Crawl inbound index or exhaustive web coverage"})),discovery_id:Some(d.id)}).await?;
    Ok(())
}

fn spam(facts: &crawler::PageFacts, anchor: &str, source: &Url) -> (i32, Vec<&'static str>) {
    let mut score = 0;
    let mut reasons = Vec::new();
    if facts.word_count < 100 {
        score += 40;
        reasons.push("source_under_100_words");
    }
    let external = facts
        .links
        .iter()
        .filter_map(|l| source.join(&l.href).ok())
        .filter(|u| u.host_str() != source.host_str())
        .count();
    if facts.links.len() >= 20 && external * 100 > facts.links.len() * 80 {
        score += 40;
        reasons.push("external_link_share_over_80_percent");
    }
    if anchor.chars().count() > 120 {
        score += 20;
        reasons.push("anchor_over_120_characters");
    }
    (score, reasons)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heuristic_is_evidence_based_and_bounded() {
        let facts = crawler::extract::extract(
            b"<html><p>Short text</p><a href='https://example.org/'>Link</a></html>",
        );
        let (score, reasons) = spam(&facts, "Link", &Url::parse("https://source.org/").unwrap());
        assert_eq!(score, 40);
        assert_eq!(reasons, ["source_under_100_words"]);
        assert!(within("a.example.com", "example.com"));
        assert!(!within("notexample.com", "example.com"));
    }
}
