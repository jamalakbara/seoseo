use crate::internal;
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ResearchObservation {
    pub id: Uuid,
    pub project_id: Uuid,
    pub domain: String,
    pub country: String,
    pub language: String,
    pub kind: String,
    pub data: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn save(
    pool: &PgPool,
    p: &crate::models::Project,
    kind: &str,
    data: Value,
) -> AppResult<ResearchObservation> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let current: (Option<String>, String, String) = sqlx::query_as(
        "SELECT domain::text,country_code,language_code FROM projects WHERE id=$1 FOR UPDATE",
    )
    .bind(p.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if current
        != (
            p.domain.clone(),
            p.country_code.clone(),
            p.language_code.clone(),
        )
    {
        return Err(AppError::Conflict(
            "project website or market changed; reload before saving".into(),
        ));
    }
    let result = sqlx::query_as("INSERT INTO m6_observations(id,project_id,domain,country,language,kind,data) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *")
        .bind(new_id()).bind(p.id).bind(&p.domain).bind(&p.country_code).bind(&p.language_code).bind(kind).bind(data)
        .fetch_one(&mut *tx).await.map_err(internal)?;
    // Bound retained evidence, including prior website identities.
    sqlx::query("DELETE FROM m6_observations WHERE project_id=$1 AND kind=$2 AND id NOT IN (SELECT id FROM m6_observations WHERE project_id=$1 AND kind=$2 ORDER BY created_at DESC,id DESC LIMIT 100)")
        .bind(p.id).bind(kind).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(result)
}
pub async fn list(
    pool: &PgPool,
    p: &crate::models::Project,
    kind: &str,
) -> AppResult<Vec<ResearchObservation>> {
    sqlx::query_as("SELECT * FROM m6_observations WHERE project_id=$1 AND domain=$2 AND country=$3 AND language=$4 AND kind=$5 ORDER BY created_at DESC,id DESC LIMIT 100")
        .bind(p.id).bind(&p.domain).bind(&p.country_code).bind(&p.language_code).bind(kind)
        .fetch_all(pool).await.map_err(internal)
}
