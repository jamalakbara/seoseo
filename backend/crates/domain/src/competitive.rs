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
    pub limit: Option<usize>,
    pub offset: Option<usize>,
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
    pub monthly_search_volume: Option<i64>,
    pub volume_source: Option<String>,
    pub estimated_sample_clicks: Option<f64>,
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
    /// Rounded observed-sample clicks only when every matched keyword has eligible exact rank/volume evidence; never total domain traffic.
    pub estimated_monthly_traffic: Option<i64>,
    pub traffic_keywords_with_volume: usize,
    pub partial_sample_estimated_clicks: Option<f64>,
    pub traffic_method: &'static str,
    pub scope_keywords: usize,
    pub sampled_keywords: usize,
    pub checked_keywords: usize,
    pub exact_keywords: usize,
    pub matched_keywords: usize,
    pub oldest_snapshot_at: Option<DateTime<Utc>>,
    pub newest_snapshot_at: Option<DateTime<Utc>>,
    pub distribution: PositionDistribution,
    /// Paginated best results per keyword; totals/distribution use the full bounded sample.
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
    let mut result = summarize(domain, project_domain, country, language, authority, rows);
    result.keywords = result
        .keywords
        .into_iter()
        .skip(q.offset.unwrap_or(0))
        .take(q.limit.unwrap_or(100).clamp(1, 500))
        .collect();
    Ok(result)
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
            monthly_search_volume: r.monthly_volume,
            volume_source: r.volume_source.clone(),
            estimated_sample_clicks: r
                .monthly_volume
                .filter(|_| r.exact == Some(true))
                .map(|v| v as f64 * ctr(position)),
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
    let traffic_keywords_with_volume = keywords
        .iter()
        .filter(|k| k.estimated_sample_clicks.is_some())
        .count();
    let partial_sample_estimated_clicks = (traffic_keywords_with_volume > 0).then(|| {
        keywords
            .iter()
            .filter_map(|k| k.estimated_sample_clicks)
            .sum::<f64>()
    });
    let estimated_monthly_traffic =
        if matched_keywords > 0 && traffic_keywords_with_volume == matched_keywords {
            partial_sample_estimated_clicks.map(|n| n.round() as i64)
        } else {
            None
        };
    keywords.sort_by(|a, b| (a.position, &a.keyword).cmp(&(b.position, &b.keyword)));
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
    let mut warnings = vec!["Local sample of this project's saved, researched, rank-tracked and volume-imported keywords; not an exhaustive domain index. Latest snapshot per keyword in the last 30 days, positions 1–30 only. Absence does not prove a domain does not rank.".into()];
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
        estimated_monthly_traffic,
        traffic_keywords_with_volume,
        partial_sample_estimated_clicks,
        traffic_method: "Observed sample only: imported monthly search volume × heuristic CTR = 0.28 / position^1.5. Requires an exact snapshot; Wikipedia pageviews are never search volume. Full sample estimate stays null with incomplete eligible volume coverage.",
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

fn ctr(position: i32) -> f64 {
    0.28 / (position.max(1) as f64).powf(1.5)
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompetitiveVolumeRow {
    pub keyword: String,
    pub monthly_search_volume: i64,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompetitiveVolumeInput {
    pub country: Option<String>,
    pub language: Option<String>,
    pub source_label: String,
    pub rows: Vec<CompetitiveVolumeRow>,
}
pub async fn volumes(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    input: CompetitiveVolumeInput,
) -> AppResult<serde_json::Value> {
    a.require(crate::Role::Admin)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    if input.rows.is_empty() || input.rows.len() > 1000 {
        return Err(AppError::Validation(
            "Import 1-1000 monthly search-volume rows".into(),
        ));
    }
    let source = validate::non_empty("volume source label", &input.source_label, 100)?;
    let country = validate::country(input.country.as_deref().unwrap_or(&p.country_code))?;
    let language = validate::language(input.language.as_deref().unwrap_or(&p.language_code))?;
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for r in input.rows {
        let keyword = scoring::normalize_keyword(&r.keyword);
        if keyword.is_empty()
            || keyword.chars().count() > 120
            || !(0..=1_000_000_000_000).contains(&r.monthly_search_volume)
            || !seen.insert(keyword.clone())
        {
            return Err(AppError::Validation(
                "Use unique bounded keywords and monthly search volumes from 0 to 1 trillion"
                    .into(),
            ));
        }
        rows.push(db::competitive::VolumeInput {
            keyword,
            volume: r.monthly_search_volume,
        });
    }
    db::competitive::import_volumes(pool, a.org_id, pid, &country, &language, &source, &rows)
        .await?;
    Ok(
        serde_json::json!({"imported":rows.len(),"source":"user_reported_monthly_search_volume","confidence":"estimated"}),
    )
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
            monthly_volume: None,
            volume_source: None,
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
        assert_eq!(r.keywords.len(), 120);
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
