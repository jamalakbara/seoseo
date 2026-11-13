use crate::{Actor, validate};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use db::competitive::Evidence;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct CannibalizationQuery {
    pub country: Option<String>,
    pub language: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CompetingRankedPage {
    pub url: String,
    pub position: i32,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CannibalizationCandidate {
    pub keyword: String,
    pub provider: String,
    pub exact: bool,
    pub fetched_at: DateTime<Utc>,
    pub pages: Vec<CompetingRankedPage>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CannibalizationFlags {
    pub domain: String,
    pub country: String,
    pub language: String,
    pub scope_keywords: usize,
    pub sampled_keywords: usize,
    pub checked_keywords: usize,
    pub matched_keywords: usize,
    pub flagged_keywords: usize,
    pub items: Vec<CannibalizationCandidate>,
    pub warnings: Vec<String>,
}
pub async fn flags(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    q: CannibalizationQuery,
) -> AppResult<CannibalizationFlags> {
    let p = db::projects::get(pool, a.org_id, pid).await?;
    let domain = crate::competitive::normalize_domain(
        p.domain
            .as_deref()
            .ok_or_else(|| AppError::Validation("set the project website first".into()))?,
    )?;
    let country = validate::country(q.country.as_deref().unwrap_or(&p.country_code))?;
    let language = validate::language(q.language.as_deref().unwrap_or(&p.language_code))?;
    if !(1..=100).contains(&q.limit.unwrap_or(25)) {
        return Err(AppError::Validation("limit must be 1-100".into()));
    }
    let rows = db::competitive::evidence(pool, a.org_id, pid, &country, &language).await?;
    let mut result = summarize(domain, country, language, rows);
    result.items = result
        .items
        .into_iter()
        .skip(q.offset.unwrap_or(0))
        .take(q.limit.unwrap_or(25))
        .collect();
    Ok(result)
}
fn summarize(
    domain: String,
    country: String,
    language: String,
    rows: Vec<Evidence>,
) -> CannibalizationFlags {
    let scope_keywords = rows.first().map_or(0, |r| r.scope_total as usize);
    let mut sampled = BTreeSet::new();
    let mut checked = BTreeSet::new();
    let mut groups: BTreeMap<String, Vec<Evidence>> = BTreeMap::new();
    for row in rows {
        sampled.insert(row.keyword.clone());
        if row.fetched_at.is_some() {
            checked.insert(row.keyword.clone());
        }
        groups.entry(row.keyword.clone()).or_default().push(row);
    }
    let mut items = Vec::new();
    let mut matched_keywords = 0;
    for (keyword, rows) in groups {
        let mut pages: BTreeMap<String, i32> = BTreeMap::new();
        for row in &rows {
            let (Some(raw), Some(pos)) = (&row.url, row.position) else {
                continue;
            };
            if !(1..=30).contains(&pos) {
                continue;
            }
            let Some(url) = url::Url::parse(raw)
                .ok()
                .filter(|u| u.username().is_empty() && u.password().is_none())
                .and_then(|u| crawler::urls::normalize(&u))
            else {
                continue;
            };
            if url
                .host_str()
                .and_then(scoring::registrable_domain)
                .as_deref()
                != Some(domain.as_str())
            {
                continue;
            }
            pages
                .entry(url.to_string())
                .and_modify(|p| *p = (*p).min(pos))
                .or_insert(pos);
        }
        if !pages.is_empty() {
            matched_keywords += 1;
        }
        if pages.len() < 2 {
            continue;
        }
        let row = &rows[0];
        let (Some(provider), Some(exact), Some(fetched_at)) =
            (&row.provider, row.exact, row.fetched_at)
        else {
            continue;
        };
        let mut pages: Vec<_> = pages
            .into_iter()
            .map(|(url, position)| CompetingRankedPage { url, position })
            .collect();
        pages.sort_by(|a, b| a.position.cmp(&b.position).then(a.url.cmp(&b.url)));
        items.push(CannibalizationCandidate {
            keyword,
            provider: provider.clone(),
            exact,
            fetched_at,
            pages,
        });
    }
    let mut warnings=vec!["Multiple URLs for one keyword are review candidates, not proof of harmful cannibalization. Several useful results from one site can be legitimate.".into(),"Uses the latest cached results within 30 days, positions 1-30, for up to 500 project keywords. No new searches; aggregated providers do not establish Google rankings. Subdomains roll up to the project registrable domain.".into()];
    if sampled.len() < scope_keywords {
        warnings.push("Only the first 500 keywords alphabetically were sampled; other keywords remain unchecked.".into());
    }
    CannibalizationFlags {
        domain,
        country,
        language,
        scope_keywords,
        sampled_keywords: sampled.len(),
        checked_keywords: checked.len(),
        matched_keywords,
        flagged_keywords: items.len(),
        items,
        warnings,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(keyword: &str, url: &str, pos: i32) -> Evidence {
        Evidence {
            keyword: keyword.into(),
            scope_total: 2,
            provider: Some("fixture".into()),
            exact: Some(false),
            fetched_at: Some(Utc::now()),
            domain: Some("example.com".into()),
            position: Some(pos),
            url: Some(url.into()),
            monthly_volume: None,
            volume_source: None,
        }
    }
    #[test]
    fn flags_distinct_urls_not_duplicate_fragments_or_foreign_hosts() {
        let r = summarize(
            "example.com".into(),
            "US".into(),
            "en".into(),
            vec![
                row("one", "https://example.com/a#one", 1),
                row("one", "https://example.com/a#two", 2),
                row("one", "https://other.example/b", 3),
                row("two", "https://example.com/a", 2),
                row("two", "https://blog.example.com/b", 5),
            ],
        );
        assert_eq!(r.flagged_keywords, 1);
        assert_eq!(r.items[0].keyword, "two");
        assert!(!r.items[0].exact);
        assert_eq!(r.matched_keywords, 2);
    }
}
