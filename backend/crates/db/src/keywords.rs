//! Keyword research runs, their ideas, and a project's saved keywords.
//! Read queries take the project id so callers stay tenant-scoped.

use crate::{escape_like, internal};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! research_columns {
    () => {
        "id, project_id, started_by, job_id, seed, country, language, status, phase, config, \
         ideas_total, warnings, error, created_at, finished_at"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct KeywordResearch {
    pub id: Uuid,
    pub project_id: Uuid,
    pub started_by: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub seed: String,
    pub country: String,
    pub language: String,
    /// queued | running | completed | failed
    pub status: String,
    /// queued | ideas | demand | serp | done
    pub phase: String,
    pub config: serde_json::Value,
    pub ideas_total: i32,
    /// Human-readable notes: skipped or rate-limited providers, missing setup.
    #[schema(value_type = Vec<String>)]
    pub warnings: serde_json::Value,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_research(
    pool: &PgPool,
    id: Uuid,
    project_id: Uuid,
    started_by: Uuid,
    seed: &str,
    country: &str,
    language: &str,
    config: serde_json::Value,
) -> AppResult<KeywordResearch> {
    sqlx::query_as(concat!(
        "INSERT INTO keyword_researches (id, project_id, started_by, seed, country, language, config)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING ",
        research_columns!()
    ))
    .bind(id)
    .bind(project_id)
    .bind(started_by)
    .bind(seed)
    .bind(country)
    .bind(language)
    .bind(config)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn set_research_job(pool: &PgPool, id: Uuid, job_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE keyword_researches SET job_id = $2 WHERE id = $1")
        .bind(id)
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Unscoped read for the worker (the job payload carries the id).
pub async fn get_research_any(pool: &PgPool, id: Uuid) -> AppResult<KeywordResearch> {
    sqlx::query_as(concat!(
        "SELECT ",
        research_columns!(),
        " FROM keyword_researches WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("keyword research"))
}

pub async fn get_research(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<KeywordResearch> {
    sqlx::query_as(concat!(
        "SELECT ",
        research_columns!(),
        " FROM keyword_researches WHERE id = $1 AND project_id = $2"
    ))
    .bind(id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("keyword research"))
}

pub async fn list_researches(
    pool: &PgPool,
    project_id: Uuid,
    limit: i64,
) -> AppResult<Vec<KeywordResearch>> {
    sqlx::query_as(concat!(
        "SELECT ",
        research_columns!(),
        " FROM keyword_researches WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2"
    ))
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn count_active_researches(pool: &PgPool, project_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM keyword_researches
         WHERE project_id = $1 AND status IN ('queued', 'running')",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(internal)
}

pub async fn delete_research(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let r = sqlx::query("DELETE FROM keyword_researches WHERE id = $1 AND project_id = $2")
        .bind(id)
        .bind(project_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("keyword research"));
    }
    Ok(())
}

pub async fn set_research_phase(pool: &PgPool, id: Uuid, phase: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_researches SET phase = $2, updated_at = now(),
                status = CASE WHEN status = 'queued' THEN 'running' ELSE status END,
                ideas_total = (SELECT count(*) FROM keyword_ideas i WHERE i.research_id = $1)
         WHERE id = $1",
    )
    .bind(id)
    .bind(phase)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Append a note for the user (deduplicated).
pub async fn add_research_warning(pool: &PgPool, id: Uuid, warning: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_researches SET warnings = warnings || to_jsonb($2::text), updated_at = now()
         WHERE id = $1 AND NOT warnings @> to_jsonb($2::text)",
    )
    .bind(id)
    .bind(warning)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn finish_research(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_researches SET status = 'completed', phase = 'done', finished_at = now(),
                updated_at = now(),
                ideas_total = (SELECT count(*) FROM keyword_ideas i WHERE i.research_id = $1)
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub async fn fail_research(pool: &PgPool, id: Uuid, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_researches SET status = 'failed', error = left($2, 1000),
                finished_at = now(), updated_at = now()
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ---------------------------------------------------------------- ideas

macro_rules! idea_columns {
    () => {
        "id, research_id, keyword, sources, word_count, popularity, est_volume, volume_bucket, \
         volume_source, volume_confidence, trend, kd, kd_source, intent, serp_features"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct KeywordIdea {
    pub id: Uuid,
    pub research_id: Uuid,
    pub keyword: String,
    /// Where the idea came from, e.g. `suggest_google`, `suggest_bing`, `related`, `question`.
    pub sources: Vec<String>,
    pub word_count: i32,
    /// Relative popularity within this research (0–100). Not a search volume.
    pub popularity: i32,
    /// Estimated monthly searches; null when there is no basis for an estimate.
    pub est_volume: Option<i64>,
    pub volume_bucket: Option<String>,
    pub volume_source: Option<String>,
    /// measured | estimated | relative
    pub volume_confidence: Option<String>,
    /// 12 months of `{ month, views }` when a demand signal exists.
    pub trend: Option<serde_json::Value>,
    /// Keyword difficulty 0–100; null until a SERP was analysed.
    pub kd: Option<i32>,
    pub kd_source: Option<String>,
    /// informational | commercial | transactional | navigational
    pub intent: String,
    /// Null until a SERP was analysed.
    pub serp_features: Option<Vec<String>>,
}

pub struct NewIdea {
    pub id: Uuid,
    pub keyword: String,
    pub sources: Vec<String>,
    pub popularity: i32,
    pub intent: String,
}

/// Insert ideas; a keyword already present keeps its row (resume-safe).
pub async fn insert_ideas(pool: &PgPool, research_id: Uuid, rows: &[NewIdea]) -> AppResult<()> {
    for chunk in rows.chunks(500) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO keyword_ideas (id, research_id, keyword, sources, word_count, popularity, intent) ",
        );
        qb.push_values(chunk, |mut b, r| {
            b.push_bind(r.id)
                .push_bind(research_id)
                .push_bind(&r.keyword)
                .push_bind(&r.sources)
                .push_bind(r.keyword.split_whitespace().count() as i32)
                .push_bind(r.popularity)
                .push_bind(&r.intent);
        });
        qb.push(" ON CONFLICT (research_id, keyword) DO NOTHING");
        qb.build().execute(pool).await.map_err(internal)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct IdeaFilter {
    pub search: Option<String>,
    pub intent: Option<String>,
    pub source: Option<String>,
    pub min_popularity: Option<i32>,
    pub max_kd: Option<i32>,
    pub min_volume: Option<i64>,
    pub min_words: Option<i32>,
    pub max_words: Option<i32>,
    /// Only question-style ideas.
    pub questions: Option<bool>,
}

/// Whitelisted `ORDER BY` for idea and saved-keyword lists; unknown metrics sort last.
fn order_by(sort: Option<&str>, desc: Option<bool>, default: &str) -> String {
    let (col, default_desc) = match sort.unwrap_or(default) {
        "keyword" => ("keyword", false),
        "est_volume" | "volume" => ("est_volume", true),
        "kd" => ("kd", false),
        "word_count" | "words" => ("word_count", false),
        "created_at" => ("created_at", true),
        _ => ("popularity", true),
    };
    let dir = if desc.unwrap_or(default_desc) {
        "DESC"
    } else {
        "ASC"
    };
    format!(" ORDER BY {col} {dir} NULLS LAST, keyword ASC")
}

pub async fn ideas(
    pool: &PgPool,
    research_id: Uuid,
    f: &IdeaFilter,
    sort: Option<&str>,
    desc: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<KeywordIdea>, i64)> {
    let build = |head: &str| -> QueryBuilder<Postgres> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(head);
        qb.push(" WHERE research_id = ").push_bind(research_id);
        if let Some(s) = f.search.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND keyword ILIKE ")
                .push_bind(format!("%{}%", escape_like(s)));
        }
        if let Some(i) = f.intent.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND intent = ").push_bind(i.to_string());
        }
        if let Some(s) = f.source.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND ")
                .push_bind(s.to_string())
                .push(" = ANY(sources)");
        }
        if f.questions == Some(true) {
            qb.push(" AND 'question' = ANY(sources)");
        }
        if let Some(v) = f.min_popularity {
            qb.push(" AND popularity >= ").push_bind(v);
        }
        if let Some(v) = f.max_kd {
            qb.push(" AND kd <= ").push_bind(v);
        }
        if let Some(v) = f.min_volume {
            qb.push(" AND est_volume >= ").push_bind(v);
        }
        if let Some(v) = f.min_words {
            qb.push(" AND word_count >= ").push_bind(v);
        }
        if let Some(v) = f.max_words {
            qb.push(" AND word_count <= ").push_bind(v);
        }
        qb
    };
    let total: i64 = build("SELECT count(*) FROM keyword_ideas")
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(internal)?;
    let mut q = build(concat!("SELECT ", idea_columns!(), " FROM keyword_ideas"));
    q.push(order_by(sort, desc, "popularity"));
    q.push(" LIMIT ").push_bind(limit);
    q.push(" OFFSET ").push_bind(offset);
    let rows = q.build_query_as().fetch_all(pool).await.map_err(internal)?;
    Ok((rows, total))
}

/// Ideas of a research in popularity order (for the worker).
pub async fn top_ideas(
    pool: &PgPool,
    research_id: Uuid,
    limit: i64,
) -> AppResult<Vec<KeywordIdea>> {
    sqlx::query_as(concat!(
        "SELECT ",
        idea_columns!(),
        " FROM keyword_ideas WHERE research_id = $1 ORDER BY popularity DESC, keyword LIMIT $2"
    ))
    .bind(research_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub struct VolumeUpdate<'a> {
    pub est_volume: i64,
    pub bucket: &'a str,
    pub source: &'a str,
    pub confidence: &'a str,
    pub trend: serde_json::Value,
}

pub async fn set_idea_volume(pool: &PgPool, idea_id: Uuid, v: &VolumeUpdate<'_>) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_ideas SET est_volume = $2, volume_bucket = $3, volume_source = $4,
                volume_confidence = $5, trend = $6 WHERE id = $1",
    )
    .bind(idea_id)
    .bind(v.est_volume)
    .bind(v.bucket)
    .bind(v.source)
    .bind(v.confidence)
    .bind(&v.trend)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

pub struct SerpUpdate<'a> {
    pub kd: Option<i32>,
    pub kd_source: Option<&'a str>,
    pub intent: &'a str,
    pub features: &'a [String],
    pub snapshot_id: Uuid,
}

pub async fn set_idea_serp(pool: &PgPool, idea_id: Uuid, s: &SerpUpdate<'_>) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_ideas SET kd = $2, kd_source = $3, intent = $4, serp_features = $5,
                serp_snapshot_id = $6 WHERE id = $1",
    )
    .bind(idea_id)
    .bind(s.kd)
    .bind(s.kd_source)
    .bind(s.intent)
    .bind(s.features)
    .bind(s.snapshot_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ---------------------------------------------------------------- saved keywords

macro_rules! saved_columns {
    () => {
        "id, project_id, keyword, country, language, tags, popularity, est_volume, volume_bucket, \
         volume_source, volume_confidence, trend, kd, kd_source, intent, serp_features, \
         metrics_updated_at, created_at"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SavedKeyword {
    pub id: Uuid,
    pub project_id: Uuid,
    pub keyword: String,
    pub country: String,
    pub language: String,
    pub tags: Vec<String>,
    /// Relative popularity from the research it was saved from; null for manual entries.
    pub popularity: Option<i32>,
    pub est_volume: Option<i64>,
    pub volume_bucket: Option<String>,
    pub volume_source: Option<String>,
    pub volume_confidence: Option<String>,
    pub trend: Option<serde_json::Value>,
    pub kd: Option<i32>,
    pub kd_source: Option<String>,
    pub intent: Option<String>,
    pub serp_features: Option<Vec<String>>,
    pub metrics_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Save ideas of a research (copying their metrics). Returns how many were newly saved.
pub async fn save_ideas(
    pool: &PgPool,
    project_id: Uuid,
    research_id: Uuid,
    idea_ids: &[Uuid],
    tags: &[String],
    created_by: Uuid,
) -> AppResult<u64> {
    let r = sqlx::query(
        "INSERT INTO saved_keywords (id, project_id, keyword, country, language, tags, popularity,
                est_volume, volume_bucket, volume_source, volume_confidence, trend, kd, kd_source,
                intent, serp_features, metrics_updated_at, created_by)
         SELECT gen_random_uuid(), r.project_id, i.keyword, r.country, r.language, $4, i.popularity,
                i.est_volume, i.volume_bucket, i.volume_source, i.volume_confidence, i.trend, i.kd,
                i.kd_source, i.intent, i.serp_features, now(), $5
         FROM keyword_ideas i JOIN keyword_researches r ON r.id = i.research_id
         WHERE r.id = $2 AND r.project_id = $1 AND i.id = ANY($3)
         ON CONFLICT (project_id, keyword, country, language) DO NOTHING",
    )
    .bind(project_id)
    .bind(research_id)
    .bind(idea_ids)
    .bind(tags)
    .bind(created_by)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(r.rows_affected())
}

pub struct NewSaved<'a> {
    pub keyword: &'a str,
    pub intent: &'a str,
}

/// Save keywords typed by the user (no metrics yet). Returns how many were newly saved.
pub async fn save_manual(
    pool: &PgPool,
    project_id: Uuid,
    country: &str,
    language: &str,
    rows: &[NewSaved<'_>],
    tags: &[String],
    created_by: Uuid,
) -> AppResult<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO saved_keywords (id, project_id, keyword, country, language, tags, intent, created_by) ",
    );
    qb.push_values(rows, |mut b, r| {
        b.push_bind(common::new_id())
            .push_bind(project_id)
            .push_bind(r.keyword)
            .push_bind(country)
            .push_bind(language)
            .push_bind(tags)
            .push_bind(r.intent)
            .push_bind(created_by);
    });
    qb.push(" ON CONFLICT (project_id, keyword, country, language) DO NOTHING");
    let r = qb.build().execute(pool).await.map_err(internal)?;
    Ok(r.rows_affected())
}

pub async fn count_saved(pool: &PgPool, project_id: Uuid) -> AppResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM saved_keywords WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(internal)
}

#[derive(Debug, Default)]
pub struct SavedFilter {
    pub search: Option<String>,
    pub tag: Option<String>,
    pub intent: Option<String>,
}

pub async fn saved(
    pool: &PgPool,
    project_id: Uuid,
    f: &SavedFilter,
    sort: Option<&str>,
    desc: Option<bool>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<SavedKeyword>, i64)> {
    let build = |head: &str| -> QueryBuilder<Postgres> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(head);
        qb.push(" WHERE project_id = ").push_bind(project_id);
        if let Some(s) = f.search.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND keyword ILIKE ")
                .push_bind(format!("%{}%", escape_like(s)));
        }
        if let Some(t) = f.tag.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND ")
                .push_bind(t.to_string())
                .push(" = ANY(tags)");
        }
        if let Some(i) = f.intent.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND intent = ").push_bind(i.to_string());
        }
        qb
    };
    let total: i64 = build("SELECT count(*) FROM saved_keywords")
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(internal)?;
    let mut q = build(concat!("SELECT ", saved_columns!(), " FROM saved_keywords"));
    q.push(order_by(sort, desc, "created_at"));
    q.push(" LIMIT ").push_bind(limit);
    q.push(" OFFSET ").push_bind(offset);
    let rows = q.build_query_as().fetch_all(pool).await.map_err(internal)?;
    Ok((rows, total))
}

pub async fn saved_tags(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT t FROM saved_keywords, unnest(tags) AS t WHERE project_id = $1 ORDER BY t",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

pub async fn get_saved(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<SavedKeyword> {
    sqlx::query_as(concat!(
        "SELECT ",
        saved_columns!(),
        " FROM saved_keywords WHERE id = $1 AND project_id = $2"
    ))
    .bind(id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("saved keyword"))
}

pub async fn set_saved_tags(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
    tags: &[String],
) -> AppResult<SavedKeyword> {
    sqlx::query_as(concat!(
        "UPDATE saved_keywords SET tags = $3 WHERE id = $1 AND project_id = $2 RETURNING ",
        saved_columns!()
    ))
    .bind(id)
    .bind(project_id)
    .bind(tags)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("saved keyword"))
}

pub async fn delete_saved(pool: &PgPool, project_id: Uuid, ids: &[Uuid]) -> AppResult<u64> {
    let r = sqlx::query("DELETE FROM saved_keywords WHERE project_id = $1 AND id = ANY($2)")
        .bind(project_id)
        .bind(ids)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(r.rows_affected())
}

/// Store SERP-derived metrics on a saved keyword after an on-demand analysis.
pub async fn set_saved_serp(
    pool: &PgPool,
    project_id: Uuid,
    keyword: &str,
    country: &str,
    language: &str,
    s: &SerpUpdate<'_>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE saved_keywords SET kd = $5, kd_source = $6, intent = $7, serp_features = $8,
                metrics_updated_at = now()
         WHERE project_id = $1 AND keyword = $2 AND country = $3 AND language = $4",
    )
    .bind(project_id)
    .bind(keyword)
    .bind(country)
    .bind(language)
    .bind(s.kd)
    .bind(s.kd_source)
    .bind(s.intent)
    .bind(s.features)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Store SERP-derived metrics on this project's research ideas for the same keyword and
/// market, so an on-demand analysis shows up in research tables too.
pub async fn set_project_ideas_serp(
    pool: &PgPool,
    project_id: Uuid,
    keyword: &str,
    country: &str,
    language: &str,
    s: &SerpUpdate<'_>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE keyword_ideas i SET kd = $5, kd_source = $6, intent = $7, serp_features = $8,
                serp_snapshot_id = $9
         FROM keyword_researches r
         WHERE r.id = i.research_id AND r.project_id = $1 AND i.keyword = $2
           AND r.country = $3 AND r.language = $4",
    )
    .bind(project_id)
    .bind(keyword)
    .bind(country)
    .bind(language)
    .bind(s.kd)
    .bind(s.kd_source)
    .bind(s.intent)
    .bind(s.features)
    .bind(s.snapshot_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Selection is tenant scoped by the caller and frozen when the refresh is enqueued.
pub async fn saved_by_id(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<Option<SavedKeyword>> {
    sqlx::query_as("SELECT * FROM saved_keywords WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}

pub async fn enqueue_refresh(
    pool: &PgPool,
    org_id: Uuid,
    project_id: Uuid,
    ids: &[Uuid],
) -> AppResult<crate::models::Job> {
    let mut tx = pool.begin().await.map_err(internal)?;
    // Serialize concurrent requests so only one active refresh exists per project.
    sqlx::query("SELECT id FROM projects WHERE id = $1 FOR UPDATE")
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jobs WHERE project_id = $1 AND kind = 'saved_keyword_refresh' AND status IN ('queued','running'))")
        .bind(project_id).fetch_one(&mut *tx).await.map_err(internal)?;
    if active {
        return Err(common::AppError::Conflict(
            "a saved-keyword refresh is already running".into(),
        ));
    }
    let selected: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM saved_keywords WHERE project_id = $1 AND (cardinality($2::uuid[]) = 0 OR id = ANY($2)) ORDER BY id")
        .bind(project_id).bind(ids).fetch_all(&mut *tx).await.map_err(internal)?;
    if selected.is_empty() {
        return Err(common::AppError::Validation(
            "no saved keywords to refresh".into(),
        ));
    }
    if !ids.is_empty() && selected.len() != ids.len() {
        return Err(common::AppError::Validation(
            "selection contains unknown saved keywords".into(),
        ));
    }
    let job = sqlx::query_as("INSERT INTO jobs (id, kind, organization_id, project_id, payload, max_attempts) VALUES ($1, 'saved_keyword_refresh', $2, $3, $4, 3) RETURNING id, kind, organization_id, project_id, payload, status, attempts, max_attempts, last_error, progress, run_at, created_at, finished_at")
        .bind(common::new_id()).bind(org_id).bind(project_id).bind(serde_json::json!({"ids": selected}))
        .fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(job)
}

/// A successful demand lookup may have no matching article; clear old estimates in that case.
pub async fn set_saved_volume(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
    v: Option<&VolumeUpdate<'_>>,
) -> AppResult<()> {
    sqlx::query("UPDATE saved_keywords SET est_volume = $3, volume_bucket = $4, volume_source = $5, volume_confidence = $6, trend = $7 WHERE project_id = $1 AND id = $2")
        .bind(project_id).bind(id).bind(v.map(|v| v.est_volume)).bind(v.map(|v| v.bucket))
        .bind(v.map(|v| v.source)).bind(v.map(|v| v.confidence)).bind(v.map(|v| &v.trend))
        .execute(pool).await.map_err(internal)?;
    Ok(())
}
