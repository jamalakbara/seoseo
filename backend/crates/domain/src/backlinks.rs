//! Imported backlink evidence. No crawl or network verification is implied by an import.
use crate::{Actor, Role};
use common::{AppError, AppResult};
use db::backlinks::{BacklinkEvidence, BacklinkSnapshot};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportedBacklink {
    pub source_url: String,
    pub target_url: String,
    pub anchor: Option<String>,
    pub nofollow: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportBacklinksInput {
    /// Exact current project website; prevents a stale import after a website change.
    pub expected_domain: String,
    /// Stable lowercase dataset key, e.g. gsc-export. Diffs never cross datasets.
    pub dataset: String,
    /// Must be true: the provided rows replace the entire sample for this dataset.
    pub complete_sample: bool,
    /// At most 1000 distinct external source/target URL pairs, within the API's 2 MiB body cap.
    pub links: Vec<ImportedBacklink>,
}

fn in_domain(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|p| p.ends_with('.'))
}

fn web_url(raw: &str) -> AppResult<url::Url> {
    let mut url = url::Url::parse(raw.trim())
        .map_err(|_| AppError::Validation("Use an absolute HTTP(S) URL".into()))?;
    if crawler::UrlPolicy::default().check_url(&url).is_err() || url.port().is_some() {
        return Err(AppError::Validation(
            "Link URLs must use public HTTP(S) hosts, default ports and no credentials".into(),
        ));
    }
    url.set_fragment(None);
    if url.as_str().len() > 2048 {
        return Err(AppError::Validation(
            "Link URLs must be at most 2048 bytes".into(),
        ));
    }
    Ok(url)
}

fn normalize(domain: &str, input: &ImportBacklinksInput) -> AppResult<Vec<BacklinkEvidence>> {
    if !input.complete_sample {
        return Err(AppError::Validation("Confirm complete_sample: this import replaces this dataset's sample, including an empty sample".into()));
    }
    if input.dataset.is_empty()
        || input.dataset.len() > 40
        || !input
            .dataset
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
    {
        return Err(AppError::Validation(
            "Dataset must be 1-40 lowercase letters, digits, hyphens or underscores".into(),
        ));
    }
    if input.links.len() > 1000 {
        return Err(AppError::Validation(
            "Import at most 1000 backlink pairs".into(),
        ));
    }
    let project_root = scoring::registrable_domain(domain)
        .ok_or_else(|| AppError::Validation("Set a registrable project website first".into()))?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(input.links.len());
    for link in &input.links {
        let source = web_url(&link.source_url)?;
        let target = web_url(&link.target_url)?;
        let source_domain = source
            .host_str()
            .and_then(scoring::registrable_domain)
            .ok_or_else(|| {
                AppError::Validation("Source URL must have a registrable domain".into())
            })?;
        if !target
            .host_str()
            .is_some_and(|host| in_domain(host, domain))
            || source_domain == project_root
        {
            return Err(AppError::Validation("Target URLs must belong to the project website; source URLs must belong to a different registrable domain".into()));
        }
        if !seen.insert((source.to_string(), target.to_string())) {
            return Err(AppError::Validation(
                "Duplicate source/target pair after URL normalization; consolidate it into one row"
                    .into(),
            ));
        }
        if link
            .anchor
            .as_ref()
            .is_some_and(|a| a.chars().count() > 500)
        {
            return Err(AppError::Validation(
                "Anchor text must be at most 500 characters".into(),
            ));
        }
        rows.push(BacklinkEvidence {
            source_url: source.to_string(),
            target_url: target.to_string(),
            source_domain,
            anchor: link.anchor.as_ref().map(|a| a.trim().to_string()),
            nofollow: link.nofollow,
            state: "live".into(),
        });
    }
    Ok(rows)
}

pub async fn import(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    input: ImportBacklinksInput,
) -> AppResult<BacklinkSnapshot> {
    actor.require(Role::Admin)?;
    let project = db::projects::get(pool, actor.org_id, pid).await?;
    let domain = project
        .domain
        .as_deref()
        .ok_or_else(|| AppError::Validation("Set the project website first".into()))?;
    if domain != input.expected_domain || project.archived_at.is_some() {
        return Err(AppError::Conflict(
            "Website changed or project is archived; reload before importing".into(),
        ));
    }
    let rows = normalize(domain, &input)?;
    db::backlinks::import(
        pool,
        actor.org_id,
        pid,
        db::backlinks::ImportSnapshot {
            expected_domain: domain,
            dataset: &input.dataset,
            links: rows,
            created_by: actor.user_id,
        },
    )
    .await
}

pub async fn list(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<Vec<BacklinkSnapshot>> {
    db::projects::get(pool, actor.org_id, pid).await?;
    db::backlinks::list(pool, actor.org_id, pid).await
}

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct BacklinkQuery {
    /// all (default), live (includes new), new, lost
    pub state: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BacklinkGroup {
    pub value: String,
    pub links: usize,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct BacklinkAnchorGroup {
    pub anchor: Option<String>,
    pub links: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BacklinkSnapshotView {
    pub snapshot: BacklinkSnapshot,
    pub source: &'static str,
    pub confidence: &'static str,
    pub note: &'static str,
    /// Unknown until an independently supported scoring source is available.
    pub spam_score: Option<i32>,
    pub referring_domain_count: usize,
    /// Aggregates use current sample membership (live + new), not lost rows; at most 100 each.
    pub referring_domains: Vec<BacklinkGroup>,
    pub top_pages: Vec<BacklinkGroup>,
    pub anchors: Vec<BacklinkAnchorGroup>,
    pub items: Vec<BacklinkEvidence>,
    pub total: usize,
}

pub async fn get(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    id: Uuid,
    q: BacklinkQuery,
) -> AppResult<BacklinkSnapshotView> {
    let (snapshot, rows) = db::backlinks::get(pool, actor.org_id, pid, id).await?;
    let state = q.state.as_deref().unwrap_or("all");
    if !matches!(state, "all" | "live" | "new" | "lost") {
        return Err(AppError::Validation(
            "State must be all, live, new or lost".into(),
        ));
    }
    let mut domains = BTreeMap::new();
    let mut pages = BTreeMap::new();
    let mut anchors = BTreeMap::new();
    for r in rows.iter().filter(|r| r.state != "lost") {
        *domains.entry(r.source_domain.clone()).or_insert(0usize) += 1;
        *pages.entry(r.target_url.clone()).or_insert(0usize) += 1;
        *anchors.entry(r.anchor.clone()).or_insert(0usize) += 1;
    }
    let referring_domain_count = domains.len();
    let groups = |values: BTreeMap<String, usize>| {
        let mut out: Vec<_> = values
            .into_iter()
            .map(|(value, links)| BacklinkGroup { value, links })
            .collect();
        out.sort_by(|a, b| b.links.cmp(&a.links).then(a.value.cmp(&b.value)));
        out.truncate(100);
        out
    };
    let mut anchors: Vec<_> = anchors
        .into_iter()
        .map(|(anchor, links)| BacklinkAnchorGroup { anchor, links })
        .collect();
    anchors.sort_by(|a, b| b.links.cmp(&a.links).then(a.anchor.cmp(&b.anchor)));
    anchors.truncate(100);
    let filtered: Vec<_> = rows
        .into_iter()
        .filter(|r| state == "all" || (state == "live" && r.state != "lost") || r.state == state)
        .collect();
    let total = filtered.len();
    let items = filtered
        .into_iter()
        .skip(q.offset.unwrap_or(0))
        .take(q.limit.unwrap_or(100).clamp(1, 200))
        .collect();
    Ok(BacklinkSnapshotView {
        snapshot,
        source: "user_import",
        confidence: "relative",
        note: "Imported sample only; links are not independently verified. New/lost describe presence or absence compared with the previous import for the same dataset and website. The first import is a baseline.",
        spam_score: None,
        referring_domain_count,
        referring_domains: groups(domains),
        top_pages: groups(pages),
        anchors,
        items,
        total,
    })
}

pub async fn delete(pool: &PgPool, actor: &Actor, pid: Uuid, id: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::backlinks::delete(pool, actor.org_id, pid, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ImportBacklinksInput {
        ImportBacklinksInput {
            expected_domain: "example.com".into(),
            dataset: "export".into(),
            complete_sample: true,
            links: vec![ImportedBacklink {
                source_url: "https://other.org/page#fragment".into(),
                target_url: "https://www.example.com/path#section".into(),
                anchor: None,
                nofollow: None,
            }],
        }
    }
    #[test]
    fn normalization_and_boundaries() {
        let normalized = normalize("example.com", &input()).unwrap();
        assert_eq!(normalized[0].source_url, "https://other.org/page");
        assert_eq!(normalized[0].target_url, "https://www.example.com/path");
        assert_eq!(normalized[0].nofollow, None);
        for target in [
            "https://notexample.com/",
            "https://example.com.evil.org/",
            "https://u:p@example.com/",
            "http://127.0.0.1/",
        ] {
            let mut i = input();
            i.links[0].target_url = target.into();
            assert!(normalize("example.com", &i).is_err());
        }
        let mut i = input();
        i.links[0].source_url = "https://blog.example.com/page".into();
        assert!(normalize("example.com", &i).is_err());
        let mut i = input();
        i.complete_sample = false;
        assert!(normalize("example.com", &i).is_err());
        let mut i = input();
        i.links.push(ImportedBacklink {
            source_url: "https://other.org/page".into(),
            target_url: "https://www.example.com/path".into(),
            anchor: None,
            nofollow: Some(true),
        });
        assert!(normalize("example.com", &i).is_err());
    }
}
