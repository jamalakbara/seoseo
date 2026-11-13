use crate::Actor;
use common::{AppError, AppResult};
use db::audit_compare::{ComparisonEvidence, ComparisonPage};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
pub struct CompareAuditQuery {
    pub baseline_id: Uuid,
    /// changes (default) | all | changed | unchanged | only_in_baseline | only_in_current
    pub kind: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditPageChange {
    pub url: String,
    pub baseline_page_id: Option<Uuid>,
    pub current_page_id: Option<Uuid>,
    pub observation: &'static str,
    pub changed_fields: Vec<String>,
    pub newly_observed_issues: Vec<String>,
    pub no_longer_observed_issues: Vec<String>,
    pub unconfirmed_absent_issues: Vec<String>,
    pub content_comparable: bool,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct AuditComparisonCounts {
    pub changed: usize,
    pub unchanged: usize,
    pub only_in_baseline: usize,
    pub only_in_current: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditComparison {
    pub baseline: db::audits::Audit,
    pub current: db::audits::Audit,
    pub health_score_change: Option<i32>,
    pub counts: AuditComparisonCounts,
    pub total_matching: usize,
    pub pages: Vec<AuditPageChange>,
    pub warnings: Vec<String>,
}

pub async fn compare(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    current: Uuid,
    query: CompareAuditQuery,
) -> AppResult<AuditComparison> {
    crate::audits::get(pool, actor, pid, current).await?;
    crate::audits::get(pool, actor, pid, query.baseline_id).await?;
    if current == query.baseline_id {
        return Err(AppError::Validation("choose two different audits".into()));
    }
    let kind = query.kind.as_deref().unwrap_or("changes");
    if ![
        "changes",
        "all",
        "changed",
        "unchanged",
        "only_in_baseline",
        "only_in_current",
    ]
    .contains(&kind)
    {
        return Err(AppError::Validation("unknown comparison kind".into()));
    }
    if !(1..=500).contains(&query.limit.unwrap_or(50)) {
        return Err(AppError::Validation(
            "limit must be between 1 and 500".into(),
        ));
    }
    let evidence =
        db::audit_compare::evidence(pool, actor.org_id, pid, query.baseline_id, current).await?;
    let normalize = |s: &str| {
        url::Url::parse(s)
            .ok()
            .and_then(|u| crawler::urls::normalize(&u))
    };
    if normalize(&evidence.baseline.start_url).is_none()
        || normalize(&evidence.baseline.start_url) != normalize(&evidence.current.start_url)
    {
        return Err(AppError::Validation(
            "compare audits with the same normalized start URL".into(),
        ));
    }
    let mut result = summarize(evidence);
    result.pages.retain(|p| {
        kind == "all"
            || (kind == "changes" && p.observation != "unchanged")
            || p.observation == kind
    });
    result.total_matching = result.pages.len();
    result.pages = result
        .pages
        .into_iter()
        .skip(query.offset.unwrap_or(0))
        .take(query.limit.unwrap_or(50))
        .collect();
    Ok(result)
}

fn change(
    before: Option<&ComparisonPage>,
    after: Option<&ComparisonPage>,
    method_same: bool,
) -> AuditPageChange {
    let comparable =
        method_same && before.is_some_and(|p| p.comparable) && after.is_some_and(|p| p.comparable);
    let before_issues: BTreeSet<_> = before
        .into_iter()
        .flat_map(|p| p.issues.iter().cloned())
        .collect();
    let after_issues: BTreeSet<_> = after
        .into_iter()
        .flat_map(|p| p.issues.iter().cloned())
        .collect();
    let missing: Vec<_> = before_issues.difference(&after_issues).cloned().collect();
    let newly_observed_issues: Vec<_> = after_issues.difference(&before_issues).cloned().collect();
    let mut changed_fields = Vec::new();
    if let (Some(b), Some(a)) = (before, after)
        && let Some(values) = b.values.as_object()
    {
        for (field, value) in values {
            if (comparable
                || ["status_code", "fetch_class", "redirected"].contains(&field.as_str()))
                && a.values.get(field) != Some(value)
            {
                changed_fields.push(field.clone());
            }
        }
    }
    let observation = match (before, after) {
        (None, Some(_)) => "only_in_current",
        (Some(_), None) => "only_in_baseline",
        _ if !changed_fields.is_empty()
            || !newly_observed_issues.is_empty()
            || !missing.is_empty() =>
        {
            "changed"
        }
        _ => "unchanged",
    };
    AuditPageChange {
        url: before.or(after).unwrap().url.clone(),
        baseline_page_id: before.map(|p| p.id),
        current_page_id: after.map(|p| p.id),
        observation,
        changed_fields,
        newly_observed_issues,
        no_longer_observed_issues: if comparable { missing.clone() } else { vec![] },
        unconfirmed_absent_issues: if comparable { vec![] } else { missing },
        content_comparable: comparable,
    }
}

fn summarize(e: ComparisonEvidence) -> AuditComparison {
    let method = |a: &db::audits::Audit| {
        (
            a.config["render_js"].as_bool().unwrap_or(false),
            a.config["lighthouse"].as_bool().unwrap_or(false),
        )
    };
    let method_same = method(&e.baseline) == method(&e.current);
    let mut warnings=vec!["Only observed audit evidence is compared. A URL absent from a crawl is not proven removed; an absent issue is not proof of a permanent fix. Site-wide issue counts may change with crawl coverage or rules.".into()];
    if e.baseline.config != e.current.config {
        warnings.push(
            "Crawl settings differ; coverage and findings may not be directly comparable.".into(),
        );
    }
    if !method_same {
        warnings.push("Rendering/Lighthouse settings differ: content and issue absences are treated as unconfirmed.".into());
    }
    if [&e.baseline, &e.current]
        .iter()
        .any(|a| a.stop_reason.is_some() || a.pages_failed > 0)
    {
        warnings.push("At least one audit stopped early or contains failed fetches. Missing URLs and findings can be coverage gaps.".into());
    }
    let before: BTreeMap<_, _> = e.before.iter().map(|p| (p.url.as_str(), p)).collect();
    let after: BTreeMap<_, _> = e.after.iter().map(|p| (p.url.as_str(), p)).collect();
    let urls: BTreeSet<_> = before.keys().chain(after.keys()).copied().collect();
    let mut counts = AuditComparisonCounts::default();
    let pages = urls
        .into_iter()
        .map(|u| {
            let p = change(before.get(u).copied(), after.get(u).copied(), method_same);
            match p.observation {
                "changed" => counts.changed += 1,
                "unchanged" => counts.unchanged += 1,
                "only_in_baseline" => counts.only_in_baseline += 1,
                _ => counts.only_in_current += 1,
            }
            p
        })
        .collect();
    AuditComparison {
        health_score_change: e
            .baseline
            .health_score
            .zip(e.current.health_score)
            .map(|(a, b)| b - a),
        baseline: e.baseline,
        current: e.current,
        counts,
        pages,
        total_matching: 0,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(ok: bool, issue: bool) -> ComparisonPage {
        ComparisonPage {
            id: Uuid::new_v4(),
            url: "https://example.com/a".into(),
            comparable: ok,
            values: serde_json::json!({"title": if ok {"hash"} else {""},"status_code":if ok {200}else{503}}),
            issues: if issue {
                vec!["missing-title".into()]
            } else {
                vec![]
            },
        }
    }
    #[test]
    fn missing_or_failed_pages_do_not_resolve_issues() {
        let b = page(true, true);
        let a = page(false, false);
        for result in [
            change(Some(&b), None, true),
            change(Some(&b), Some(&a), true),
        ] {
            assert!(result.no_longer_observed_issues.is_empty());
            assert_eq!(result.unconfirmed_absent_issues, vec!["missing-title"]);
        }
    }
    #[test]
    fn comparable_observations_and_method_changes_remain_distinct() {
        let b = page(true, true);
        let a = page(true, false);
        let result = change(Some(&b), Some(&a), true);
        assert_eq!(result.no_longer_observed_issues, vec!["missing-title"]);
        assert!(result.changed_fields.is_empty());
        assert!(
            !change(Some(&b), Some(&a), false)
                .unconfirmed_absent_issues
                .is_empty()
        );
        assert_eq!(change(None, Some(&a), true).observation, "only_in_current");
        assert_eq!(change(Some(&a), Some(&a), true).observation, "unchanged");
    }
}
