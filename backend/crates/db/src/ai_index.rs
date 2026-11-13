use crate::internal;
use chrono::{DateTime, Utc};
use common::AppResult;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! cells_sql {()=>{"WITH runs AS (
 SELECT r.* FROM ai_visibility_runs r JOIN ai_visibility_trackers t ON t.id=r.tracker_id JOIN projects p ON p.id=t.project_id
 WHERE p.organization_id=$1 AND p.id=$2 AND t.domain=p.domain::text AND r.config->>'country'=$3 AND r.config->>'language'=$4
), expected AS (
 SELECT r.id AS run_id,r.tracker_id,r.gateway_id,q.value AS prompt,(q.ordinality-1)::integer AS prompt_index,m.value AS model
 FROM runs r CROSS JOIN LATERAL jsonb_array_elements_text(r.config->'prompts') WITH ORDINALITY q
 CROSS JOIN LATERAL jsonb_array_elements_text(r.config->'models') m
), cells AS (
 SELECT e.*,a.answer,a.created_at AS answered_at,a.engine,a.grounding,
 (a.answer IS NOT NULL AND a.error IS NULL) AS successful,(a.error IS NOT NULL) AS failed,
 (a.answer IS NULL AND a.error IS NULL) AS unchecked,COALESCE(a.answer ~* $6,false) AND a.error IS NULL AS matched
 FROM expected e LEFT JOIN ai_visibility_answers a ON a.run_id=e.run_id AND a.prompt_index=e.prompt_index AND a.model=e.model
 WHERE ($5::text IS NULL OR e.prompt ILIKE $5)
) "};}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiIndexSummary {
    pub retained_runs: i64,
    pub unique_prompts: i64,
    pub expected_answers: i64,
    pub successful_answers: i64,
    pub failed_answers: i64,
    pub unchecked_answers: i64,
    /// Null when no brand term was requested.
    pub brand_mentions: Option<i64>,
}
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct IndexedAiPrompt {
    pub prompt: String,
    pub retained_runs: i64,
    pub expected_answers: i64,
    pub successful_answers: i64,
    pub failed_answers: i64,
    pub unchecked_answers: i64,
    pub brand_mentions: Option<i64>,
}
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct IndexedAiAnswer {
    pub tracker_id: Uuid,
    pub run_id: Uuid,
    pub prompt_index: i32,
    pub prompt: String,
    pub model: String,
    pub engine: String,
    pub grounding: String,
    pub gateway_id: String,
    pub answered_at: DateTime<Utc>,
    pub excerpt: String,
}
pub struct IndexEvidence {
    pub summary: AiIndexSummary,
    pub total_matching: i64,
    pub prompts: Vec<IndexedAiPrompt>,
    pub answers: Vec<IndexedAiAnswer>,
}
pub struct IndexFilter<'a> {
    pub org: Uuid,
    pub pid: Uuid,
    pub country: &'a str,
    pub language: &'a str,
    pub topic: Option<&'a str>,
    pub pattern: Option<&'a str>,
    pub view: &'a str,
    pub limit: i64,
    pub offset: i64,
}

pub async fn evidence(pool: &PgPool, f: IndexFilter<'_>) -> AppResult<IndexEvidence> {
    let topic = f.topic.map(|s| format!("%{}%", crate::escape_like(s)));
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let summary=sqlx::query_as(concat!(cells_sql!(),"SELECT count(DISTINCT run_id) AS retained_runs,count(DISTINCT prompt) AS unique_prompts,count(*) AS expected_answers,
     count(*) FILTER(WHERE successful) AS successful_answers,count(*) FILTER(WHERE failed) AS failed_answers,count(*) FILTER(WHERE unchecked) AS unchecked_answers,
     CASE WHEN $6::text IS NULL THEN NULL ELSE count(*) FILTER(WHERE matched) END AS brand_mentions FROM cells"))
        .bind(f.org).bind(f.pid).bind(f.country).bind(f.language).bind(&topic).bind(f.pattern).fetch_one(&mut *tx).await.map_err(internal)?;
    let mut prompts = Vec::new();
    let mut answers = Vec::new();
    let total_matching;
    if f.view == "prompts" {
        total_matching=sqlx::query_scalar(concat!(cells_sql!(),"SELECT count(*) FROM (SELECT prompt FROM cells GROUP BY prompt HAVING $6::text IS NULL OR bool_or(matched)) grouped"))
            .bind(f.org).bind(f.pid).bind(f.country).bind(f.language).bind(&topic).bind(f.pattern).fetch_one(&mut *tx).await.map_err(internal)?;
        prompts=sqlx::query_as(concat!(cells_sql!(),"SELECT prompt,count(DISTINCT run_id) AS retained_runs,count(*) AS expected_answers,
         count(*) FILTER(WHERE successful) AS successful_answers,count(*) FILTER(WHERE failed) AS failed_answers,count(*) FILTER(WHERE unchecked) AS unchecked_answers,
         CASE WHEN $6::text IS NULL THEN NULL ELSE count(*) FILTER(WHERE matched) END AS brand_mentions
         FROM cells GROUP BY prompt HAVING $6::text IS NULL OR bool_or(matched) ORDER BY prompt LIMIT $7 OFFSET $8"))
            .bind(f.org).bind(f.pid).bind(f.country).bind(f.language).bind(&topic).bind(f.pattern).bind(f.limit).bind(f.offset).fetch_all(&mut *tx).await.map_err(internal)?;
    } else {
        total_matching = sqlx::query_scalar(concat!(
            cells_sql!(),
            "SELECT count(*) FROM cells WHERE successful AND ($6::text IS NULL OR matched)"
        ))
        .bind(f.org)
        .bind(f.pid)
        .bind(f.country)
        .bind(f.language)
        .bind(&topic)
        .bind(f.pattern)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
        answers=sqlx::query_as(concat!(cells_sql!(),"SELECT tracker_id,run_id,prompt_index,prompt,model,engine,grounding,gateway_id,answered_at,
         CASE WHEN $6::text IS NULL THEN left(answer,500) ELSE substring(answer FROM greatest(1,regexp_instr(answer,$6,1,1,0,'i')-100) FOR 500) END AS excerpt
         FROM cells WHERE successful AND ($6::text IS NULL OR matched) ORDER BY answered_at DESC,run_id,model,prompt_index LIMIT $7 OFFSET $8"))
            .bind(f.org).bind(f.pid).bind(f.country).bind(f.language).bind(&topic).bind(f.pattern).bind(f.limit).bind(f.offset).fetch_all(&mut *tx).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(IndexEvidence {
        summary,
        total_matching,
        prompts,
        answers,
    })
}
