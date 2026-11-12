//! Bounded competitive research from observed search results in a project's market.
use crate::{Actor, providers::DomainAuthority, validate};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use db::competitive::Evidence;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct DomainQuery {
    /// Domain or URL; defaults to the project's website. Subdomains roll up to the registrable domain.
    pub domain: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RankedKeyword {
    pub keyword: String,
    pub position: i32,
    pub url: String,
    pub provider: String,
    pub exact: bool,
    pub fetched_at: DateTime<Utc>,
    /// Null means the project website was absent from this sampled results page.
    pub project_position: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TopPage {
    pub url: String,
    pub keywords: usize,
    pub best_position: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SerpCompetitor {
    pub domain: String,
    /// Keywords for which both the requested domain and this domain were observed.
    pub shared_keywords: usize,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct PositionDistribution {
    pub top_3: usize,
    pub positions_4_10: usize,
    pub positions_11_20: usize,
    pub positions_21_30: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DomainOverview {
    pub domain: String,
    pub project_domain: Option<String>,
    pub country: String,
    pub language: String,
    pub source: &'static str,
    pub confidence: &'static str,
    pub authority: DomainAuthority,
    /// Unavailable: the local sample cannot establish monthly domain traffic.
    pub estimated_monthly_traffic: Option<i64>,
    pub scope_keywords: usize,
    pub sampled_keywords: usize,
    pub checked_keywords: usize,
    pub exact_keywords: usize,
    pub matched_keywords: usize,
    pub oldest_snapshot_at: Option<DateTime<Utc>>,
    pub newest_snapshot_at: Option<DateTime<Utc>>,
    pub distribution: PositionDistribution,
    /// Best result per keyword, capped at 100; totals/distribution use the full sample.
    pub keywords: Vec<RankedKeyword>,
    /// Best-result URLs, capped at 100.
    pub top_pages: Vec<TopPage>,
    pub competitors: Vec<SerpCompetitor>,
    /// Requested-domain keywords where the project's domain was not observed, capped at 100.
    pub keyword_gaps: Vec<String>,
    pub warnings: Vec<String>,
}

pub(crate) fn normalize_domain(input: &str) -> AppResult<String> {
    let host = validate::domain(input)?;
    scoring::registrable_domain(&host).ok_or_else(|| {
        AppError::Validation("enter a registrable domain such as example.com".into())
    })
}

pub async fn overview(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    q: DomainQuery,
) -> AppResult<DomainOverview> {
    let p = db::projects::get(pool, actor.org_id, project_id).await?;
    let domain =
        normalize_domain(q.domain.as_deref().or(p.domain.as_deref()).ok_or_else(|| {
            AppError::Validation("enter a domain or set the project's website".into())
        })?)?;
    let project_domain = p.domain.as_deref().map(normalize_domain).transpose()?;
    let country = validate::country(q.country.as_deref().unwrap_or(&p.country_code))?;
    let language = validate::language(q.language.as_deref().unwrap_or(&p.language_code))?;
    let rows =
        db::competitive::evidence(pool, actor.org_id, project_id, &country, &language).await?;
    let authority = crate::providers::lookup_authority(pool, &domain).await?;
    Ok(summarize(
        domain,
        project_domain,
        country,
        language,
        authority,
        rows,
    ))
}

fn summarize(
    domain: String,
    project_domain: Option<String>,
    country: String,
    language: String,
    authority: DomainAuthority,
    rows: Vec<Evidence>,
) -> DomainOverview {
    let scope_keywords = rows.first().map_or(0, |r| r.scope_total as usize);
    let mut sampled = BTreeSet::new();
    let mut checked = BTreeSet::new();
    let mut exact = BTreeSet::new();
    let mut oldest = None;
    let mut newest = None;
    let mut best: BTreeMap<(String, String), &Evidence> = BTreeMap::new();
    for r in &rows {
        sampled.insert(&r.keyword);
        if let Some(at) = r.fetched_at {
            checked.insert(&r.keyword);
            if r.exact == Some(true) {
                exact.insert(&r.keyword);
            }
            oldest = Some(oldest.map_or(at, |old: DateTime<Utc>| old.min(at)));
            newest = Some(newest.map_or(at, |old: DateTime<Utc>| old.max(at)));
        }
        if let (Some(d), Some(pos), Some(_)) = (&r.domain, r.position, &r.url) {
            let key = (r.keyword.clone(), d.clone());
            if best
                .get(&key)
                .is_none_or(|old| old.position.is_none_or(|p| pos < p))
            {
                best.insert(key, r);
            }
        }
    }
    let mut keywords = Vec::new();
    let mut distribution = PositionDistribution::default();
    let mut pages: BTreeMap<String, TopPage> = BTreeMap::new();
    let mut competitors: BTreeMap<String, usize> = BTreeMap::new();
    let mut gaps = Vec::new();
    for ((keyword, d), r) in &best {
        if d != &domain {
            continue;
        }
        let position = r.position.unwrap_or(0);
        let (Some(url), Some(provider), Some(at)) = (&r.url, &r.provider, r.fetched_at) else {
            continue;
        };
        match position {
            1..=3 => distribution.top_3 += 1,
            4..=10 => distribution.positions_4_10 += 1,
            11..=20 => distribution.positions_11_20 += 1,
            21..=30 => distribution.positions_21_30 += 1,
            _ => continue,
        }
        let project_position = project_domain
            .as_ref()
            .and_then(|p| best.get(&(keyword.clone(), p.clone())))
            .and_then(|r| r.position);
        if project_domain.as_ref().is_some_and(|p| p != &domain) && project_position.is_none() {
            gaps.push(keyword.clone());
        }
        keywords.push(RankedKeyword {
            keyword: keyword.clone(),
            position,
            url: url.clone(),
            provider: provider.clone(),
            exact: r.exact == Some(true),
            fetched_at: at,
            project_position,
        });
        let page = pages.entry(url.clone()).or_insert(TopPage {
            url: url.clone(),
            keywords: 0,
            best_position: position,
        });
        page.keywords += 1;
        page.best_position = page.best_position.min(position);
    }
    // Count overlap only on keywords where the requested domain was observed.
    let matched: BTreeSet<_> = keywords.iter().map(|k| &k.keyword).collect();
    for (keyword, d) in best.keys() {
        if d != &domain && matched.contains(keyword) {
            *competitors.entry(d.clone()).or_default() += 1;
        }
    }
    let matched_keywords = keywords.len();
    keywords.sort_by(|a, b| (a.position, &a.keyword).cmp(&(b.position, &b.keyword)));
    keywords.truncate(100);
    let mut top_pages: Vec<_> = pages.into_values().collect();
    top_pages.sort_by(|a, b| b.keywords.cmp(&a.keywords).then(a.url.cmp(&b.url)));
    top_pages.truncate(100);
    let mut competitors: Vec<_> = competitors
        .into_iter()
        .map(|(domain, shared_keywords)| SerpCompetitor {
            domain,
            shared_keywords,
        })
        .collect();
    competitors.sort_by(|a, b| {
        b.shared_keywords
            .cmp(&a.shared_keywords)
            .then(a.domain.cmp(&b.domain))
    });
    competitors.truncate(50);
    gaps.truncate(100);
    let mut warnings = vec!["Local sample of this project's saved/researched keywords; not an exhaustive domain index. Latest snapshot per keyword in the last 30 days, positions 1–30 only. Absence does not prove a domain does not rank.".into()];
    if sampled.len() < scope_keywords {
        warnings.push("Only the first 500 keywords alphabetically are sampled. Narrow the market to change the scope.".into());
    }
    if checked.len() < sampled.len() {
        warnings.push("Some keywords have no recent SERP snapshot. Analyze or refresh their search results to improve coverage.".into());
    }
    if exact.len() < checked.len() {
        warnings.push("Some snapshots are metasearch ordering rather than exact engine rankings; see each keyword's provider and exact flag.".into());
    }
    DomainOverview {
        domain,
        project_domain,
        country,
        language,
        source: "project_serp_sample",
        confidence: "relative",
        authority,
        estimated_monthly_traffic: None,
        scope_keywords,
        sampled_keywords: sampled.len(),
        checked_keywords: checked.len(),
        exact_keywords: exact.len(),
        matched_keywords,
        oldest_snapshot_at: oldest,
        newest_snapshot_at: newest,
        distribution,
        keywords,
        top_pages,
        competitors,
        keyword_gaps: gaps,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(keyword: &str, domain: &str, position: i32) -> Evidence {
        Evidence {
            keyword: keyword.into(),
            scope_total: 4,
            provider: Some("fixture".into()),
            exact: Some(true),
            fetched_at: Some(Utc::now()),
            domain: Some(domain.into()),
            position: Some(position),
            url: Some(format!("https://{domain}/page")),
        }
    }

    fn report(rows: Vec<Evidence>) -> DomainOverview {
        summarize(
            "rival.com".into(),
            Some("ours.com".into()),
            "US".into(),
            "en".into(),
            DomainAuthority {
                domain: "rival.com".into(),
                sr: None,
                harmonic_rank: None,
                pagerank_rank: None,
                source: "fixture",
                release: None,
                authority_loaded: false,
            },
            rows,
        )
    }

    #[test]
    fn best_result_counts_once_and_overlap_requires_shared_keyword() {
        let mut missing = row("unmeasured", "rival.com", 1);
        missing.domain = None;
        missing.position = None;
        missing.url = None;
        missing.provider = None;
        missing.fetched_at = None;
        missing.exact = None;
        let mut meta = row("gap", "rival.com", 12);
        meta.exact = Some(false);
        let mut third = row("gap", "third.com", 7);
        third.exact = Some(false);
        let r = report(vec![
            row("shared", "rival.com", 8),
            row("shared", "rival.com", 2),
            row("shared", "ours.com", 3),
            meta,
            third,
            row("unrelated", "third.com", 1),
            missing,
        ]);
        assert_eq!(r.sampled_keywords, 4);
        assert_eq!(r.checked_keywords, 3);
        assert_eq!(r.exact_keywords, 2);
        assert_eq!(r.matched_keywords, 2);
        assert_eq!(r.distribution.top_3, 1);
        assert_eq!(r.distribution.positions_11_20, 1);
        assert_eq!(r.top_pages[0].keywords, 2);
        assert_eq!(r.keyword_gaps, ["gap"]);
        assert!(r.competitors.iter().all(|c| c.shared_keywords == 1));
        assert_eq!(r.keywords[0].project_position, Some(3));
        assert!(r.estimated_monthly_traffic.is_none());
    }

    #[test]
    fn response_cap_does_not_change_totals() {
        let rows = (0..120)
            .map(|i| row(&format!("keyword {i:03}"), "rival.com", 5))
            .collect();
        let r = report(rows);
        assert_eq!(r.keywords.len(), 100);
        assert_eq!(r.keyword_gaps.len(), 100);
        assert_eq!(r.matched_keywords, 120);
        assert_eq!(r.distribution.positions_4_10, 120);
        assert_eq!(r.top_pages[0].keywords, 120);
    }

    #[test]
    fn empty_sample_and_domain_normalization() {
        let r = report(vec![]);
        assert_eq!(r.checked_keywords, 0);
        assert!(r.oldest_snapshot_at.is_none());
        assert!(r.keywords.is_empty());
        assert_eq!(
            normalize_domain("https://news.example.co.uk/path").unwrap(),
            "example.co.uk"
        );
        assert!(normalize_domain("co.uk").is_err());
    }
}
