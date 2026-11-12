use crate::models::{Report, ReportListItem, ReportTemplate};
use crate::{internal, map_err};
use common::{AppError, AppResult};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

macro_rules! report_columns {
    () => {
        "r.id, r.project_id, r.title, r.summary, r.html, r.skill, r.template_id, \
         r.created_by, r.shared_at, r.created_at, r.updated_at"
    };
}

macro_rules! report_list_columns {
    () => {
        "r.id, r.project_id, r.title, r.summary, r.skill, r.template_id, \
         r.created_by, r.shared_at, r.created_at, r.updated_at"
    };
}

macro_rules! template_columns {
    () => {
        "t.id, t.project_id, t.name, t.description, t.instructions, t.created_at, t.updated_at"
    };
}

pub struct NewReport<'a> {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub html: &'a str,
    pub skill: Option<&'a str>,
    pub template_id: Option<Uuid>,
    pub created_by: &'a str,
}

pub async fn create(pool: &PgPool, org_id: Uuid, p: NewReport<'_>) -> AppResult<Report> {
    sqlx::query_as(
        "INSERT INTO reports (id, project_id, title, summary, html, skill, template_id, created_by)
         SELECT $3, p.id, $4, $5, $6, $7, t.id, $9
         FROM projects p
         LEFT JOIN report_templates t ON t.id = $8 AND t.project_id = p.id
         WHERE p.id = $1 AND p.organization_id = $2
           AND ($8::uuid IS NULL OR t.id IS NOT NULL)
         RETURNING id, project_id, title, summary, html, skill, template_id, created_by,
                   shared_at, created_at, updated_at",
    )
    .bind(p.project_id)
    .bind(org_id)
    .bind(p.id)
    .bind(p.title)
    .bind(p.summary)
    .bind(p.html)
    .bind(p.skill)
    .bind(p.template_id)
    .bind(p.created_by)
    .fetch_optional(pool)
    .await
    .map_err(|e| map_err(e, "report already exists"))?
    .ok_or(AppError::NotFound("project or report template"))
}

pub async fn list(pool: &PgPool, org_id: Uuid, project_id: Uuid) -> AppResult<Vec<ReportListItem>> {
    sqlx::query_as(concat!(
        "SELECT ",
        report_list_columns!(),
        " FROM reports r
         JOIN projects p ON p.id = r.project_id
         WHERE r.project_id = $1 AND p.organization_id = $2
         ORDER BY r.created_at DESC LIMIT 200"
    ))
    .bind(project_id)
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn get(pool: &PgPool, org_id: Uuid, project_id: Uuid, id: Uuid) -> AppResult<Report> {
    sqlx::query_as(concat!(
        "SELECT ",
        report_columns!(),
        " FROM reports r
         JOIN projects p ON p.id = r.project_id
         WHERE r.id = $1 AND r.project_id = $2 AND p.organization_id = $3"
    ))
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("report"))
}

pub struct ReportPatch<'a> {
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub html: &'a str,
    pub skill: Option<&'a str>,
    pub template_id: Option<Uuid>,
}

pub async fn update(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
    p: ReportPatch<'_>,
) -> AppResult<Report> {
    sqlx::query_as(concat!(
        "UPDATE reports r SET title = $4, summary = $5, html = $6, skill = $7,
             template_id = $8, share_token_hash = NULL, shared_at = NULL, updated_at = now()
         FROM projects p
         WHERE r.id = $1 AND r.project_id = $2 AND p.id = r.project_id
           AND p.organization_id = $3
           AND ($8::uuid IS NULL OR EXISTS (
               SELECT 1 FROM report_templates t WHERE t.id = $8 AND t.project_id = r.project_id))
         RETURNING ",
        report_columns!(),
        ""
    ))
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .bind(p.title)
    .bind(p.summary)
    .bind(p.html)
    .bind(p.skill)
    .bind(p.template_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("report or report template"))
}

pub async fn delete(pool: &PgPool, org_id: Uuid, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM reports r USING projects p
         WHERE r.id = $1 AND r.project_id = $2 AND p.id = r.project_id AND p.organization_id = $3",
    )
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("report"));
    }
    Ok(())
}

pub async fn set_share(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
    token_hash: Option<&str>,
) -> AppResult<Report> {
    sqlx::query_as(concat!(
        "UPDATE reports r SET share_token_hash = $4,
             shared_at = CASE WHEN $4::text IS NULL THEN NULL ELSE now() END,
             updated_at = now()
         FROM projects p
         WHERE r.id = $1 AND r.project_id = $2 AND p.id = r.project_id AND p.organization_id = $3
         RETURNING ",
        report_columns!(),
        ""
    ))
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("report"))
}

pub async fn get_shared(pool: &PgPool, token_hash: &str) -> AppResult<Report> {
    sqlx::query_as(concat!(
        "SELECT ",
        report_columns!(),
        " FROM reports r
         WHERE r.share_token_hash = $1 AND r.shared_at IS NOT NULL"
    ))
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("shared report"))
}

pub struct NewTemplate<'a> {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub instructions: &'a str,
}

async fn lock_project(
    tx: &mut Transaction<'_, Postgres>,
    org_id: Uuid,
    project_id: Uuid,
) -> AppResult<()> {
    let found = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM projects WHERE id = $1 AND organization_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(org_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)?;
    found.map(|_| ()).ok_or(AppError::NotFound("project"))
}

pub async fn create_template(
    pool: &PgPool,
    org_id: Uuid,
    p: NewTemplate<'_>,
) -> AppResult<ReportTemplate> {
    let mut tx = pool.begin().await.map_err(internal)?;
    lock_project(&mut tx, org_id, p.project_id).await?;
    let count =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM report_templates WHERE project_id = $1")
            .bind(p.project_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if count >= 10 {
        return Err(AppError::Conflict(
            "a project can have at most 10 report templates; delete one first".into(),
        ));
    }
    let row = sqlx::query_as(
        "INSERT INTO report_templates (id, project_id, name, description, instructions)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, project_id, name, description, instructions, created_at, updated_at",
    )
    .bind(p.id)
    .bind(p.project_id)
    .bind(p.name)
    .bind(p.description)
    .bind(p.instructions)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_err(e, "a report template with this name already exists"))?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}

pub async fn list_templates(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
) -> AppResult<Vec<ReportTemplate>> {
    sqlx::query_as(concat!(
        "SELECT ",
        template_columns!(),
        " FROM report_templates t
         JOIN projects p ON p.id = t.project_id
         WHERE t.project_id = $1 AND p.organization_id = $2 ORDER BY t.created_at DESC"
    ))
    .bind(project_id)
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn update_template(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
    name: &str,
    description: Option<&str>,
    instructions: &str,
) -> AppResult<ReportTemplate> {
    sqlx::query_as(concat!(
        "UPDATE report_templates t SET name = $4, description = $5, instructions = $6,
             updated_at = now()
         FROM projects p
         WHERE t.id = $1 AND t.project_id = $2 AND p.id = t.project_id AND p.organization_id = $3
         RETURNING ",
        template_columns!(),
        ""
    ))
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .bind(name)
    .bind(description)
    .bind(instructions)
    .fetch_optional(pool)
    .await
    .map_err(|e| map_err(e, "a report template with this name already exists"))?
    .ok_or(AppError::NotFound("report template"))
}

pub async fn delete_template(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM report_templates t USING projects p
         WHERE t.id = $1 AND t.project_id = $2 AND p.id = t.project_id AND p.organization_id = $3",
    )
    .bind(id)
    .bind(project_id)
    .bind(org_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("report template"));
    }
    Ok(())
}
