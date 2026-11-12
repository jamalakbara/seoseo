//! Site audit persistence: audits, crawl frontier, pages, link graph, issues.
//! Read queries take the project id so callers stay tenant-scoped.

use crate::{escape_like, internal};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! audit_columns {
    () => {
        "id, project_id, started_by, job_id, start_url, status, phase, config, pages_crawled, \
         pages_total, pages_failed, health_score, issue_counts, stop_reason, error, started_at, \
         finished_at, created_at"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Audit {
    pub id: Uuid,
    pub project_id: Uuid,
    pub started_by: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub start_url: String,
    /// queued | running | completed | failed | cancelled
    pub status: String,
    /// queued | probe | discover | crawl | analyze | done
    pub phase: String,
    pub config: serde_json::Value,
    pub pages_crawled: i32,
    pub pages_total: i32,
    pub pages_failed: i32,
    pub health_score: Option<i32>,
    /// `{ "critical": n, "warning": n, "info": n, "by_type": { id: pages } }`
    pub issue_counts: serde_json::Value,
    pub stop_reason: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct StartAudit<'a> {
    pub org: Uuid,
    pub project: Uuid,
    pub user: Uuid,
    pub url: &'a str,
    pub config: serde_json::Value,
    /// Onboarding reuses an existing audit of this domain. Manual starts pass None.
    pub first_domain: Option<&'a str>,
}

pub async fn start_with_job(pool: &PgPool, input: StartAudit<'_>) -> AppResult<Audit> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let domain: Option<String> = sqlx::query_scalar("SELECT domain::text FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE")
        .bind(input.org).bind(input.project).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if let Some(expected) = input.first_domain {
        if domain.as_deref() != Some(expected) {
            return Err(AppError::Conflict(
                "website changed; reload onboarding".into(),
            ));
        }
        let existing: Option<Audit> = sqlx::query_as(concat!("SELECT ",audit_columns!()," FROM audits WHERE project_id=$1 AND lower(split_part(split_part(split_part(start_url,'://',2),'/',1),':',1))=$2 ORDER BY created_at DESC LIMIT 1"))
            .bind(input.project).bind(expected).fetch_optional(&mut *tx).await.map_err(internal)?;
        if let Some(audit) = existing {
            tx.commit().await.map_err(internal)?;
            return Ok(audit);
        }
    }
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM audits WHERE project_id=$1 AND status IN ('queued','running'))")
        .bind(input.project).fetch_one(&mut *tx).await.map_err(internal)?;
    if active {
        return Err(AppError::Conflict(
            "an audit is already running for this project; wait for it or cancel it".into(),
        ));
    }
    let aid = common::new_id();
    let job = common::new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'site_audit',$2,$3,$4,3)")
        .bind(job).bind(input.org).bind(input.project).bind(serde_json::json!({"audit_id":aid})).execute(&mut *tx).await.map_err(internal)?;
    let audit = sqlx::query_as(concat!("INSERT INTO audits(id,project_id,started_by,start_url,config,job_id) VALUES($1,$2,$3,$4,$5,$6) RETURNING ",audit_columns!()))
        .bind(aid).bind(input.project).bind(input.user).bind(input.url).bind(input.config).bind(job).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(audit)
}

pub async fn latest_for_domain(
    pool: &PgPool,
    org: Uuid,
    project: Uuid,
    domain: &str,
    completed: bool,
) -> AppResult<Option<Audit>> {
    // Project scope is verified inside this query, not only by the service.
    sqlx::query_as(concat!("SELECT ",audit_columns!()," FROM audits WHERE project_id=$2 AND EXISTS(SELECT 1 FROM projects WHERE id=$2 AND organization_id=$1) AND lower(split_part(split_part(split_part(start_url,'://',2),'/',1),':',1))=$3 AND (NOT $4 OR status='completed') ORDER BY created_at DESC LIMIT 1"))
        .bind(org).bind(project).bind(domain).bind(completed).fetch_optional(pool).await.map_err(internal)
}

pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    project_id: Uuid,
    started_by: Uuid,
    start_url: &str,
    config: serde_json::Value,
) -> AppResult<Audit> {
    sqlx::query_as(concat!(
        "INSERT INTO audits (id, project_id, started_by, start_url, config) VALUES ($1, $2, $3, $4, $5) RETURNING ",
        audit_columns!()
    ))
    .bind(id)
    .bind(project_id)
    .bind(started_by)
    .bind(start_url)
    .bind(config)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn set_job(pool: &PgPool, id: Uuid, job_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE audits SET job_id = $2 WHERE id = $1")
        .bind(id)
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Unscoped read for the worker (job payload carries the audit id).
pub async fn get_any(pool: &PgPool, id: Uuid) -> AppResult<Audit> {
    sqlx::query_as(concat!(
        "SELECT ",
        audit_columns!(),
        " FROM audits WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("audit"))
}

pub async fn get(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<Audit> {
    sqlx::query_as(concat!(
        "SELECT ",
        audit_columns!(),
        " FROM audits WHERE id = $1 AND project_id = $2"
    ))
    .bind(id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("audit"))
}

pub async fn list(pool: &PgPool, project_id: Uuid, limit: i64) -> AppResult<Vec<Audit>> {
    sqlx::query_as(concat!(
        "SELECT ",
        audit_columns!(),
        " FROM audits WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2"
    ))
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn count_active(pool: &PgPool, project_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM audits WHERE project_id = $1 AND status IN ('queued', 'running')",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn delete(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query("DELETE FROM audits WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(project_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("audit"));
    }
    Ok(())
}

pub async fn cancel(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<Audit> {
    sqlx::query_as(concat!(
        "UPDATE audits SET status = 'cancelled', stop_reason = 'cancelled', finished_at = now(), updated_at = now()
         WHERE id = $1 AND project_id = $2 AND status IN ('queued', 'running') RETURNING ",
        audit_columns!()
    ))
    .bind(id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| AppError::Conflict("only queued or running audits can be cancelled".into()))
}

pub async fn set_phase(pool: &PgPool, id: Uuid, phase: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE audits SET phase = $2, status = CASE WHEN status = 'queued' THEN 'running' ELSE status END,
                started_at = COALESCE(started_at, now()), updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(phase)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn set_start_url(pool: &PgPool, id: Uuid, url: &str) -> AppResult<()> {
    sqlx::query("UPDATE audits SET start_url = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(url)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Recount progress from the source tables (idempotent across restarts).
pub async fn refresh_progress(pool: &PgPool, id: Uuid) -> AppResult<(i32, i32)> {
    let row: (i32, i32, i32) = sqlx::query_as(
        "UPDATE audits a SET
            pages_crawled = (SELECT count(*) FROM audit_pages p WHERE p.audit_id = a.id),
            pages_failed  = (SELECT count(*) FROM audit_pages p WHERE p.audit_id = a.id AND p.fetch_class <> 'html'),
            pages_total   = LEAST((a.config->>'max_pages')::int,
                              (SELECT count(*) FROM audit_frontier f WHERE f.audit_id = a.id AND f.state <> 'skipped')),
            updated_at = now()
         WHERE a.id = $1 RETURNING pages_crawled, pages_total, pages_failed",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    Ok((row.0, row.1))
}

pub async fn finish(
    pool: &PgPool,
    id: Uuid,
    health_score: i32,
    issue_counts: serde_json::Value,
    stop_reason: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE audits SET status = 'completed', phase = 'done', health_score = $2, issue_counts = $3,
                stop_reason = COALESCE($4, stop_reason), finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'running'",
    )
    .bind(id)
    .bind(health_score)
    .bind(issue_counts)
    .bind(stop_reason)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn set_stop_reason(pool: &PgPool, id: Uuid, reason: &str) -> AppResult<()> {
    sqlx::query("UPDATE audits SET stop_reason = $2 WHERE id = $1")
        .bind(id)
        .bind(reason)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn fail(pool: &PgPool, id: Uuid, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE audits SET status = 'failed', error = left($2, 1000), finished_at = now(), updated_at = now()
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn status(pool: &PgPool, id: Uuid) -> AppResult<String> {
    sqlx::query_scalar("SELECT status FROM audits WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::NotFound("audit"))
}

// ---------------------------------------------------------------- frontier

#[derive(Clone, Copy)]
pub struct FrontierInsert<'a> {
    pub url_hash: &'a str,
    pub url: &'a str,
    pub depth: i32,
    pub source: &'a str,
    pub in_sitemap: bool,
}

/// Add URLs to the frontier; existing entries keep their state but learn `in_sitemap`
/// and the smallest depth.
pub async fn frontier_add(
    pool: &PgPool,
    audit_id: Uuid,
    rows: &[FrontierInsert<'_>],
) -> AppResult<()> {
    // Postgres rejects an upsert that touches the same row twice, so merge duplicates first.
    let mut merged: Vec<FrontierInsert<'_>> = Vec::with_capacity(rows.len());
    let mut index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for r in rows {
        match index.get(r.url_hash) {
            Some(&i) => {
                merged[i].depth = merged[i].depth.min(r.depth);
                merged[i].in_sitemap |= r.in_sitemap;
            }
            None => {
                index.insert(r.url_hash, merged.len());
                merged.push(*r);
            }
        }
    }
    for chunk in merged.chunks(500) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO audit_frontier (audit_id, url_hash, url, depth, source, in_sitemap) ",
        );
        qb.push_values(chunk, |mut b, r| {
            b.push_bind(audit_id)
                .push_bind(r.url_hash)
                .push_bind(r.url)
                .push_bind(r.depth)
                .push_bind(r.source)
                .push_bind(r.in_sitemap);
        });
        qb.push(
            " ON CONFLICT (audit_id, url_hash) DO UPDATE SET
                in_sitemap = audit_frontier.in_sitemap OR EXCLUDED.in_sitemap,
                depth = LEAST(audit_frontier.depth, EXCLUDED.depth)",
        );
        qb.build().execute(pool).await.map_err(internal)?;
    }
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FrontierRow {
    pub url_hash: String,
    pub url: String,
    pub depth: i32,
    pub in_sitemap: bool,
}

pub async fn frontier_next(
    pool: &PgPool,
    audit_id: Uuid,
    limit: i64,
) -> AppResult<Vec<FrontierRow>> {
    sqlx::query_as(
        "SELECT url_hash, url, depth, in_sitemap FROM audit_frontier
         WHERE audit_id = $1 AND state = 'pending' ORDER BY depth, url LIMIT $2",
    )
    .bind(audit_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn frontier_mark(
    pool: &PgPool,
    audit_id: Uuid,
    url_hash: &str,
    state: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE audit_frontier SET state = $3 WHERE audit_id = $1 AND url_hash = $2")
        .bind(audit_id)
        .bind(url_hash)
        .bind(state)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

pub async fn frontier_count(pool: &PgPool, audit_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM audit_frontier WHERE audit_id = $1")
        .bind(audit_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

/// Skip whatever is still pending (crawl budget reached).
pub async fn frontier_skip_pending(pool: &PgPool, audit_id: Uuid) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE audit_frontier SET state = 'skipped' WHERE audit_id = $1 AND state = 'pending'",
    )
    .bind(audit_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(r.rows_affected())
}

// ---------------------------------------------------------------- pages

#[derive(Debug, Default)]
pub struct NewPage {
    pub browser_result: Option<serde_json::Value>,
    pub id: Uuid,
    pub url: String,
    pub url_hash: String,
    pub final_url: Option<String>,
    pub status_code: Option<i32>,
    pub fetch_class: String,
    pub content_type: Option<String>,
    pub response_time_ms: i32,
    pub size_bytes: i32,
    pub redirect_chain: serde_json::Value,
    pub depth: i32,
    pub in_sitemap: bool,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub meta_robots: Option<String>,
    pub x_robots_tag: Option<String>,
    pub canonical_url: Option<String>,
    pub header_canonical_url: Option<String>,
    pub is_indexable: bool,
    pub lang: Option<String>,
    pub has_viewport: bool,
    pub h1: Option<String>,
    pub h1_count: i32,
    pub word_count: i32,
    pub images_total: i32,
    pub images_missing_alt: i32,
    pub internal_links: i32,
    pub external_links: i32,
    pub structured_data_count: i32,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub hreflang: serde_json::Value,
    pub content_hash: Option<String>,
    pub error: Option<String>,
}

pub struct NewLink {
    pub to_url_hash: String,
    pub to_url: String,
    pub anchor: Option<String>,
    pub nofollow: bool,
}

pub struct NewIssue {
    pub page_id: Option<Uuid>,
    pub page_url: Option<String>,
    pub issue_type: String,
    pub severity: String,
    pub details: serde_json::Value,
}

/// Store a crawled page with its outgoing links and page-level issues, atomically.
/// Returns false if the page already existed (resumed crawl), in which case nothing is written.
pub async fn insert_page(
    pool: &PgPool,
    audit_id: Uuid,
    p: &NewPage,
    links: &[NewLink],
    issues: &[NewIssue],
) -> AppResult<bool> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let inserted = sqlx::query(
        "INSERT INTO audit_pages (id, audit_id, url, url_hash, final_url, status_code, fetch_class, content_type,
            response_time_ms, size_bytes, redirect_chain, depth, in_sitemap, title, meta_description, meta_robots,
            x_robots_tag, canonical_url, header_canonical_url, is_indexable, lang, has_viewport, h1, h1_count,
            word_count, images_total, images_missing_alt, internal_links, external_links, structured_data_count,
            og_title, og_description, og_image, hreflang, content_hash, error, browser_result)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,
                 $27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37)
         ON CONFLICT (audit_id, url_hash) DO NOTHING",
    )
    .bind(p.id)
    .bind(audit_id)
    .bind(&p.url)
    .bind(&p.url_hash)
    .bind(&p.final_url)
    .bind(p.status_code)
    .bind(&p.fetch_class)
    .bind(&p.content_type)
    .bind(p.response_time_ms)
    .bind(p.size_bytes)
    .bind(&p.redirect_chain)
    .bind(p.depth)
    .bind(p.in_sitemap)
    .bind(&p.title)
    .bind(&p.meta_description)
    .bind(&p.meta_robots)
    .bind(&p.x_robots_tag)
    .bind(&p.canonical_url)
    .bind(&p.header_canonical_url)
    .bind(p.is_indexable)
    .bind(&p.lang)
    .bind(p.has_viewport)
    .bind(&p.h1)
    .bind(p.h1_count)
    .bind(p.word_count)
    .bind(p.images_total)
    .bind(p.images_missing_alt)
    .bind(p.internal_links)
    .bind(p.external_links)
    .bind(p.structured_data_count)
    .bind(&p.og_title)
    .bind(&p.og_description)
    .bind(&p.og_image)
    .bind(&p.hreflang)
    .bind(&p.content_hash)
    .bind(&p.error)
    .bind(&p.browser_result)
    .execute(&mut *tx)
    .await
    .map_err(internal)?
    .rows_affected()
        == 1;
    if !inserted {
        return Ok(false);
    }
    for chunk in links.chunks(500) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO audit_links (audit_id, from_page_id, to_url_hash, to_url, anchor, nofollow) ",
        );
        qb.push_values(chunk, |mut b, l| {
            b.push_bind(audit_id)
                .push_bind(p.id)
                .push_bind(&l.to_url_hash)
                .push_bind(&l.to_url)
                .push_bind(&l.anchor)
                .push_bind(l.nofollow);
        });
        qb.build().execute(&mut *tx).await.map_err(internal)?;
    }
    insert_issues(&mut tx, audit_id, issues).await?;
    tx.commit().await.map_err(internal)?;
    Ok(true)
}

async fn insert_issues(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    audit_id: Uuid,
    issues: &[NewIssue],
) -> AppResult<()> {
    for chunk in issues.chunks(500) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO audit_issues (id, audit_id, page_id, page_url, issue_type, severity, details) ",
        );
        qb.push_values(chunk, |mut b, i| {
            b.push_bind(common::new_id())
                .push_bind(audit_id)
                .push_bind(i.page_id)
                .push_bind(&i.page_url)
                .push_bind(&i.issue_type)
                .push_bind(&i.severity)
                .push_bind(&i.details);
        });
        qb.build().execute(&mut **tx).await.map_err(internal)?;
    }
    Ok(())
}

/// Replace all issues of the given types (site-level rules are recomputed on every finalize).
pub async fn replace_issues(
    pool: &PgPool,
    audit_id: Uuid,
    types: &[&str],
    issues: &[NewIssue],
) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("DELETE FROM audit_issues WHERE audit_id = $1 AND issue_type = ANY($2)")
        .bind(audit_id)
        .bind(types)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    insert_issues(&mut tx, audit_id, issues).await?;
    tx.commit().await.map_err(internal)?;
    Ok(())
}

/// Page summary used by the site-wide rules.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PageSummary {
    pub hreflang: serde_json::Value,
    pub id: Uuid,
    pub url: String,
    pub url_hash: String,
    pub status_code: Option<i32>,
    pub fetch_class: String,
    pub is_indexable: bool,
    pub in_sitemap: bool,
    pub redirected: bool,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub content_hash: Option<String>,
}

pub async fn page_summaries(pool: &PgPool, audit_id: Uuid) -> AppResult<Vec<PageSummary>> {
    sqlx::query_as(
        "SELECT id, url, url_hash, status_code, fetch_class, is_indexable, in_sitemap,
                jsonb_array_length(redirect_chain) > 0 AS redirected, title, meta_description, content_hash, hreflang
         FROM audit_pages WHERE audit_id = $1",
    )
    .bind(audit_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn link_edges(pool: &PgPool, audit_id: Uuid) -> AppResult<Vec<(Uuid, String)>> {
    sqlx::query_as("SELECT from_page_id, to_url_hash FROM audit_links WHERE audit_id = $1")
        .bind(audit_id)
        .fetch_all(pool)
        .await
        .map_err(internal)
}

/// Store inlink counts and BFS click depth (`-1` = not reachable by links).
pub async fn update_graph_metrics(
    pool: &PgPool,
    audit_id: Uuid,
    rows: &[(Uuid, i32, i32)],
) -> AppResult<()> {
    for chunk in rows.chunks(1000) {
        let ids: Vec<Uuid> = chunk.iter().map(|r| r.0).collect();
        let inlinks: Vec<i32> = chunk.iter().map(|r| r.1).collect();
        let depths: Vec<i32> = chunk.iter().map(|r| r.2).collect();
        sqlx::query(
            "UPDATE audit_pages p SET inlinks = v.inlinks, depth = v.depth
             FROM UNNEST($2::uuid[], $3::int[], $4::int[]) AS v(id, inlinks, depth)
             WHERE p.audit_id = $1 AND p.id = v.id",
        )
        .bind(audit_id)
        .bind(&ids)
        .bind(&inlinks)
        .bind(&depths)
        .execute(pool)
        .await
        .map_err(internal)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct IssueCount {
    pub issue_type: String,
    pub severity: String,
    /// Distinct pages affected.
    pub pages: i64,
}

pub async fn issue_counts(pool: &PgPool, audit_id: Uuid) -> AppResult<Vec<IssueCount>> {
    sqlx::query_as(
        "SELECT issue_type, severity, count(DISTINCT COALESCE(page_id::text, page_url)) AS pages
         FROM audit_issues WHERE audit_id = $1 GROUP BY issue_type, severity
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END, pages DESC",
    )
    .bind(audit_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

// ---------------------------------------------------------------- results reads

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct PageRow {
    pub id: Uuid,
    pub url: String,
    pub final_url: Option<String>,
    pub status_code: Option<i32>,
    pub fetch_class: String,
    pub response_time_ms: Option<i32>,
    pub depth: i32,
    pub in_sitemap: bool,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub word_count: i32,
    pub is_indexable: bool,
    pub canonical_url: Option<String>,
    pub inlinks: i32,
    pub internal_links: i32,
    pub external_links: i32,
    pub images_missing_alt: i32,
    pub issue_count: i64,
}

#[derive(Debug, Default)]
pub struct PageFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub issue_type: Option<String>,
    pub indexable: Option<bool>,
}

pub async fn pages(
    pool: &PgPool,
    audit_id: Uuid,
    f: &PageFilter,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<PageRow>, i64)> {
    // Built twice (count + page): QueryBuilder arguments cannot be cloned.
    let build = |head: &str, tail: &str| -> QueryBuilder<Postgres> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(head);
        qb.push(" WHERE p.audit_id = ").push_bind(audit_id);
        if let Some(s) = f.search.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND (p.url ILIKE ")
                .push_bind(format!("%{}%", escape_like(s)));
            qb.push(" OR p.title ILIKE ")
                .push_bind(format!("%{}%", escape_like(s)))
                .push(")");
        }
        match f.status.as_deref() {
            Some("2xx") => {
                qb.push(" AND p.status_code BETWEEN 200 AND 299");
            }
            Some("3xx") => {
                qb.push(" AND jsonb_array_length(p.redirect_chain) > 0");
            }
            Some("4xx") => {
                qb.push(" AND p.status_code BETWEEN 400 AND 499");
            }
            Some("5xx") => {
                qb.push(" AND p.status_code >= 500");
            }
            Some("error") => {
                qb.push(" AND p.status_code IS NULL");
            }
            _ => {}
        }
        if let Some(ix) = f.indexable {
            qb.push(" AND p.is_indexable = ").push_bind(ix);
        }
        if let Some(t) = f.issue_type.as_deref().filter(|t| !t.is_empty()) {
            qb.push(" AND EXISTS (SELECT 1 FROM audit_issues i WHERE i.page_id = p.id AND i.issue_type = ")
                .push_bind(t.to_string())
                .push(")");
        }
        qb.push(tail);
        qb
    };
    let total: i64 = build("SELECT count(*) FROM audit_pages p", "")
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(internal)?;
    let mut q = build(
        "SELECT p.id, p.url, p.final_url, p.status_code, p.fetch_class, p.response_time_ms, p.depth, p.in_sitemap,
                p.title, p.meta_description, p.h1, p.word_count, p.is_indexable, p.canonical_url, p.inlinks,
                p.internal_links, p.external_links, p.images_missing_alt,
                (SELECT count(*) FROM audit_issues i WHERE i.page_id = p.id) AS issue_count
         FROM audit_pages p",
        " ORDER BY p.depth < 0, p.depth, p.url LIMIT ",
    );
    q.push_bind(limit).push(" OFFSET ").push_bind(offset);
    let rows = q.build_query_as().fetch_all(pool).await.map_err(internal)?;
    Ok((rows, total))
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct PageDetail {
    pub browser_result: Option<serde_json::Value>,
    pub id: Uuid,
    pub url: String,
    pub final_url: Option<String>,
    pub status_code: Option<i32>,
    pub fetch_class: String,
    pub content_type: Option<String>,
    pub response_time_ms: Option<i32>,
    pub size_bytes: Option<i32>,
    pub redirect_chain: serde_json::Value,
    pub depth: i32,
    pub in_sitemap: bool,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub meta_robots: Option<String>,
    pub x_robots_tag: Option<String>,
    pub canonical_url: Option<String>,
    pub header_canonical_url: Option<String>,
    pub is_indexable: bool,
    pub lang: Option<String>,
    pub has_viewport: bool,
    pub h1: Option<String>,
    pub h1_count: i32,
    pub word_count: i32,
    pub images_total: i32,
    pub images_missing_alt: i32,
    pub internal_links: i32,
    pub external_links: i32,
    pub inlinks: i32,
    pub structured_data_count: i32,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub hreflang: serde_json::Value,
    pub error: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

pub async fn page_detail(pool: &PgPool, audit_id: Uuid, page_id: Uuid) -> AppResult<PageDetail> {
    sqlx::query_as(
        "SELECT id, url, final_url, status_code, fetch_class, content_type, response_time_ms, size_bytes,
                redirect_chain, depth, in_sitemap, title, meta_description, meta_robots, x_robots_tag,
                canonical_url, header_canonical_url, is_indexable, lang, has_viewport, h1, h1_count, word_count,
                images_total, images_missing_alt, internal_links, external_links, inlinks, structured_data_count,
                og_title, og_description, og_image, hreflang, error, fetched_at, browser_result
         FROM audit_pages WHERE audit_id = $1 AND id = $2",
    )
    .bind(audit_id)
    .bind(page_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("page"))
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LinkRow {
    pub url: String,
    pub anchor: Option<String>,
    pub nofollow: bool,
    pub status_code: Option<i32>,
}

/// Links out of a page (with the target's status when it was crawled).
pub async fn outlinks(pool: &PgPool, audit_id: Uuid, page_id: Uuid) -> AppResult<Vec<LinkRow>> {
    sqlx::query_as(
        "SELECT l.to_url AS url, l.anchor, l.nofollow, t.status_code
         FROM audit_links l LEFT JOIN audit_pages t ON t.audit_id = l.audit_id AND t.url_hash = l.to_url_hash
         WHERE l.audit_id = $1 AND l.from_page_id = $2 ORDER BY l.to_url LIMIT 1000",
    )
    .bind(audit_id)
    .bind(page_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// Pages linking to a page.
pub async fn inlinks(pool: &PgPool, audit_id: Uuid, page_id: Uuid) -> AppResult<Vec<LinkRow>> {
    sqlx::query_as(
        "SELECT s.url, l.anchor, l.nofollow, s.status_code
         FROM audit_pages t
         JOIN audit_links l ON l.audit_id = t.audit_id AND l.to_url_hash = t.url_hash
         JOIN audit_pages s ON s.id = l.from_page_id
         WHERE t.audit_id = $1 AND t.id = $2 AND s.id <> t.id ORDER BY s.url LIMIT 1000",
    )
    .bind(audit_id)
    .bind(page_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct IssueRow {
    pub id: Uuid,
    pub page_id: Option<Uuid>,
    pub page_url: Option<String>,
    pub issue_type: String,
    pub severity: String,
    pub details: serde_json::Value,
}

pub async fn issues(
    pool: &PgPool,
    audit_id: Uuid,
    issue_type: Option<&str>,
    page_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<IssueRow>, i64)> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_issues WHERE audit_id = $1
           AND ($2::text IS NULL OR issue_type = $2) AND ($3::uuid IS NULL OR page_id = $3)",
    )
    .bind(audit_id)
    .bind(issue_type)
    .bind(page_id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    let rows = sqlx::query_as(
        "SELECT id, page_id, page_url, issue_type, severity, details FROM audit_issues
         WHERE audit_id = $1 AND ($2::text IS NULL OR issue_type = $2) AND ($3::uuid IS NULL OR page_id = $3)
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END, issue_type, page_url
         LIMIT $4 OFFSET $5",
    )
    .bind(audit_id)
    .bind(issue_type)
    .bind(page_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok((rows, total))
}
