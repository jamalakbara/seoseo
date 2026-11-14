//! Private, project-scoped assistant conversations and atomic queue writes.
use crate::{internal, models::Project};
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AssistantChat {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub domain: Option<String>,
    pub country: String,
    pub language: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AssistantTurn {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub job_id: Uuid,
    pub model: String,
    #[serde(skip)]
    pub gateway_id: String,
    pub prompt: String,
    pub status: String,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub evidence: Value,
    pub request_started: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
) -> AppResult<Vec<AssistantChat>> {
    sqlx::query_as("SELECT c.* FROM assistant_chats c JOIN projects p ON p.id=c.project_id WHERE p.organization_id=$1 AND p.id=$2 AND c.user_id=$3 ORDER BY c.created_at DESC,c.id DESC LIMIT 20")
        .bind(org).bind(pid).bind(user).fetch_all(pool).await.map_err(internal)
}
pub async fn chat(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
    id: Uuid,
) -> AppResult<AssistantChat> {
    sqlx::query_as("SELECT c.* FROM assistant_chats c JOIN projects p ON p.id=c.project_id WHERE p.organization_id=$1 AND p.id=$2 AND c.user_id=$3 AND c.id=$4")
        .bind(org).bind(pid).bind(user).bind(id).fetch_optional(pool).await.map_err(internal)?.ok_or(AppError::NotFound("assistant chat"))
}
pub async fn create(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
    title: &str,
) -> AppResult<AssistantChat> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p: Project=sqlx::query_as("SELECT * FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE")
        .bind(org).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM assistant_chats WHERE project_id=$1 AND user_id=$2",
    )
    .bind(pid)
    .bind(user)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if count >= 20 {
        return Err(AppError::Conflict(
            "Delete an old chat before creating more (20 per project and user)".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO assistant_chats(id,project_id,user_id,title,domain,country,language) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *")
        .bind(new_id()).bind(pid).bind(user).bind(title).bind(p.domain).bind(p.country_code).bind(p.language_code).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}

pub async fn turns(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
    id: Uuid,
) -> AppResult<Vec<AssistantTurn>> {
    chat(pool, org, pid, user, id).await?;
    // A crashed single-attempt job must leave an actionable terminal turn.
    sqlx::query("UPDATE assistant_turns t SET status='failed',error='Worker stopped before the answer was saved. Send a new message to try again.',finished_at=now() FROM jobs j WHERE t.chat_id=$1 AND t.job_id=j.id AND t.status IN ('queued','running') AND j.status='failed'")
        .bind(id).execute(pool).await.map_err(internal)?;
    sqlx::query_as("SELECT * FROM assistant_turns WHERE chat_id=$1 ORDER BY created_at,id LIMIT 40")
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(internal)
}

pub struct NewTurn<'a> {
    pub org: Uuid,
    pub pid: Uuid,
    pub user: Uuid,
    pub chat: Uuid,
    pub model: &'a str,
    pub gateway: &'a str,
    pub prompt: &'a str,
}
pub async fn start(pool: &PgPool, input: NewTurn<'_>) -> AppResult<AssistantTurn> {
    let mut tx = pool.begin().await.map_err(internal)?;
    // Serialize the workspace-wide four-turn budget across different projects.
    sqlx::query("SELECT id FROM organizations WHERE id=$1 FOR UPDATE")
        .bind(input.org)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    sqlx::query("UPDATE assistant_turns t SET status='failed',error='Worker stopped before the answer was saved. Send a new message to try again.',finished_at=now() FROM jobs j,assistant_chats c,projects p WHERE t.job_id=j.id AND t.chat_id=c.id AND c.project_id=p.id AND p.organization_id=$1 AND t.status IN ('queued','running') AND j.status='failed'")
        .bind(input.org).execute(&mut *tx).await.map_err(internal)?;
    // Match lock order used by project edits, then serialize every chat write.
    let p: Project=sqlx::query_as("SELECT * FROM projects WHERE organization_id=$1 AND id=$2 AND archived_at IS NULL FOR UPDATE")
        .bind(input.org).bind(input.pid).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let c: AssistantChat = sqlx::query_as(
        "SELECT * FROM assistant_chats WHERE project_id=$1 AND user_id=$2 AND id=$3 FOR UPDATE",
    )
    .bind(input.pid)
    .bind(input.user)
    .bind(input.chat)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("assistant chat"))?;
    if c.domain != p.domain || c.country != p.country_code || c.language != p.language_code {
        return Err(AppError::Conflict(
            "Website or market changed. Start a new chat.".into(),
        ));
    }
    let (count, active): (i64,bool)=sqlx::query_as("SELECT count(*),COALESCE(bool_or(status IN ('queued','running')),false) FROM assistant_turns WHERE chat_id=$1")
        .bind(c.id).fetch_one(&mut *tx).await.map_err(internal)?;
    if active || count >= 40 {
        return Err(AppError::Conflict(
            "Wait for the current answer, or start a new chat after 40 turns.".into(),
        ));
    }
    // Count jobs, so deleting a running chat cannot free capacity before its request stops.
    let active_jobs: i64=sqlx::query_scalar("SELECT count(*) FROM jobs WHERE organization_id=$1 AND kind='assistant_turn' AND status IN ('queued','running')")
        .bind(input.org).fetch_one(&mut *tx).await.map_err(internal)?;
    if active_jobs >= 4 {
        return Err(AppError::Conflict(
            "Four assistant turns are already active in this workspace. Wait for an answer.".into(),
        ));
    }
    let id = new_id();
    let job = new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'assistant_turn',$2,$3,$4,1)")
        .bind(job).bind(input.org).bind(input.pid).bind(json!({"chat_id":c.id,"turn_id":id,"user_id":input.user})).execute(&mut *tx).await.map_err(internal)?;
    let row=sqlx::query_as("INSERT INTO assistant_turns(id,chat_id,job_id,model,gateway_id,prompt) VALUES($1,$2,$3,$4,$5,$6) RETURNING *")
        .bind(id).bind(c.id).bind(job).bind(input.model).bind(input.gateway).bind(input.prompt).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn delete(pool: &PgPool, org: Uuid, pid: Uuid, user: Uuid, id: Uuid) -> AppResult<()> {
    // Queued job payloads contain only IDs; deleting a chat removes prompts and answers.
    let result=sqlx::query("DELETE FROM assistant_chats c USING projects p WHERE c.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND c.user_id=$3 AND c.id=$4")
        .bind(org).bind(pid).bind(user).bind(id).execute(pool).await.map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("assistant chat"));
    }
    Ok(())
}
pub async fn claim(pool: &PgPool, id: Uuid, job: Uuid) -> AppResult<bool> {
    Ok(sqlx::query("UPDATE assistant_turns SET request_started=true,status='running' WHERE id=$1 AND job_id=$2 AND status='queued' AND NOT request_started")
        .bind(id).bind(job).execute(pool).await.map_err(internal)?.rows_affected()==1)
}
pub async fn evidence(pool: &PgPool, id: Uuid, value: Value) -> AppResult<()> {
    sqlx::query("UPDATE assistant_turns SET evidence=$2 WHERE id=$1 AND status='running'")
        .bind(id)
        .bind(value)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
pub async fn finish(pool: &PgPool, id: Uuid, result: Result<String, String>) -> AppResult<()> {
    let (status, answer, error) = match result {
        Ok(s) => ("completed", Some(s), None),
        Err(e) => ("failed", None, Some(e)),
    };
    sqlx::query("UPDATE assistant_turns SET status=$2,answer=$3,error=$4,finished_at=now() WHERE id=$1 AND status IN ('queued','running')")
        .bind(id).bind(status).bind(answer).bind(error).execute(pool).await.map_err(internal)?;
    Ok(())
}
