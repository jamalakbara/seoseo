//! Stored self-contained HTML reports and reusable instruction templates.

use crate::actor::{Actor, ActorVia, Role};
use crate::validate;
use common::{AppError, AppResult, new_id};
use db::models::{Report, ReportListItem, ReportTemplate};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const MAX_HTML_BYTES: usize = 500 * 1024;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveReportInput {
    pub title: String,
    pub summary: Option<String>,
    /// A complete, self-contained HTML document. External scripts are rejected.
    pub html: String,
    pub skill: Option<String>,
    pub template_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveReportTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareReportResult {
    /// Secret URL token. It is shown only when sharing is enabled.
    pub token: String,
    pub report: Report,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SharedReportView {
    pub title: String,
    pub summary: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<&Report> for SharedReportView {
    fn from(report: &Report) -> Self {
        Self {
            title: report.title.clone(),
            summary: report.summary.clone(),
            created_at: report.created_at,
            updated_at: report.updated_at,
        }
    }
}

fn creator(actor: &Actor) -> &'static str {
    match actor.via {
        ActorVia::ApiKey => "mcp",
        ActorVia::Session | ActorVia::NoAuth => "user",
    }
}

fn validate_html(value: &str) -> AppResult<String> {
    let html = value.trim();
    if html.is_empty() {
        return Err(AppError::Validation("report HTML is required".into()));
    }
    if html.len() > MAX_HTML_BYTES {
        return Err(AppError::Validation(
            "report HTML must be at most 500 KiB".into(),
        ));
    }
    let lower = html.to_ascii_lowercase();
    if !lower.contains("<html") || !lower.contains("</html>") {
        return Err(AppError::Validation(
            "report HTML must be a complete document with an <html> element".into(),
        ));
    }
    if lower.contains("<script src=") || lower.contains("<script src =") {
        return Err(AppError::Validation(
            "report HTML must be self-contained and cannot load external scripts".into(),
        ));
    }
    Ok(html.to_string())
}

struct ValidatedReport {
    title: String,
    summary: Option<String>,
    html: String,
    skill: Option<String>,
    template_id: Option<Uuid>,
}

fn validated(input: SaveReportInput) -> AppResult<ValidatedReport> {
    Ok(ValidatedReport {
        title: validate::non_empty("report title", &input.title, 200)?,
        summary: validate::optional("report summary", input.summary.as_deref(), 2_500)?,
        html: validate_html(&input.html)?,
        skill: validate::optional("report skill", input.skill.as_deref(), 120)?,
        template_id: input.template_id,
    })
}

pub async fn create(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: SaveReportInput,
) -> AppResult<Report> {
    actor.require(Role::Admin)?;
    let input = validated(input)?;
    db::reports::create(
        pool,
        actor.org_id,
        db::reports::NewReport {
            id: new_id(),
            project_id,
            title: &input.title,
            summary: input.summary.as_deref(),
            html: &input.html,
            skill: input.skill.as_deref(),
            template_id: input.template_id,
            created_by: creator(actor),
        },
    )
    .await
}

pub async fn list(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
) -> AppResult<Vec<ReportListItem>> {
    db::reports::list(pool, actor.org_id, project_id).await
}

pub async fn get(pool: &PgPool, actor: &Actor, project_id: Uuid, id: Uuid) -> AppResult<Report> {
    db::reports::get(pool, actor.org_id, project_id, id).await
}

pub async fn update(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
    input: SaveReportInput,
) -> AppResult<Report> {
    actor.require(Role::Admin)?;
    let input = validated(input)?;
    db::reports::update(
        pool,
        actor.org_id,
        project_id,
        id,
        db::reports::ReportPatch {
            title: &input.title,
            summary: input.summary.as_deref(),
            html: &input.html,
            skill: input.skill.as_deref(),
            template_id: input.template_id,
        },
    )
    .await
}

pub async fn delete(pool: &PgPool, actor: &Actor, project_id: Uuid, id: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::reports::delete(pool, actor.org_id, project_id, id).await
}

pub async fn share(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<ShareReportResult> {
    actor.require(Role::Admin)?;
    let token = common::token::random_token(24);
    let report = db::reports::set_share(
        pool,
        actor.org_id,
        project_id,
        id,
        Some(&common::token::sha256_hex(&token)),
    )
    .await?;
    Ok(ShareReportResult { token, report })
}

pub async fn unshare(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<Report> {
    actor.require(Role::Admin)?;
    db::reports::set_share(pool, actor.org_id, project_id, id, None).await
}

pub async fn shared(pool: &PgPool, token: &str) -> AppResult<Report> {
    if token.len() != 32
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(AppError::NotFound("shared report"));
    }
    db::reports::get_shared(pool, &common::token::sha256_hex(token)).await
}

pub async fn create_template(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: SaveReportTemplateInput,
) -> AppResult<ReportTemplate> {
    actor.require(Role::Admin)?;
    let name = validate::non_empty("template name", &input.name, 120)?;
    let description =
        validate::optional("template description", input.description.as_deref(), 500)?;
    let instructions = validate::non_empty("template instructions", &input.instructions, 3_000)?;
    db::reports::create_template(
        pool,
        actor.org_id,
        db::reports::NewTemplate {
            id: new_id(),
            project_id,
            name: &name,
            description: description.as_deref(),
            instructions: &instructions,
        },
    )
    .await
}

pub async fn list_templates(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
) -> AppResult<Vec<ReportTemplate>> {
    db::reports::list_templates(pool, actor.org_id, project_id).await
}

pub async fn update_template(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
    input: SaveReportTemplateInput,
) -> AppResult<ReportTemplate> {
    actor.require(Role::Admin)?;
    let name = validate::non_empty("template name", &input.name, 120)?;
    let description =
        validate::optional("template description", input.description.as_deref(), 500)?;
    let instructions = validate::non_empty("template instructions", &input.instructions, 3_000)?;
    db::reports::update_template(
        pool,
        actor.org_id,
        project_id,
        id,
        &name,
        description.as_deref(),
        &instructions,
    )
    .await
}

pub async fn delete_template(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::reports::delete_template(pool, actor.org_id, project_id, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_html_is_complete_and_bounded() {
        assert!(validate_html("<!doctype html><html><body>ok</body></html>").is_ok());
        assert!(validate_html("<p>fragment</p>").is_err());
        assert!(validate_html("<html><script src=\"https://x\"></script></html>").is_err());
    }
}
