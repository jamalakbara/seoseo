//! Read-only internal link visualization and conservative editorial suggestions.
use crate::Actor;
use common::{AppError, AppResult};
use db::audits::{InternalLinkEdge, InternalLinkEvidence, InternalLinkNode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::BTreeSet;
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct InternalLinkQuery {
    /// First pages alphabetically to visualize (10–500; default 200).
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InternalLinkSuggestion {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub shared_terms: Vec<String>,
    pub reason: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InternalLinkGraph {
    pub audit_id: Uuid,
    pub total_pages: i64,
    pub sampled_pages: usize,
    pub total_sample_edges: i64,
    pub edges_truncated: bool,
    pub nodes: Vec<InternalLinkNode>,
    pub edges: Vec<InternalLinkEdge>,
    /// At most three sources per target, fifty suggestions overall. Manual review required.
    pub suggestions: Vec<InternalLinkSuggestion>,
    pub warnings: Vec<String>,
}

pub async fn graph(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
    query: InternalLinkQuery,
) -> AppResult<InternalLinkGraph> {
    // Authorize before validating the requested sample size.
    crate::audits::get(pool, actor, project_id, audit_id).await?;
    let limit = query.limit.unwrap_or(200);
    if !(10..=500).contains(&limit) {
        return Err(AppError::Validation(
            "limit must be between 10 and 500".into(),
        ));
    }
    let evidence =
        db::audits::internal_link_evidence(pool, actor.org_id, project_id, audit_id, limit).await?;
    Ok(summarize(evidence))
}

fn normalized(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    crawler::urls::normalize(&url)
}

fn eligible(node: &InternalLinkNode, scope: &crawler::urls::Scope) -> bool {
    let Some(url) = normalized(&node.url) else {
        return false;
    };
    node.status_code == Some(200)
        && node.fetch_class == "html"
        && node.is_indexable
        && !node.redirected
        && scope.contains(&url)
        && [&node.canonical_url, &node.header_canonical_url]
            .into_iter()
            .all(|canonical| {
                canonical
                    .as_deref()
                    .is_none_or(|c| normalized(c).as_ref() == Some(&url))
            })
}

fn terms(node: &InternalLinkNode) -> BTreeSet<String> {
    // A lexical heuristic only; short/common words are not evidence of a useful link.
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "from", "this", "that", "your", "our", "you", "are", "how",
        "what", "why", "when", "where", "home", "page", "pages", "guide", "best", "more", "about",
        "into", "tips",
    ];
    format!(
        "{} {}",
        node.title.as_deref().unwrap_or(""),
        node.h1.as_deref().unwrap_or("")
    )
    .to_lowercase()
    .split(|c: char| !c.is_alphanumeric())
    .filter(|term| term.chars().count() >= 3 && !STOP.contains(term))
    .map(str::to_owned)
    .collect()
}

fn summarize(mut evidence: InternalLinkEvidence) -> InternalLinkGraph {
    let start = normalized(&evidence.audit.start_url);
    if let Some(url) = &start {
        let scope = crawler::urls::Scope::new(url);
        for node in &mut evidence.nodes {
            node.suggestion_eligible = eligible(node, &scope);
        }
    }
    let truncated = evidence.total_sample_edges > evidence.edges.len() as i64;
    let mut warnings = vec![
        "Counts describe this stored crawl, not the entire website. Zero observed inlinks does not prove a page is orphaned.".into(),
        "Node counts use the full stored audit; graph edges and suggestions use only the selected pages. Self-links are excluded. Dashed edges carry only nofollow links.".into(),
        "Suggestions require two shared title/H1 words and at most one observed followed source. They are lexical hints for editorial review, not automatic fixes.".into(),
    ];
    if evidence.total_pages > evidence.nodes.len() as i64 {
        warnings.push("Page sample is capped; only the first requested pages alphabetically are visualized and considered for suggestions.".into());
    }
    if evidence.audit.stop_reason.is_some() || evidence.audit.pages_failed > 0 {
        warnings.push(
            "The audit stopped early or contains failed fetches; link coverage is incomplete."
                .into(),
        );
    }
    if truncated {
        warnings.push("Graph is capped at 5000 edges. Suggestions are disabled because omitted edges could already connect a proposed pair. Reduce the page sample.".into());
    }
    let mut suggestions = Vec::new();
    if !truncated {
        let existing: BTreeSet<_> = evidence
            .edges
            .iter()
            .map(|e| (e.source_id, e.target_id))
            .collect();
        let tokens: Vec<_> = evidence.nodes.iter().map(terms).collect();
        let mut targets: Vec<_> = evidence
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                n.suggestion_eligible
                    && n.followed_incoming_sources <= 1
                    && normalized(&n.url) != start
            })
            .collect();
        targets.sort_by(|(_, a), (_, b)| {
            a.followed_incoming_sources
                .cmp(&b.followed_incoming_sources)
                .then(a.url.cmp(&b.url))
        });
        for (ti, target) in targets {
            let mut candidates = Vec::new();
            for (si, source) in evidence.nodes.iter().enumerate() {
                if !source.suggestion_eligible
                    || source.id == target.id
                    || existing.contains(&(source.id, target.id))
                {
                    continue;
                }
                let shared: Vec<_> = tokens[si].intersection(&tokens[ti]).cloned().collect();
                if shared.len() >= 2 {
                    candidates.push((source, shared));
                }
            }
            candidates.sort_by(|(a, at), (b, bt)| bt.len().cmp(&at.len()).then(a.url.cmp(&b.url)));
            for (source, shared_terms) in candidates.into_iter().take(3) {
                if suggestions.len() == 50 {
                    break;
                }
                suggestions.push(InternalLinkSuggestion { source_id: source.id, target_id: target.id, shared_terms, reason: "Low observed followed inlinks and shared title/H1 terms; review context before adding a link." });
            }
            if suggestions.len() == 50 {
                break;
            }
        }
    }
    InternalLinkGraph {
        audit_id: evidence.audit.id,
        total_pages: evidence.total_pages,
        sampled_pages: evidence.nodes.len(),
        total_sample_edges: evidence.total_sample_edges,
        edges_truncated: truncated,
        nodes: evidence.nodes,
        edges: evidence.edges,
        suggestions,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(path: &str) -> InternalLinkNode {
        InternalLinkNode {
            id: Uuid::new_v4(),
            url: format!("https://example.com/{path}"),
            title: Some("Technical SEO crawling".into()),
            h1: None,
            status_code: Some(200),
            fetch_class: "html".into(),
            is_indexable: true,
            depth: 1,
            redirected: false,
            canonical_url: None,
            header_canonical_url: None,
            incoming_sources: 0,
            followed_incoming_sources: 0,
            outgoing_targets: 0,
            unfetched_targets: 0,
            suggestion_eligible: false,
        }
    }
    fn evidence(
        nodes: Vec<InternalLinkNode>,
        edges: Vec<InternalLinkEdge>,
    ) -> InternalLinkEvidence {
        InternalLinkEvidence {
            total_pages: nodes.len() as i64,
            total_sample_edges: edges.len() as i64,
            nodes,
            edges,
            audit: db::audits::Audit {
                id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                started_by: None,
                job_id: None,
                start_url: "https://example.com/".into(),
                status: "completed".into(),
                phase: "done".into(),
                config: serde_json::json!({}),
                pages_crawled: 2,
                pages_total: 2,
                pages_failed: 0,
                health_score: None,
                issue_counts: serde_json::json!({}),
                stop_reason: None,
                error: None,
                started_at: None,
                finished_at: None,
                created_at: chrono::Utc::now(),
            },
        }
    }
    #[test]
    fn suggestions_skip_existing_nofollow_and_self_pairs() {
        let a = node("a");
        let b = node("b");
        let edge = InternalLinkEdge {
            source_id: a.id,
            target_id: b.id,
            occurrences: 2,
            followed: false,
            anchor_sample: None,
        };
        let result = summarize(evidence(vec![a.clone(), b.clone()], vec![edge]));
        assert_eq!(result.suggestions.len(), 1);
        assert_eq!(
            (
                result.suggestions[0].source_id,
                result.suggestions[0].target_id
            ),
            (b.id, a.id)
        );
    }
    #[test]
    fn rejects_redirect_noncanonical_nonindexable_and_out_of_scope_sources() {
        let scope = crawler::urls::Scope::new(&Url::parse("https://example.com/").unwrap());
        let n = node("a");
        assert!(eligible(&n, &scope));
        let mut variants = vec![n.clone(); 7];
        variants[0].redirected = true;
        variants[1].is_indexable = false;
        variants[2].status_code = Some(404);
        variants[3].canonical_url = Some("https://example.com/other".into());
        variants[4].header_canonical_url = Some("javascript:bad".into());
        variants[5].url = "https://other.example/a".into();
        variants[6].fetch_class = "error".into();
        assert!(variants.iter().all(|n| !eligible(n, &scope)));
    }
    #[test]
    fn insufficient_overlap_and_well_linked_targets_are_not_suggested() {
        let mut a = node("a");
        a.title = Some("Home page best guide".into());
        let mut b = node("b");
        b.followed_incoming_sources = 3;
        let result = summarize(evidence(vec![a, b], vec![]));
        assert!(result.suggestions.is_empty());
    }
    #[test]
    fn truncated_edges_disable_suggestions_and_home_is_not_a_target() {
        let mut e = evidence(vec![node(""), node("a")], vec![]);
        let result = summarize(evidence(e.nodes.clone(), vec![]));
        assert_eq!(result.suggestions.len(), 1);
        assert_eq!(result.suggestions[0].target_id, e.nodes[1].id);
        e.total_sample_edges = 5001;
        assert!(summarize(e).suggestions.is_empty());
    }
}
