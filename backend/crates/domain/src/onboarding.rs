//! Website onboarding: collect crawl evidence, then explicitly review and save project context.
use crate::{Actor, Role, audits, memory, projects};
use common::{AppError, AppResult};
use db::{audits::Audit, dashboard::WebsitePage};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct WebsiteResearchInput {
    /// Optional root URL, useful for HTTP-only sites. Must match the project's host.
    pub start_url: Option<String>,
}
pub async fn research(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    policy: crawler::UrlPolicy,
    renderer: bool,
    input: WebsiteResearchInput,
) -> AppResult<Audit> {
    actor.require(Role::Admin)?;
    audits::start_first(
        pool,
        actor,
        pid,
        policy,
        renderer,
        audits::StartAuditInput {
            start_url: input.start_url,
            max_pages: Some(50),
            render_js: Some(renderer),
            ..Default::default()
        },
    )
    .await
}

#[derive(Serialize, ToSchema)]
pub struct WebsiteSetupView {
    pub audit: Option<Audit>,
    pub evidence: Vec<WebsitePage>,
    pub suggested_overview: Option<String>,
    pub context: memory::ProjectContext,
    pub note: String,
}
pub async fn get(pool: &PgPool, actor: &Actor, pid: Uuid) -> AppResult<WebsiteSetupView> {
    let p = projects::get(pool, actor, pid).await?;
    let audit = db::audits::latest_for_domain(
        pool,
        actor.org_id,
        pid,
        p.domain.as_deref().unwrap_or(""),
        false,
    )
    .await?;
    let evidence = match &audit {
        Some(a) => db::dashboard::website_pages(pool, actor.org_id, pid, a.id).await?,
        None => vec![],
    };
    let suggested_overview = evidence.first().map(|page| {
        [
            page.title.as_deref(),
            page.description.as_deref(),
            page.h1.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
        .chars()
        .take(4000)
        .collect()
    });
    let context = memory::get(pool, actor, pid).await?;
    Ok(WebsiteSetupView {audit,evidence,suggested_overview,context,note:"Draft from crawl evidence, not a generated strategy. Review the website text and add your own goal before saving. If the site cannot be read, enter the overview manually. Existing memory is preserved unless you explicitly edit these fields.".into()})
}

#[derive(Deserialize, ToSchema)]
pub struct WebsiteSetupInput {
    pub revision: i64,
    /// Website shown during review. Reject a save if the project website changed meanwhile.
    pub expected_domain: String,
    pub business_overview: String,
    pub current_goal: String,
    /// Selected evidence URLs only; preserve existing pages with the same URL.
    #[serde(default)]
    pub key_pages: Vec<String>,
}
pub async fn save(
    pool: &PgPool,
    actor: &Actor,
    pid: Uuid,
    input: WebsiteSetupInput,
) -> AppResult<memory::ProjectContext> {
    actor.require(Role::Admin)?;
    let project = projects::get(pool, actor, pid).await?;
    if project.domain.as_deref() != Some(input.expected_domain.as_str()) {
        return Err(AppError::Conflict(
            "website changed; reload onboarding before saving".into(),
        ));
    }
    let current = get(pool, actor, pid).await?;
    let overview = crate::validate::non_empty("business overview", &input.business_overview, 4000)?;
    let goal = crate::validate::non_empty("current goal", &input.current_goal, 4000)?;
    let data = current.context;
    let mut sections: Vec<memory::ContextSectionInput> = data
        .sections
        .into_iter()
        .filter(|s| !["business_overview", "current_goal"].contains(&s.key.as_str()))
        .map(|s| memory::ContextSectionInput {
            key: s.key,
            title: s.title,
            content: s.content,
        })
        .collect();
    let mut key_pages: Vec<memory::KeyPageInput> = data
        .key_pages
        .into_iter()
        .map(|p| memory::KeyPageInput {
            url: p.url,
            role: p.role,
            topic: p.topic,
        })
        .collect();
    sections.push(memory::ContextSectionInput {
        key: "business_overview".into(),
        title: "Business overview".into(),
        content: overview,
    });
    sections.push(memory::ContextSectionInput {
        key: "current_goal".into(),
        title: "Current goal".into(),
        content: goal,
    });
    for url in input.key_pages {
        let page = current
            .evidence
            .iter()
            .find(|p| p.url == url)
            .ok_or_else(|| {
                AppError::Validation(
                    "selected key page is not in the current website evidence".into(),
                )
            })?;
        if !key_pages.iter().any(|p| p.url == url) {
            key_pages.push(memory::KeyPageInput {
                url,
                role: "other".into(),
                topic: Some(
                    page.title
                        .clone()
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect(),
                ),
            });
        }
    }
    memory::update_for_domain(pool,actor,pid,memory::UpdateProjectContextInput{revision:input.revision,sections,competitors:data.competitors.into_iter().map(|c| memory::CompetitorInput {domain:c.domain,name:c.name,notes:c.notes}).collect(),key_pages,research_note:Some("Reviewed website onboarding findings and confirmed the business overview and current goal.".into())},Some(&input.expected_domain)).await
}
