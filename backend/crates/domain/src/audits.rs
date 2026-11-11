//! Site audits: start, inspect and manage audits of a project's website.

use crate::actor::{Actor, Role};
use common::{AppError, AppResult, new_id};
use crawler::UrlPolicy;
use crawler::rules::{CATALOG, IssueType};
use db::audits::{self, Audit, IssueCount, IssueRow, LinkRow, PageDetail, PageFilter, PageRow};
use jobs::site_audit::{AuditConfig, KIND};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub const MAX_PAGES_LIMIT: u32 = 10_000;

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct StartAuditInput {
    /// Defaults to `https://{project domain}/`.
    pub start_url: Option<String>,
    /// Pages to crawl (1–10000, default 500).
    pub max_pages: Option<u32>,
    /// Parallel requests (1–8, default 4).
    pub concurrency: Option<u32>,
    /// Delay between requests per slot in ms (0–10000, default 250).
    pub delay_ms: Option<u64>,
    /// Obey robots.txt (default true). Turn off only for sites you own.
    pub respect_robots: Option<bool>,
    /// Audit the rendered DOM (requires the renderer profile).
    pub render_js: Option<bool>,
    /// Run desktop Lighthouse lab audits (requires the renderer profile).
    pub lighthouse: Option<bool>,
    /// Path prefixes to skip, e.g. `/tag/`.
    pub exclude_paths: Option<Vec<String>>,
}

pub async fn start(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    policy: UrlPolicy,
    renderer_available: bool,
    input: StartAuditInput,
) -> AppResult<Audit> {
    actor.require(Role::Member)?;
    let project = db::projects::get(pool, actor.org_id, project_id).await?;
    if project.archived_at.is_some() {
        return Err(AppError::Conflict(
            "restore the project before auditing it".into(),
        ));
    }
    let raw = match input
        .start_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(u) if u.contains("://") => u.to_string(),
        Some(u) => format!("https://{u}"),
        None => match &project.domain {
            Some(d) => format!("https://{d}/"),
            None => {
                return Err(AppError::Validation(
                    "set the project's website or pass a start URL".into(),
                ));
            }
        },
    };
    let url = Url::parse(&raw)
        .ok()
        .and_then(|u| crawler::urls::normalize(&u))
        .ok_or_else(|| AppError::Validation("start URL must be a valid http(s) URL".into()))?;
    policy
        .check_url(&url)
        .map_err(|e| AppError::Validation(format!("cannot audit this URL: {e}")))?;

    if (input.render_js.unwrap_or(false) || input.lighthouse.unwrap_or(false))
        && !renderer_available
    {
        return Err(AppError::Conflict(
            "start the renderer profile and set RENDERER_URL before requesting browser audits"
                .into(),
        ));
    }
    let d = AuditConfig::default();
    let range = |v: Option<u64>, lo: u64, hi: u64, def: u64, name: &str| -> AppResult<u64> {
        let v = v.unwrap_or(def);
        if (lo..=hi).contains(&v) {
            Ok(v)
        } else {
            Err(AppError::Validation(format!(
                "{name} must be between {lo} and {hi}"
            )))
        }
    };
    let exclude: Vec<String> = input
        .exclude_paths
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p.starts_with('/') {
                p
            } else {
                format!("/{p}")
            }
        })
        .collect();
    if exclude.len() > 20 {
        return Err(AppError::Validation("at most 20 excluded paths".into()));
    }
    let cfg = AuditConfig {
        max_pages: range(
            input.max_pages.map(u64::from),
            1,
            MAX_PAGES_LIMIT as u64,
            d.max_pages as u64,
            "max_pages",
        )? as u32,
        concurrency: range(
            input.concurrency.map(u64::from),
            1,
            8,
            d.concurrency as u64,
            "concurrency",
        )? as u32,
        delay_ms: range(input.delay_ms, 0, 10_000, d.delay_ms, "delay_ms")?,
        respect_robots: input.respect_robots.unwrap_or(true),
        render_js: input.render_js.unwrap_or(false),
        lighthouse: input.lighthouse.unwrap_or(false),
        renderer_url: None,
        exclude_paths: exclude,
    };

    if audits::count_active(pool, project_id).await? > 0 {
        return Err(AppError::Conflict(
            "an audit is already running for this project; wait for it or cancel it".into(),
        ));
    }
    let audit_id = new_id();
    let audit = audits::insert(
        pool,
        audit_id,
        project_id,
        actor.user_id,
        url.as_str(),
        serde_json::to_value(&cfg).map_err(AppError::internal)?,
    )
    .await?;
    // Audits run for a long time; allow a couple of resumes after worker crashes.
    let job = db::jobs::enqueue(
        pool,
        new_id(),
        KIND,
        Some(actor.org_id),
        Some(project_id),
        serde_json::json!({ "audit_id": audit_id }),
        3,
    )
    .await?;
    audits::set_job(pool, audit_id, job.id).await?;
    Ok(Audit {
        job_id: Some(job.id),
        ..audit
    })
}

async fn project_scope(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<()> {
    db::projects::get(pool, actor.org_id, project_id)
        .await
        .map(|_| ())
}

pub async fn list(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Vec<Audit>> {
    project_scope(pool, actor, project_id).await?;
    audits::list(pool, project_id, 50).await
}

pub async fn get(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
) -> AppResult<Audit> {
    project_scope(pool, actor, project_id).await?;
    audits::get(pool, project_id, audit_id).await
}

pub async fn cancel(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
) -> AppResult<Audit> {
    actor.require(Role::Member)?;
    project_scope(pool, actor, project_id).await?;
    audits::cancel(pool, project_id, audit_id).await
}

pub async fn delete(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
) -> AppResult<()> {
    actor.require(Role::Admin)?;
    project_scope(pool, actor, project_id).await?;
    let a = audits::get(pool, project_id, audit_id).await?;
    if matches!(a.status.as_str(), "queued" | "running") {
        return Err(AppError::Conflict(
            "cancel the audit before deleting it".into(),
        ));
    }
    audits::delete(pool, project_id, audit_id).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IssueSummary {
    #[serde(flatten)]
    pub count: IssueCount,
    pub title: String,
    pub category: String,
}

pub async fn issue_summary(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
) -> AppResult<Vec<IssueSummary>> {
    get(pool, actor, project_id, audit_id).await?;
    Ok(audits::issue_counts(pool, audit_id)
        .await?
        .into_iter()
        .map(|c| {
            let t = crawler::rules::issue_type(&c.issue_type);
            IssueSummary {
                title: t.map_or_else(|| c.issue_type.clone(), |t| t.title.to_string()),
                category: t.map_or("other", |t| t.category).to_string(),
                count: c,
            }
        })
        .collect())
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PageQuery {
    /// Substring of URL or title.
    pub search: Option<String>,
    /// 2xx | 3xx | 4xx | 5xx | error
    pub status: Option<String>,
    /// Only pages with this issue type.
    pub issue_type: Option<String>,
    pub indexable: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total: i64,
}

pub async fn pages(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
    q: PageQuery,
) -> AppResult<Paged<PageRow>> {
    get(pool, actor, project_id, audit_id).await?;
    let filter = PageFilter {
        search: q.search,
        status: q.status,
        issue_type: q.issue_type,
        indexable: q.indexable,
    };
    let (items, total) = audits::pages(
        pool,
        audit_id,
        &filter,
        q.limit.unwrap_or(50).clamp(1, 500),
        q.offset.unwrap_or(0).max(0),
    )
    .await?;
    Ok(Paged { items, total })
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PageView {
    pub page: PageDetail,
    pub issues: Vec<IssueRow>,
    pub outlinks: Vec<LinkRow>,
    pub inlinks: Vec<LinkRow>,
}

pub async fn page(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
    page_id: Uuid,
) -> AppResult<PageView> {
    get(pool, actor, project_id, audit_id).await?;
    let page = audits::page_detail(pool, audit_id, page_id).await?;
    let (issues, _) = audits::issues(pool, audit_id, None, Some(page_id), 500, 0).await?;
    Ok(PageView {
        page,
        issues,
        outlinks: audits::outlinks(pool, audit_id, page_id).await?,
        inlinks: audits::inlinks(pool, audit_id, page_id).await?,
    })
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct IssueQuery {
    pub issue_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn issues(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    audit_id: Uuid,
    q: IssueQuery,
) -> AppResult<Paged<IssueRow>> {
    get(pool, actor, project_id, audit_id).await?;
    let (items, total) = audits::issues(
        pool,
        audit_id,
        q.issue_type.as_deref().filter(|s| !s.is_empty()),
        None,
        q.limit.unwrap_or(100).clamp(1, 1000),
        q.offset.unwrap_or(0).max(0),
    )
    .await?;
    Ok(Paged { items, total })
}

pub fn issue_types() -> &'static [IssueType] {
    CATALOG
}
