use crate::models::Project;
use crate::{internal, map_err};
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

macro_rules! project_columns {
    () => {
        "id, organization_id, name, domain::text AS domain, brand_name, country_code, \
                       language_code, location_name, archived_at, created_at, updated_at"
    };
}

pub struct NewProject<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: &'a str,
    pub domain: Option<&'a str>,
    pub brand_name: Option<&'a str>,
    pub country_code: &'a str,
    pub language_code: &'a str,
    pub location_name: Option<&'a str>,
}

pub async fn insert(pool: &PgPool, p: NewProject<'_>) -> AppResult<Project> {
    sqlx::query_as(concat!("INSERT INTO projects (id, organization_id, name, domain, brand_name, country_code, language_code, location_name)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING ", project_columns!(), ""))
    .bind(p.id)
    .bind(p.org_id)
    .bind(p.name)
    .bind(p.domain)
    .bind(p.brand_name)
    .bind(p.country_code)
    .bind(p.language_code)
    .bind(p.location_name)
    .fetch_one(pool)
    .await
    .map_err(|e| map_err(e, "project already exists"))
}

pub async fn list(pool: &PgPool, org_id: Uuid, archived: bool) -> AppResult<Vec<Project>> {
    sqlx::query_as(concat!("SELECT ", project_columns!(), " FROM projects WHERE organization_id = $1 AND (archived_at IS NOT NULL) = $2 ORDER BY created_at"))
    .bind(org_id)
    .bind(archived)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn get(pool: &PgPool, org_id: Uuid, id: Uuid) -> AppResult<Project> {
    sqlx::query_as(concat!(
        "SELECT ",
        project_columns!(),
        " FROM projects WHERE organization_id = $1 AND id = $2"
    ))
    .bind(org_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("project"))
}

#[derive(Default)]
pub struct ProjectPatch<'a> {
    pub name: Option<&'a str>,
    /// `Some(None)` clears the value.
    pub domain: Option<Option<&'a str>>,
    pub brand_name: Option<Option<&'a str>>,
    pub country_code: Option<&'a str>,
    pub language_code: Option<&'a str>,
    pub location_name: Option<Option<&'a str>>,
}

pub async fn update(
    pool: &PgPool,
    org_id: Uuid,
    id: Uuid,
    p: ProjectPatch<'_>,
) -> AppResult<Project> {
    sqlx::query_as(concat!(
        "UPDATE projects SET
            name          = COALESCE($3, name),
            domain        = CASE WHEN $4 THEN $5 ELSE domain END,
            brand_name    = CASE WHEN $6 THEN $7 ELSE brand_name END,
            country_code  = COALESCE($8, country_code),
            language_code = COALESCE($9, language_code),
            location_name = CASE WHEN $10 THEN $11 ELSE location_name END,
            updated_at    = now()
         WHERE organization_id = $1 AND id = $2
         RETURNING ",
        project_columns!(),
        ""
    ))
    .bind(org_id)
    .bind(id)
    .bind(p.name)
    .bind(p.domain.is_some())
    .bind(p.domain.flatten())
    .bind(p.brand_name.is_some())
    .bind(p.brand_name.flatten())
    .bind(p.country_code)
    .bind(p.language_code)
    .bind(p.location_name.is_some())
    .bind(p.location_name.flatten())
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("project"))
}

pub async fn set_archived(
    pool: &PgPool,
    org_id: Uuid,
    id: Uuid,
    archived: bool,
) -> AppResult<Project> {
    sqlx::query_as(concat!("UPDATE projects SET archived_at = CASE WHEN $3 THEN now() ELSE NULL END, updated_at = now()
         WHERE organization_id = $1 AND id = $2 RETURNING ", project_columns!(), ""))
    .bind(org_id)
    .bind(id)
    .bind(archived)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("project"))
}

/// Permanently delete a project. Only archived projects may be deleted.
pub async fn delete_archived(pool: &PgPool, org_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query(
        "DELETE FROM projects WHERE organization_id = $1 AND id = $2 AND archived_at IS NOT NULL",
    )
    .bind(org_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "archive the project before deleting it".into(),
        ));
    }
    Ok(())
}
