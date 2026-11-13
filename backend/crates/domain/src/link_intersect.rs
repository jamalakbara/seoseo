use crate::Actor;
use common::{AppError, AppResult};
use db::link_intersect::{IntersectSelection, LinkIntersectScope, LinkProspect};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::BTreeSet;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkIntersectInput {
    pub competitor_project_ids: Vec<Uuid>,
    pub min_competitors: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct LinkIntersect {
    pub project_id: Uuid,
    pub domain: String,
    pub min_competitors: i64,
    pub scopes: Vec<LinkIntersectScope>,
    pub total_matching: i64,
    pub items: Vec<LinkProspect>,
    pub warnings: Vec<String>,
}

pub async fn inspect(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    input: LinkIntersectInput,
) -> AppResult<LinkIntersect> {
    let own = db::projects::get(pool, a.org_id, pid).await?;
    if input.competitor_project_ids.is_empty()
        || input.competitor_project_ids.len() > 5
        || input.competitor_project_ids.contains(&pid)
        || input
            .competitor_project_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != input.competitor_project_ids.len()
    {
        return Err(AppError::Validation(
            "select 1-5 distinct competitor projects, excluding this project".into(),
        ));
    }
    let domain = own
        .domain
        .ok_or_else(|| AppError::Validation("set the project website first".into()))?;
    let current_root = crate::competitive::normalize_domain(&domain)?;
    let mut roots = BTreeSet::from([current_root.clone()]);
    let mut identities = vec![(pid, domain.clone())];
    for id in &input.competitor_project_ids {
        let p = db::projects::get(pool, a.org_id, *id).await?;
        let domain = p
            .domain
            .ok_or_else(|| AppError::Validation("each selected project needs a website".into()))?;
        if !roots.insert(crate::competitive::normalize_domain(&domain)?) {
            return Err(AppError::Validation(
                "select distinct registrable competitor websites".into(),
            ));
        }
        identities.push((*id, domain));
    }
    let minimum = input.min_competitors.unwrap_or(1);
    let limit = input.limit.unwrap_or(25);
    let offset = input.offset.unwrap_or(0);
    if minimum < 1
        || minimum > input.competitor_project_ids.len() as i64
        || !(1..=100).contains(&limit)
        || offset < 0
    {
        return Err(AppError::Validation(
            "minimum must fit the selected competitors; limit 1-100, offset nonnegative".into(),
        ));
    }
    let e = db::link_intersect::evidence(
        pool,
        IntersectSelection {
            org: a.org_id,
            own: pid,
            current_root,
            identities,
            minimum,
            limit,
            offset,
        },
    )
    .await?;
    let mut warnings=vec!["Prospects are source domains observed linking to selected competitor samples but not observed in this project's selected samples. This does not prove your website has no link from them.".into(),"Uses the latest snapshot per dataset/source for each current website, bounded to ten latest datasets per project. Imported/unconfirmed evidence is not independently verified; missing and lost sample membership is not proof of live link loss.".into(),"Read-only: no external discovery, contact lookup, outreach or messages are performed. Review source pages and relevance before contacting anyone.".into()];
    if e.scopes.iter().any(|s| s.sampled_datasets == 0) {
        warnings.push("At least one selected website has no backlink snapshot evidence; its absence is unknown.".into());
    }
    if e.scopes
        .iter()
        .any(|s| s.sampled_datasets < s.available_datasets)
    {
        warnings.push("Some datasets were omitted by the ten-dataset sample cap; inspect scope metadata before using the results.".into());
    }
    Ok(LinkIntersect {
        project_id: pid,
        domain,
        min_competitors: minimum,
        scopes: e.scopes,
        total_matching: e.total,
        items: e.items,
        warnings,
    })
}
