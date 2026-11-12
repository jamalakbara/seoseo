use crate::internal;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct BacklinkSnapshot {
    pub id: Uuid,
    pub domain: String,
    pub dataset: String,
    /// Previous import for this dataset/domain. Kept as provenance even if it is deleted.
    pub comparison_id: Option<Uuid>,
    pub comparison_at: Option<DateTime<Utc>>,
    pub live_count: i32,
    pub new_count: i32,
    pub lost_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow, PartialEq, Eq)]
pub struct BacklinkEvidence {
    pub source_url: String,
    pub target_url: String,
    pub source_domain: String,
    pub anchor: Option<String>,
    /// Null means the imported sample did not specify the link relation.
    pub nofollow: Option<bool>,
    pub state: String,
}

macro_rules! cols {
    () => {
        "id,domain,dataset,comparison_id,comparison_at,live_count,new_count,lost_count,created_at"
    };
}

pub async fn list(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<BacklinkSnapshot>> {
    sqlx::query_as(concat!("SELECT ",cols!()," FROM backlink_snapshots WHERE project_id=$2 AND EXISTS(SELECT 1 FROM projects WHERE organization_id=$1 AND id=$2) ORDER BY created_at DESC,id DESC LIMIT 100"))
        .bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}

pub async fn get(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    id: Uuid,
) -> AppResult<(BacklinkSnapshot, Vec<BacklinkEvidence>)> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let snapshot=sqlx::query_as(concat!("SELECT ",cols!()," FROM backlink_snapshots WHERE id=$3 AND project_id=$2 AND domain=(SELECT domain::text FROM projects WHERE organization_id=$1 AND id=$2)"))
        .bind(org).bind(pid).bind(id).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("backlink snapshot"))?;
    let rows=sqlx::query_as("SELECT source_url,target_url,source_domain,anchor,nofollow,state FROM backlink_snapshot_links WHERE snapshot_id=$1 ORDER BY source_url,target_url")
        .bind(id).fetch_all(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok((snapshot, rows))
}

pub struct ImportSnapshot<'a> {
    pub expected_domain: &'a str,
    pub dataset: &'a str,
    pub links: Vec<BacklinkEvidence>,
    pub created_by: Uuid,
}

/// Serialize imports/deletions/domain changes with the project row. Baseline selection,
/// diffing, quota check and the complete snapshot commit all happen in one transaction.
pub async fn import(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    input: ImportSnapshot<'_>,
) -> AppResult<BacklinkSnapshot> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let project: crate::models::Project =
        sqlx::query_as("SELECT * FROM projects WHERE organization_id=$1 AND id=$2 FOR UPDATE")
            .bind(org)
            .bind(pid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?
            .ok_or(AppError::NotFound("project"))?;
    if project.archived_at.is_some() || project.domain.as_deref() != Some(input.expected_domain) {
        return Err(AppError::Conflict(
            "Project website changed or project was archived; reload before importing".into(),
        ));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM backlink_snapshots WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if count >= 100 {
        return Err(AppError::Conflict(
            "Snapshot limit reached (100/project); delete an old snapshot before importing".into(),
        ));
    }
    let previous: Option<BacklinkSnapshot> = sqlx::query_as(concat!("SELECT ",cols!()," FROM backlink_snapshots WHERE project_id=$1 AND domain=$2 AND dataset=$3 ORDER BY created_at DESC,id DESC LIMIT 1"))
        .bind(pid).bind(input.expected_domain).bind(input.dataset).fetch_optional(&mut *tx).await.map_err(internal)?;
    let before: Vec<BacklinkEvidence> = if let Some(prev) = &previous {
        sqlx::query_as("SELECT source_url,target_url,source_domain,anchor,nofollow,state FROM backlink_snapshot_links WHERE snapshot_id=$1 AND state<>'lost'")
            .bind(prev.id).fetch_all(&mut *tx).await.map_err(internal)?
    } else {
        Vec::new()
    };
    let rows = diff(input.links, before, previous.is_some());
    let live = rows.iter().filter(|r| r.state != "lost").count() as i32;
    let added = rows.iter().filter(|r| r.state == "new").count() as i32;
    let lost = rows.iter().filter(|r| r.state == "lost").count() as i32;
    let id = new_id();
    let snapshot = sqlx::query_as(concat!("INSERT INTO backlink_snapshots(id,project_id,domain,dataset,comparison_id,comparison_at,live_count,new_count,lost_count,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING ",cols!()))
        .bind(id).bind(pid).bind(input.expected_domain).bind(input.dataset).bind(previous.as_ref().map(|p|p.id)).bind(previous.as_ref().map(|p|p.created_at))
        .bind(live).bind(added).bind(lost).bind(input.created_by).fetch_one(&mut *tx).await.map_err(internal)?;
    if !rows.is_empty() {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO backlink_snapshot_links(snapshot_id,link_key,source_url,target_url,source_domain,anchor,nofollow,state) ",
        );
        query.push_values(&rows, |mut b, r| {
            let key = common::token::sha256_hex(&format!("{}\n{}", r.source_url, r.target_url));
            b.push_bind(id)
                .push_bind(key)
                .push_bind(&r.source_url)
                .push_bind(&r.target_url)
                .push_bind(&r.source_domain)
                .push_bind(&r.anchor)
                .push_bind(r.nofollow)
                .push_bind(&r.state);
        });
        query.build().execute(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(snapshot)
}

fn diff(
    mut current: Vec<BacklinkEvidence>,
    previous: Vec<BacklinkEvidence>,
    has_baseline: bool,
) -> Vec<BacklinkEvidence> {
    use std::collections::BTreeSet;
    let before: BTreeSet<_> = previous
        .iter()
        .map(|r| (&r.source_url, &r.target_url))
        .collect();
    let after: BTreeSet<_> = current
        .iter()
        .map(|r| (r.source_url.clone(), r.target_url.clone()))
        .collect();
    for row in &mut current {
        row.state = if has_baseline && !before.contains(&(&row.source_url, &row.target_url)) {
            "new"
        } else {
            "live"
        }
        .into();
    }
    for mut row in previous {
        if !after.contains(&(row.source_url.clone(), row.target_url.clone())) {
            row.state = "lost".into();
            current.push(row);
        }
    }
    current
}

pub async fn delete(pool: &PgPool, org: Uuid, pid: Uuid, id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT id FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE")
        .bind(org).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let result = sqlx::query("DELETE FROM backlink_snapshots WHERE project_id=$1 AND id=$2")
        .bind(pid)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("backlink snapshot"));
    }
    tx.commit().await.map_err(internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn link(name: &str) -> BacklinkEvidence {
        BacklinkEvidence {
            source_url: name.into(),
            target_url: "target".into(),
            source_domain: "source".into(),
            anchor: None,
            nofollow: None,
            state: "live".into(),
        }
    }
    #[test]
    fn baseline_membership_changes_and_metadata_updates() {
        assert_eq!(diff(vec![link("a")], vec![], false)[0].state, "live");
        let mut updated = link("a");
        updated.anchor = Some("changed".into());
        let result = diff(vec![updated, link("c")], vec![link("a"), link("b")], true);
        assert_eq!(
            result.iter().map(|r| r.state.as_str()).collect::<Vec<_>>(),
            ["live", "new", "lost"]
        );
        assert_eq!(result[0].anchor.as_deref(), Some("changed"));
        assert_eq!(diff(vec![], vec![link("a")], true)[0].state, "lost");
    }
}
