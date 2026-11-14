//! Stored business workspace, report evidence and grace-period account lifecycle.
use crate::{internal, models::Project};
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct BusinessProfile {
    pub project_id: Uuid,
    pub revision: i64,
    pub data: Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct BusinessEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub data: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revision: i64,
}
pub async fn profile(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Option<BusinessProfile>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT * FROM business_profiles WHERE project_id=$1")
        .bind(pid)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}
pub async fn save_profile(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    revision: i64,
    data: Value,
) -> AppResult<BusinessProfile> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let _:Uuid=sqlx::query_scalar("SELECT id FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let rev: i64 = sqlx::query_scalar("SELECT revision FROM business_profiles WHERE project_id=$1")
        .bind(pid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .unwrap_or(0);
    if revision != rev {
        return Err(AppError::Conflict(
            "Business profile changed; reload".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO business_profiles(project_id,data) VALUES($1,$2) ON CONFLICT(project_id) DO UPDATE SET revision=business_profiles.revision+1,data=excluded.data,updated_at=now() RETURNING *").bind(pid).bind(data).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn business_entries(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<BusinessEntry>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT * FROM business_entries WHERE project_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3").bind(pid).bind(limit).bind(offset).fetch_all(pool).await.map_err(internal)
}
pub async fn save_business_entry(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    id: Option<Uuid>,
    revision: i64,
    kind: &str,
    data: Value,
) -> AppResult<BusinessEntry> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let _:Uuid=sqlx::query_scalar("SELECT id FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let row = if let Some(id) = id {
        sqlx::query_as("UPDATE business_entries SET data=$4,revision=revision+1 WHERE id=$1 AND project_id=$2 AND revision=$3 AND kind=$5 RETURNING *").bind(id).bind(pid).bind(revision).bind(data).bind(kind).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::Conflict("Entry changed or missing; reload".into()))?
    } else {
        if revision != 0 {
            return Err(AppError::Validation(
                "New entry revision must be zero".into(),
            ));
        }
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM business_entries WHERE project_id=$1")
                .bind(pid)
                .fetch_one(&mut *tx)
                .await
                .map_err(internal)?;
        if count >= 1000 {
            return Err(AppError::Conflict(
                "At most 1000 business records per project; remove an old record".into(),
            ));
        }
        sqlx::query_as(
            "INSERT INTO business_entries(id,project_id,kind,data) VALUES($1,$2,$3,$4) RETURNING *",
        )
        .bind(new_id())
        .bind(pid)
        .bind(kind)
        .bind(data)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?
    };
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn delete_business_entry(pool: &PgPool, org: Uuid, pid: Uuid, id: Uuid) -> AppResult<()> {
    let r=sqlx::query("DELETE FROM business_entries e USING projects p WHERE e.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND e.id=$3").bind(org).bind(pid).bind(id).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("business record"));
    }
    Ok(())
}
pub async fn report_evidence(pool: &PgPool, p: &Project, kind: &str) -> AppResult<Value> {
    let sql = match kind {
        "audit" => {
            "SELECT to_jsonb(a) FROM (SELECT id,start_url,status,pages_crawled,pages_failed,health_score,issue_counts,stop_reason,finished_at FROM audits WHERE project_id=$1 AND lower(split_part(split_part(split_part(start_url,'://',2),'/',1),':',1))=$2 AND status='completed' ORDER BY created_at DESC LIMIT 1) a"
        }
        "rank" => {
            "SELECT jsonb_build_object('sample_limit',200,'rows',COALESCE(jsonb_agg(q),'[]'::jsonb)) FROM (SELECT t.name,t.domain,t.country,t.language,k.keyword,r.position,r.depth,r.provider,r.exact,r.checked_at FROM rank_trackers t JOIN LATERAL(SELECT id FROM rank_runs WHERE tracker_id=t.id AND status='completed' ORDER BY created_at DESC,id DESC LIMIT 1) run ON true JOIN rank_results r ON r.run_id=run.id JOIN rank_tracker_keywords k ON k.id=r.keyword_id WHERE t.project_id=$1 AND t.domain=$2 AND t.country=$3 AND t.language=$4 ORDER BY t.name,k.keyword LIMIT 200) q"
        }
        "keywords" => {
            "SELECT jsonb_build_object('sample_limit',200,'rows',COALESCE(jsonb_agg(q),'[]'::jsonb)) FROM (SELECT keyword,country,language,est_volume,volume_source,volume_confidence,kd,kd_source,intent,tags FROM saved_keywords WHERE project_id=$1 AND country=$3 AND language=$4 AND $2::text IS NOT NULL ORDER BY keyword LIMIT 200) q"
        }
        "links" => {
            "SELECT jsonb_build_object('sample_limit',100,'rows',COALESCE(jsonb_agg(q),'[]'::jsonb)) FROM (SELECT dataset,domain,live_count,new_count,lost_count,created_at FROM backlink_snapshots WHERE project_id=$1 AND domain=$2 AND $3::text IS NOT NULL AND $4::text IS NOT NULL ORDER BY created_at DESC,id DESC LIMIT 100) q"
        }
        "ai" => {
            "SELECT jsonb_build_object('sample_limit',100,'rows',COALESCE(jsonb_agg(q),'[]'::jsonb)) FROM (SELECT t.name,t.domain,r.id,r.status,r.error,r.created_at,r.finished_at,r.config FROM ai_visibility_runs r JOIN ai_visibility_trackers t ON t.id=r.tracker_id WHERE t.project_id=$1 AND t.domain=$2 AND r.config->>'country'=$3 AND r.config->>'language'=$4 ORDER BY r.created_at DESC,r.id DESC LIMIT 100) q"
        }
        _ => {
            return Err(AppError::Validation(
                "Report kind must be audit, rank, keywords, links or ai".into(),
            ));
        }
    };
    // Audit uses two parameters; the other fixed queries bind all four.
    let query = sqlx::query_scalar::<_, Value>(sql)
        .bind(p.id)
        .bind(&p.domain);
    let row = if kind == "audit" {
        query.fetch_optional(pool).await
    } else {
        query
            .bind(&p.country_code)
            .bind(&p.language_code)
            .fetch_optional(pool)
            .await
    }
    .map_err(internal)?;
    Ok(
        json!({"kind":kind,"project":p.name,"domain":p.domain,"country":p.country_code,"language":p.language_code,"generated_at":chrono::Utc::now(),"evidence":row,"note":"Stored evidence only; no provider calls. Rows are bounded samples. Missing positions/metrics remain unknown; imported link absence is not verified loss. AI API runs are not consumer search visibility."}),
    )
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct AccountDeletion {
    pub user_id: Uuid,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub delete_after: chrono::DateTime<chrono::Utc>,
    pub blocked_reason: Option<String>,
}
pub async fn deletion(pool: &PgPool, user: Uuid) -> AppResult<Option<AccountDeletion>> {
    sqlx::query_as("SELECT * FROM account_deletions WHERE user_id=$1")
        .bind(user)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}
pub async fn request_deletion(pool: &PgPool, user: Uuid) -> AppResult<AccountDeletion> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let _: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?
        .ok_or(AppError::Unauthorized)?;
    let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM members m WHERE m.user_id=$1 AND m.role='owner' AND EXISTS(SELECT 1 FROM members x WHERE x.organization_id=m.organization_id AND x.user_id<>$1) AND NOT EXISTS(SELECT 1 FROM members o WHERE o.organization_id=m.organization_id AND o.user_id<>$1 AND o.role='owner'))").bind(user).fetch_one(&mut *tx).await.map_err(internal)?;
    if blocked {
        return Err(AppError::Conflict(
            "Transfer ownership of shared workspaces to another member before requesting deletion"
                .into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO account_deletions(user_id) VALUES($1) ON CONFLICT(user_id) DO UPDATE SET user_id=excluded.user_id RETURNING *").bind(user).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn cancel_deletion(pool: &PgPool, user: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM account_deletions WHERE user_id=$1")
        .bind(user)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
pub async fn purge_due(pool: &PgPool) -> AppResult<u64> {
    let users:Vec<Uuid>=sqlx::query_scalar("SELECT user_id FROM account_deletions WHERE delete_after<=now() ORDER BY (blocked_reason IS NOT NULL),delete_after LIMIT 20").fetch_all(pool).await.map_err(internal)?;
    let mut count = 0;
    for user in users {
        let mut tx = pool.begin().await.map_err(internal)?;
        let due:Option<Uuid>=sqlx::query_scalar("SELECT d.user_id FROM account_deletions d JOIN users u ON u.id=d.user_id WHERE d.user_id=$1 AND d.delete_after<=now() FOR UPDATE OF u,d").bind(user).fetch_optional(&mut *tx).await.map_err(internal)?;
        if due.is_none() {
            continue;
        }
        let owned:Vec<Uuid>=sqlx::query_scalar("SELECT o.id FROM organizations o JOIN members m ON m.organization_id=o.id WHERE m.user_id=$1 AND m.role='owner' ORDER BY o.id FOR UPDATE OF o").bind(user).fetch_all(&mut *tx).await.map_err(internal)?;
        let _:Vec<Uuid>=sqlx::query_scalar("SELECT user_id FROM members WHERE organization_id=ANY($1::uuid[]) ORDER BY organization_id,user_id FOR UPDATE").bind(&owned).fetch_all(&mut *tx).await.map_err(internal)?;
        let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM members m WHERE m.user_id=$1 AND m.role='owner' AND EXISTS(SELECT 1 FROM members x WHERE x.organization_id=m.organization_id AND x.user_id<>$1) AND NOT EXISTS(SELECT 1 FROM members x WHERE x.organization_id=m.organization_id AND x.user_id<>$1 AND x.role='owner'))").bind(user).fetch_one(&mut *tx).await.map_err(internal)?;
        if blocked {
            sqlx::query("UPDATE account_deletions SET blocked_reason='Shared workspace ownership changed; transfer ownership before deletion' WHERE user_id=$1").bind(user).execute(&mut *tx).await.map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            continue;
        }
        for org in owned {
            sqlx::query("DELETE FROM organizations o WHERE o.id=$1 AND NOT EXISTS(SELECT 1 FROM members WHERE organization_id=o.id AND user_id<>$2)").bind(org).bind(user).execute(&mut *tx).await.map_err(internal)?;
        }
        // A deleted user's grants cannot remain connected to shared projects.
        for statement in [
            "DELETE FROM gsc_connections WHERE connected_by=$1",
            "DELETE FROM ga4_connections WHERE connected_by=$1",
            "DELETE FROM bing_webmaster_connections WHERE connected_by=$1",
        ] {
            sqlx::query(statement)
                .bind(user)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
        }
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(user)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        count += 1;
    }
    Ok(count)
}
