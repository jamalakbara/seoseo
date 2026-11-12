use crate::{internal, models::Project};
use common::{AppError, AppResult};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectContextRow {
    pub revision: i64,
    pub sections: Value,
    pub competitors: Value,
    pub key_pages: Value,
    pub research_log: Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
) -> AppResult<Option<ProjectContextRow>> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND organization_id = $2)",
    )
    .bind(project_id)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    if !exists {
        return Err(AppError::NotFound("project"));
    }
    sqlx::query_as(
        "SELECT c.revision, c.sections, c.competitors, c.key_pages, c.research_log, c.updated_at
         FROM project_contexts c
         JOIN projects p ON p.id = c.project_id
         WHERE c.project_id = $1 AND p.organization_id = $2",
    )
    .bind(project_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)
}

pub struct ContextUpdate {
    pub expected_revision: i64,
    pub expected_domain: Option<String>,
    pub sections: Value,
    pub competitors: Value,
    pub key_pages: Value,
    pub research_log: Value,
}

/// Replace the context document with compare-and-swap semantics. Revision 0 creates it.
pub async fn put(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    input: ContextUpdate,
) -> AppResult<ProjectContextRow> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let project: Project =
        sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 FOR UPDATE")
            .bind(project_id)
            .bind(org_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?
            .ok_or(AppError::NotFound("project"))?;
    if project.archived_at.is_some() {
        return Err(AppError::Conflict(
            "restore the project before editing memory".into(),
        ));
    }
    if input
        .expected_domain
        .as_deref()
        .is_some_and(|d| project.domain.as_deref() != Some(d))
    {
        return Err(AppError::Conflict(
            "website changed; reload onboarding before saving".into(),
        ));
    }
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM project_contexts WHERE project_id=$1")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?
            .unwrap_or(0);
    if revision != input.expected_revision {
        return Err(AppError::Conflict(
            "project memory changed since it was loaded; reload it and apply the edit again".into(),
        ));
    }
    let row = sqlx::query_as(
        "INSERT INTO project_contexts
             (project_id, revision, sections, competitors, key_pages, research_log)
         SELECT p.id, 1, $4, $5, $6, $7
         FROM projects p
         WHERE p.id = $1 AND p.organization_id = $2
         ON CONFLICT (project_id) DO UPDATE SET
             revision = project_contexts.revision + 1,
             sections = EXCLUDED.sections,
             competitors = EXCLUDED.competitors,
             key_pages = EXCLUDED.key_pages,
             research_log = EXCLUDED.research_log,
             updated_at = now()
         WHERE project_contexts.revision = $3
         RETURNING revision, sections, competitors, key_pages, research_log, updated_at",
    )
    .bind(project_id)
    .bind(org_id)
    .bind(input.expected_revision)
    .bind(input.sections)
    .bind(input.competitors)
    .bind(input.key_pages)
    .bind(input.research_log)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    if let Some(row) = row {
        tx.commit().await.map_err(internal)?;
        return Ok(row);
    }

    // Distinguish a stale document from an inaccessible project without leaking tenants.
    let _: Project = crate::projects::get(pool, org_id, project_id).await?;
    Err(AppError::Conflict(
        "project memory changed since it was loaded; reload it and apply the edit again".into(),
    ))
}
