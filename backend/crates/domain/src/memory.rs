//! Revisioned project context shared by the UI, API and MCP clients.

use crate::actor::{Actor, ActorVia, Role};
use crate::validate;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use utoipa::ToSchema;
use uuid::Uuid;

const MAX_SECTIONS: usize = 20;
const MAX_COMPETITORS: usize = 50;
const MAX_KEY_PAGES: usize = 100;
const MAX_RESEARCH_LOG: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContextSection {
    pub key: String,
    pub title: String,
    pub content: String,
    pub updated_by: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ContextSectionInput {
    pub key: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Competitor {
    pub domain: String,
    pub name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CompetitorInput {
    pub domain: String,
    pub name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KeyPage {
    pub url: String,
    pub role: String,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct KeyPageInput {
    pub url: String,
    /// hub, spoke, money, or other.
    pub role: String,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResearchLogEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub summary: String,
    pub updated_by: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProjectContext {
    pub project_id: Uuid,
    /// Send this value as `revision` when updating. Stale updates are rejected.
    pub revision: i64,
    pub sections: Vec<ContextSection>,
    pub competitors: Vec<Competitor>,
    pub key_pages: Vec<KeyPage>,
    pub research_log: Vec<ResearchLogEntry>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProjectContextInput {
    /// Revision returned by the last GET. Use 0 for a context that has never been saved.
    pub revision: i64,
    #[serde(default)]
    pub sections: Vec<ContextSectionInput>,
    #[serde(default)]
    pub competitors: Vec<CompetitorInput>,
    #[serde(default)]
    pub key_pages: Vec<KeyPageInput>,
    /// Optional immutable research-log entry appended by the server.
    pub research_note: Option<String>,
}

fn author(actor: &Actor) -> &'static str {
    match actor.via {
        ActorVia::ApiKey => "mcp",
        ActorVia::Session | ActorVia::NoAuth => "user",
    }
}

fn decode<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> AppResult<Vec<T>> {
    serde_json::from_value(value).map_err(AppError::internal)
}

fn from_row(project_id: Uuid, row: db::memory::ProjectContextRow) -> AppResult<ProjectContext> {
    Ok(ProjectContext {
        project_id,
        revision: row.revision,
        sections: decode(row.sections)?,
        competitors: decode(row.competitors)?,
        key_pages: decode(row.key_pages)?,
        research_log: decode(row.research_log)?,
        updated_at: Some(row.updated_at),
    })
}

pub async fn get(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<ProjectContext> {
    match db::memory::get(pool, actor.org_id, project_id).await? {
        Some(row) => from_row(project_id, row),
        None => Ok(ProjectContext {
            project_id,
            revision: 0,
            sections: Vec::new(),
            competitors: Vec::new(),
            key_pages: Vec::new(),
            research_log: Vec::new(),
            updated_at: None,
        }),
    }
}

fn bounded(field: &str, actual: usize, max: usize) -> AppResult<()> {
    if actual > max {
        Err(AppError::Validation(format!(
            "{field} can contain at most {max} items"
        )))
    } else {
        Ok(())
    }
}

pub async fn update(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: UpdateProjectContextInput,
) -> AppResult<ProjectContext> {
    actor.require(Role::Admin)?;
    if input.revision < 0 {
        return Err(AppError::Validation("revision cannot be negative".into()));
    }
    bounded("sections", input.sections.len(), MAX_SECTIONS)?;
    bounded("competitors", input.competitors.len(), MAX_COMPETITORS)?;
    bounded("key pages", input.key_pages.len(), MAX_KEY_PAGES)?;

    let who = author(actor).to_string();
    let mut section_keys = HashSet::new();
    let sections = input
        .sections
        .into_iter()
        .map(|s| {
            let key = validate::non_empty("section key", &s.key, 60)?;
            if !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            {
                return Err(AppError::Validation(
                    "section keys may contain only letters, numbers, '-' and '_'".into(),
                ));
            }
            if !section_keys.insert(key.clone()) {
                return Err(AppError::Validation(format!(
                    "section key '{key}' is duplicated"
                )));
            }
            Ok(ContextSection {
                key,
                title: validate::non_empty("section title", &s.title, 120)?,
                content: validate::non_empty("section content", &s.content, 10_000)?,
                updated_by: who.clone(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    let mut domains = HashSet::new();
    let competitors = input
        .competitors
        .into_iter()
        .map(|c| {
            let domain = validate::domain(&c.domain)?;
            if !domains.insert(domain.clone()) {
                return Err(AppError::Validation(format!(
                    "competitor '{domain}' is duplicated"
                )));
            }
            Ok(Competitor {
                domain,
                name: validate::optional("competitor name", c.name.as_deref(), 120)?,
                notes: validate::optional("competitor notes", c.notes.as_deref(), 2_000)?,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    let mut urls = HashSet::new();
    let key_pages = input
        .key_pages
        .into_iter()
        .map(|p| {
            let parsed = url::Url::parse(p.url.trim())
                .map_err(|_| AppError::Validation("key page must be an absolute URL".into()))?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                return Err(AppError::Validation(
                    "key page URL must use http or https".into(),
                ));
            }
            let url = parsed.to_string();
            if !urls.insert(url.clone()) {
                return Err(AppError::Validation(format!(
                    "key page '{url}' is duplicated"
                )));
            }
            let role = p.role.trim().to_lowercase();
            if !matches!(role.as_str(), "hub" | "spoke" | "money" | "other") {
                return Err(AppError::Validation(
                    "key page role must be hub, spoke, money, or other".into(),
                ));
            }
            Ok(KeyPage {
                url,
                role,
                topic: validate::optional("key page topic", p.topic.as_deref(), 200)?,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    let current = get(pool, actor, project_id).await?;
    if current.revision != input.revision {
        return Err(AppError::Conflict(
            "project memory changed since it was loaded; reload it and apply the edit again".into(),
        ));
    }
    let mut research_log = current.research_log;
    if let Some(note) = input.research_note {
        research_log.push(ResearchLogEntry {
            id: new_id(),
            created_at: Utc::now(),
            summary: validate::non_empty("research note", &note, 2_000)?,
            updated_by: who,
        });
        if research_log.len() > MAX_RESEARCH_LOG {
            research_log.drain(..research_log.len() - MAX_RESEARCH_LOG);
        }
    }

    let row = db::memory::put(
        pool,
        actor.org_id,
        project_id,
        db::memory::ContextUpdate {
            expected_revision: input.revision,
            sections: serde_json::to_value(sections).map_err(AppError::internal)?,
            competitors: serde_json::to_value(competitors).map_err(AppError::internal)?,
            key_pages: serde_json::to_value(key_pages).map_err(AppError::internal)?,
            research_log: serde_json::to_value(research_log).map_err(AppError::internal)?,
        },
    )
    .await?;
    from_row(project_id, row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_explicit() {
        assert!(bounded("x", MAX_SECTIONS, MAX_SECTIONS).is_ok());
        assert!(bounded("x", MAX_SECTIONS + 1, MAX_SECTIONS).is_err());
    }
}
