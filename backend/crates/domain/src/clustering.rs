//! Deterministic, read-only clustering of project keywords using cached SERP evidence.
use crate::{Actor, validate};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use db::competitive::Evidence;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct ClusterQuery {
    pub country: Option<String>,
    pub language: Option<String>,
    /// Minimum shared top-ten URLs for every pair in a group (2–10; default 3).
    pub min_shared: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClusterKeyword {
    pub keyword: String,
    pub provider: String,
    pub exact: bool,
    pub fetched_at: DateTime<Utc>,
    /// Distinct HTTP(S) URLs in positions 1–10, with fragments removed.
    pub urls: BTreeSet<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct KeywordCluster {
    /// Stable alphabetical representative; not a volume or business-priority recommendation.
    pub representative: String,
    pub minimum_shared_urls: usize,
    pub keywords: Vec<ClusterKeyword>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnavailableClusterKeyword {
    pub keyword: String,
    /// no_recent_snapshot | insufficient_results
    pub reason: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct KeywordClusters {
    pub country: String,
    pub language: String,
    pub min_shared: usize,
    pub scope_keywords: usize,
    pub sampled_keywords: usize,
    pub eligible_keywords: usize,
    pub clusters: Vec<KeywordCluster>,
    /// Eligible keywords that could not join a group; not proof of distinct intent.
    pub ungrouped: Vec<ClusterKeyword>,
    pub unavailable: Vec<UnavailableClusterKeyword>,
    pub warnings: Vec<String>,
}

pub async fn clusters(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    query: ClusterQuery,
) -> AppResult<KeywordClusters> {
    let project = db::projects::get(pool, actor.org_id, project_id).await?;
    let country = validate::country(query.country.as_deref().unwrap_or(&project.country_code))?;
    let language = validate::language(query.language.as_deref().unwrap_or(&project.language_code))?;
    let minimum = query.min_shared.unwrap_or(3);
    if !(2..=10).contains(&minimum) {
        return Err(AppError::Validation(
            "min_shared must be between 2 and 10".into(),
        ));
    }
    let rows =
        db::competitive::evidence(pool, actor.org_id, project_id, &country, &language).await?;
    Ok(summarize(rows, country, language, minimum, Utc::now()))
}

fn result_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

fn overlap(a: &ClusterKeyword, b: &ClusterKeyword) -> usize {
    if a.provider != b.provider || a.exact != b.exact {
        return 0;
    }
    a.urls.intersection(&b.urls).count()
}

fn summarize(
    rows: Vec<Evidence>,
    country: String,
    language: String,
    minimum: usize,
    now: DateTime<Utc>,
) -> KeywordClusters {
    let scope_keywords = rows.first().map_or(0, |r| r.scope_total as usize);
    let mut by_keyword: BTreeMap<String, Vec<Evidence>> = BTreeMap::new();
    for row in rows {
        by_keyword.entry(row.keyword.clone()).or_default().push(row);
    }
    let sampled_keywords = by_keyword.len();
    let mut unavailable = Vec::new();
    let mut groups: Vec<Vec<ClusterKeyword>> = Vec::new();
    let mut eligible_keywords = 0;
    for (keyword, rows) in by_keyword {
        let row = &rows[0];
        let (Some(provider), Some(exact), Some(at)) = (&row.provider, row.exact, row.fetched_at)
        else {
            unavailable.push(UnavailableClusterKeyword {
                keyword,
                reason: "no_recent_snapshot",
            });
            continue;
        };
        if at <= now - chrono::Duration::days(7) || at > now {
            unavailable.push(UnavailableClusterKeyword {
                keyword,
                reason: "no_recent_snapshot",
            });
            continue;
        }
        let urls = rows
            .iter()
            .filter(|r| r.position.is_some_and(|p| (1..=10).contains(&p)))
            .filter_map(|r| r.url.as_deref().and_then(result_url))
            .collect::<BTreeSet<_>>();
        if urls.len() < minimum {
            unavailable.push(UnavailableClusterKeyword {
                keyword,
                reason: "insufficient_results",
            });
            continue;
        }
        let member = ClusterKeyword {
            keyword,
            provider: provider.clone(),
            exact,
            fetched_at: at,
            urls,
        };
        eligible_keywords += 1;
        // Complete-link membership avoids the A~B, B~C, A!~C chaining failure.
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.iter().all(|other| overlap(other, &member) >= minimum))
        {
            group.push(member);
        } else {
            groups.push(vec![member]);
        }
    }
    let mut clusters = Vec::new();
    let mut ungrouped = Vec::new();
    for group in groups {
        if group.len() == 1 {
            ungrouped.extend(group);
            continue;
        }
        let minimum_shared_urls = group
            .iter()
            .enumerate()
            .flat_map(|(i, a)| group.iter().skip(i + 1).map(move |b| overlap(a, b)))
            .min()
            .unwrap_or(0);
        clusters.push(KeywordCluster {
            representative: group[0].keyword.clone(),
            minimum_shared_urls,
            keywords: group,
        });
    }
    let mut warnings = vec![
        "Heuristic page-planning suggestions from cached top-ten URLs, not proof of search intent. Review groups before changing pages.".into(),
        "Only snapshots from the last 7 days are eligible. Providers/exactness are never mixed; no provider requests are made.".into(),
    ];
    if sampled_keywords < scope_keywords {
        warnings.push("Only the first 500 project keywords alphabetically are sampled. Other keywords were not evaluated.".into());
    }
    KeywordClusters {
        country,
        language,
        min_shared: minimum,
        scope_keywords,
        sampled_keywords,
        eligible_keywords,
        clusters,
        ungrouped,
        unavailable,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(keyword: &str, urls: &[&str], provider: &str, at: DateTime<Utc>) -> Vec<Evidence> {
        urls.iter()
            .enumerate()
            .map(|(i, url)| Evidence {
                keyword: keyword.into(),
                scope_total: 4,
                provider: Some(provider.into()),
                exact: Some(true),
                fetched_at: Some(at),
                domain: Some("example.com".into()),
                position: Some(i as i32 + 1),
                url: Some((*url).into()),
                monthly_volume: None,
                volume_source: None,
            })
            .collect()
    }
    fn run(rows: Vec<Evidence>, now: DateTime<Utc>) -> KeywordClusters {
        summarize(rows, "US".into(), "en".into(), 2, now)
    }
    #[test]
    fn complete_link_prevents_chaining_and_is_order_independent() {
        let now = Utc::now();
        let a = ["https://e.com/1", "https://e.com/2"];
        let b = [
            "https://e.com/1",
            "https://e.com/2",
            "https://e.com/3",
            "https://e.com/4",
        ];
        let c = ["https://e.com/3", "https://e.com/4"];
        let mut evidence = rows("a", &a, "p", now);
        evidence.extend(rows("b", &b, "p", now));
        evidence.extend(rows("c", &c, "p", now));
        let result = run(evidence, now);
        assert_eq!(result.clusters.len(), 1);
        assert_eq!(result.clusters[0].keywords.len(), 2);
        assert_eq!(result.clusters[0].minimum_shared_urls, 2);
        assert_eq!(result.ungrouped[0].keyword, "c");
        let mut shuffled = rows("c", &c, "p", now);
        shuffled.extend(rows("b", &b, "p", now));
        shuffled.extend(rows("a", &a, "p", now));
        assert_eq!(
            serde_json::to_value(run(shuffled, now)).unwrap(),
            serde_json::to_value(result).unwrap()
        );
    }
    #[test]
    fn separates_providers_exactness_stale_and_short_results() {
        let now = Utc::now();
        let urls = ["https://e.com/1", "https://e.com/2"];
        let mut evidence = rows("a", &urls, "one", now);
        evidence.extend(rows("b", &urls, "two", now));
        let mut aggregate = rows("c", &urls, "one", now);
        for row in &mut aggregate {
            row.exact = Some(false);
        }
        evidence.extend(aggregate);
        evidence.extend(rows("old", &urls, "one", now - chrono::Duration::days(8)));
        evidence.extend(rows(
            "short",
            &["https://e.com/1#one", "https://e.com/1#two"],
            "one",
            now,
        ));
        evidence.extend(
            rows("deep", &["https://e.com/1", "https://e.com/2"], "one", now)
                .into_iter()
                .map(|mut r| {
                    r.position = Some(11);
                    r
                }),
        );
        let result = run(evidence, now);
        assert!(result.clusters.is_empty());
        assert_eq!(result.ungrouped.len(), 3);
        assert_eq!(result.unavailable.len(), 3);
    }
    #[test]
    fn url_identity_is_conservative_and_ignores_duplicates() {
        assert_eq!(
            result_url("https://EXAMPLE.com:443/a#x"),
            Some("https://example.com/a".into())
        );
        assert_ne!(
            result_url("https://e.com/a?q=1"),
            result_url("https://e.com/a?q=2")
        );
        assert_ne!(result_url("http://e.com/a"), result_url("https://e.com/a"));
        for raw in [
            "javascript:alert(1)",
            "https://user:pass@e.com/",
            "/relative",
            "invalid",
        ] {
            assert!(result_url(raw).is_none());
        }
    }
    #[test]
    fn empty_scope_has_no_invented_groups() {
        let result = run(vec![], Utc::now());
        assert_eq!(result.scope_keywords, 0);
        assert_eq!(result.sampled_keywords, 0);
        assert!(result.clusters.is_empty());
    }
}
