//! Tenant-scoped alert settings, immutable evaluations, and a transactional delivery outbox.
use crate::{internal, models::Project};
use common::{AppError, AppResult, new_id};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct NotificationSettings {
    pub project_id: Uuid,
    pub revision: i64,
    pub domain: String,
    pub country: String,
    pub language: String,
    pub rank_enabled: bool,
    pub page_enabled: bool,
    pub min_change: i32,
    pub include_depth_changes: bool,
    pub key_urls: Vec<String>,
    pub enabled_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct NotificationEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub source_id: Uuid,
    pub domain: String,
    pub country: String,
    pub language: String,
    pub data: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct NotificationEndpoint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub enabled: bool,
    pub revision: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct NotificationDelivery {
    pub id: Uuid,
    pub project_id: Uuid,
    pub event_id: Option<Uuid>,
    pub endpoint_id: Option<Uuid>,
    pub endpoint_revision: Option<i64>,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub domain: String,
    pub country: String,
    pub language: String,
    pub dedupe_key: String,
    pub payload: Value,
    pub job_id: Option<Uuid>,
    pub status: String,
    pub error: Option<String>,
    pub attempted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct DigestSubscription {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub revision: i64,
    pub enabled: bool,
    pub domain: String,
    pub country: String,
    pub language: String,
    pub weekday: i16,
    pub hour_utc: i16,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}
pub async fn settings(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
) -> AppResult<Option<NotificationSettings>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT s.* FROM notification_settings s JOIN projects p ON p.id=s.project_id WHERE p.id=$1 AND p.organization_id=$2").bind(pid).bind(org).fetch_optional(pool).await.map_err(internal)
}
pub struct SettingsWrite {
    pub revision: i64,
    pub expected_domain: String,
    pub rank_enabled: bool,
    pub page_enabled: bool,
    pub min_change: i32,
    pub include_depth_changes: bool,
    pub key_urls: Vec<String>,
}
pub async fn save_settings(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    v: SettingsWrite,
) -> AppResult<NotificationSettings> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p:Project=sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if p.domain.as_deref() != Some(v.expected_domain.as_str()) {
        return Err(AppError::Conflict(
            "Website changed; reload alert settings".into(),
        ));
    }
    let rev: i64 =
        sqlx::query_scalar("SELECT revision FROM notification_settings WHERE project_id=$1")
            .bind(pid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?
            .unwrap_or(0);
    if rev != v.revision {
        return Err(AppError::Conflict(
            "Alert settings changed; reload before saving".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO notification_settings(project_id,domain,country,language,rank_enabled,page_enabled,min_change,include_depth_changes,key_urls) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(project_id) DO UPDATE SET revision=notification_settings.revision+1,domain=excluded.domain,country=excluded.country,language=excluded.language,rank_enabled=excluded.rank_enabled,page_enabled=excluded.page_enabled,min_change=excluded.min_change,include_depth_changes=excluded.include_depth_changes,key_urls=excluded.key_urls,enabled_at=clock_timestamp() RETURNING *")
        .bind(pid).bind(v.expected_domain).bind(p.country_code).bind(p.language_code).bind(v.rank_enabled).bind(v.page_enabled).bind(v.min_change).bind(v.include_depth_changes).bind(v.key_urls).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn projects_due(pool: &PgPool) -> AppResult<Vec<(Uuid, Uuid)>> {
    sqlx::query_as("WITH due AS (SELECT s.project_id FROM notification_settings s JOIN projects p ON p.id=s.project_id WHERE p.archived_at IS NULL AND (s.rank_enabled OR s.page_enabled) AND s.domain=p.domain::text AND s.country=p.country_code AND s.language=p.language_code ORDER BY s.last_checked_at,s.project_id FOR UPDATE OF s SKIP LOCKED LIMIT 1000), claimed AS (UPDATE notification_settings s SET last_checked_at=clock_timestamp() FROM due WHERE s.project_id=due.project_id RETURNING s.project_id) SELECT p.organization_id,p.id FROM claimed JOIN projects p ON p.id=claimed.project_id").fetch_all(pool).await.map_err(internal)
}
pub async fn candidates(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<Vec<(String, Uuid)>> {
    sqlx::query_as("SELECT kind,id FROM (SELECT 'rank_movement'::text AS kind,r.id,r.finished_at FROM rank_runs r JOIN rank_trackers t ON t.id=r.tracker_id JOIN notification_settings s ON s.project_id=t.project_id JOIN projects p ON p.id=s.project_id WHERE p.id=$1 AND p.organization_id=$2 AND s.rank_enabled AND r.status='completed' AND r.finished_at>=s.enabled_at AND t.domain=s.domain AND t.country=s.country AND t.language=s.language UNION ALL SELECT 'key_page_change'::text,a.id,a.finished_at FROM audits a JOIN notification_settings s ON s.project_id=a.project_id JOIN projects p ON p.id=s.project_id WHERE p.id=$1 AND p.organization_id=$2 AND s.page_enabled AND a.status='completed' AND a.finished_at>=s.enabled_at AND lower(split_part(split_part(split_part(a.start_url,'://',2),'/',1),':',1))=s.domain) q WHERE NOT EXISTS(SELECT 1 FROM notification_evaluations e WHERE e.project_id=$1 AND e.kind=q.kind AND e.source_id=q.id) ORDER BY finished_at,id LIMIT 20")
        .bind(pid).bind(org).fetch_all(pool).await.map_err(internal)
}
pub struct Evaluation {
    pub tx: Transaction<'static, Postgres>,
    pub settings: NotificationSettings,
    pub project: Project,
    pub source_id: Uuid,
    pub kind: String,
    pub baseline: Option<Uuid>,
    pub rows: Vec<Value>,
}
pub async fn prepare(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    kind: &str,
    source: Uuid,
) -> AppResult<Option<Evaluation>> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p: Project =
        sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 FOR UPDATE")
            .bind(pid)
            .bind(org)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?
            .ok_or(AppError::NotFound("project"))?;
    let s: Option<NotificationSettings> =
        sqlx::query_as("SELECT * FROM notification_settings WHERE project_id=$1")
            .bind(pid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
    let Some(s) = s else { return Ok(None) };
    if p.archived_at.is_some()
        || p.domain.as_deref() != Some(&s.domain)
        || p.country_code != s.country
        || p.language_code != s.language
    {
        return Ok(None);
    }
    let done:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_evaluations WHERE project_id=$1 AND kind=$2 AND source_id=$3)").bind(pid).bind(kind).bind(source).fetch_one(&mut *tx).await.map_err(internal)?;
    if done {
        return Ok(None);
    }
    let (baseline, rows) = if kind == "rank_movement" && s.rank_enabled {
        let tracker:Option<Uuid>=sqlx::query_scalar("SELECT r.tracker_id FROM rank_runs r JOIN rank_trackers t ON t.id=r.tracker_id WHERE r.id=$1 AND t.project_id=$2 AND r.status='completed' AND r.finished_at >= $3 AND t.domain=$4 AND t.country=$5 AND t.language=$6").bind(source).bind(pid).bind(s.enabled_at).bind(&s.domain).bind(&s.country).bind(&s.language).fetch_optional(&mut *tx).await.map_err(internal)?;
        let Some(tracker) = tracker else {
            return Ok(None);
        };
        let baseline:Option<Uuid>=sqlx::query_scalar("SELECT b.id FROM rank_runs b JOIN rank_runs a ON a.id=$1 WHERE b.tracker_id=$2 AND b.status='completed' AND (b.created_at,b.id)<(a.created_at,a.id) AND b.finished_at<=a.finished_at ORDER BY b.created_at DESC,b.id DESC LIMIT 1").bind(source).bind(tracker).fetch_optional(&mut *tx).await.map_err(internal)?;
        let rows=sqlx::query_scalar("SELECT jsonb_build_object('keyword',k.keyword,'before',b.position,'after',a.position,'provider',a.provider,'previous_provider',b.provider,'exact',a.exact,'previous_exact',b.exact,'depth',a.depth,'previous_depth',b.depth,'previous_measured',b.run_id IS NOT NULL) FROM rank_results a JOIN rank_tracker_keywords k ON k.id=a.keyword_id LEFT JOIN rank_results b ON b.keyword_id=a.keyword_id AND b.run_id=$2 WHERE a.run_id=$1 ORDER BY k.keyword LIMIT 501").bind(source).bind(baseline).fetch_all(&mut *tx).await.map_err(internal)?;
        (baseline, rows)
    } else if kind == "key_page_change" && s.page_enabled {
        let eligible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM audits WHERE id=$1 AND project_id=$2 AND status='completed' AND finished_at >= $3 AND lower(split_part(split_part(split_part(start_url,'://',2),'/',1),':',1))=$4)").bind(source).bind(pid).bind(s.enabled_at).bind(&s.domain).fetch_one(&mut *tx).await.map_err(internal)?;
        if !eligible {
            return Ok(None);
        }
        let baseline:Option<Uuid>=sqlx::query_scalar("SELECT b.id FROM audits b JOIN audits a ON a.id=$1 WHERE b.project_id=$2 AND b.status='completed' AND b.start_url=a.start_url AND COALESCE((b.config->>'render_js')::boolean,false)=COALESCE((a.config->>'render_js')::boolean,false) AND (b.created_at,b.id)<(a.created_at,a.id) AND b.finished_at<=a.finished_at ORDER BY b.created_at DESC,b.id DESC LIMIT 1").bind(source).bind(pid).fetch_optional(&mut *tx).await.map_err(internal)?;
        let rows=sqlx::query_scalar("SELECT jsonb_build_object('url',u.url,'comparable',a.status_code=200 AND b.status_code=200 AND a.fetch_class='html' AND b.fetch_class='html' AND jsonb_array_length(a.redirect_chain)=0 AND jsonb_array_length(b.redirect_chain)=0,'before',jsonb_build_object('title',b.title,'meta_description',b.meta_description,'canonical',b.canonical_url,'header_canonical',b.header_canonical_url,'meta_robots',b.meta_robots,'x_robots_tag',b.x_robots_tag,'indexable',b.is_indexable),'after',jsonb_build_object('title',a.title,'meta_description',a.meta_description,'canonical',a.canonical_url,'header_canonical',a.header_canonical_url,'meta_robots',a.meta_robots,'x_robots_tag',a.x_robots_tag,'indexable',a.is_indexable)) FROM unnest($3::text[]) u(url) LEFT JOIN audit_pages a ON a.audit_id=$1 AND a.url=u.url LEFT JOIN audit_pages b ON b.audit_id=$2 AND b.url=u.url ORDER BY u.url LIMIT 100").bind(source).bind(baseline).bind(&s.key_urls).fetch_all(&mut *tx).await.map_err(internal)?;
        (baseline, rows)
    } else {
        return Ok(None);
    };
    Ok(Some(Evaluation {
        tx,
        settings: s,
        project: p,
        source_id: source,
        kind: kind.into(),
        baseline,
        rows,
    }))
}
pub async fn commit_evaluation(
    mut e: Evaluation,
    summary: Value,
    event: Option<Value>,
) -> AppResult<()> {
    sqlx::query("INSERT INTO notification_evaluations(project_id,kind,source_id,settings_revision,summary) VALUES($1,$2,$3,$4,$5)").bind(e.project.id).bind(&e.kind).bind(e.source_id).bind(e.settings.revision).bind(summary).execute(&mut *e.tx).await.map_err(internal)?;
    if let Some(data) = event {
        let id = new_id();
        sqlx::query("INSERT INTO notification_events(id,project_id,kind,source_id,domain,country,language,data) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(id).bind(e.project.id).bind(&e.kind).bind(e.source_id).bind(&e.settings.domain).bind(&e.settings.country).bind(&e.settings.language).bind(&data).execute(&mut *e.tx).await.map_err(internal)?;
        let endpoints:Vec<NotificationEndpoint>=sqlx::query_as("SELECT * FROM notification_endpoints WHERE project_id=$1 AND enabled ORDER BY id LIMIT 10").bind(e.project.id).fetch_all(&mut *e.tx).await.map_err(internal)?;
        for endpoint in endpoints {
            let payload = json!({"id":id,"type":e.kind,"project_id":e.project.id,"project_name":e.project.name,"domain":e.settings.domain,"country":e.settings.country,"language":e.settings.language,"occurred_at":chrono::Utc::now(),"data":data});
            queue(
                &mut e.tx,
                &e.project,
                &endpoint.kind,
                Some(id),
                Some(&endpoint),
                None,
                &format!("{id}:{}", endpoint.id),
                payload,
            )
            .await?;
        }
    }
    e.tx.commit().await.map_err(internal)
}
#[allow(clippy::too_many_arguments)]
async fn queue(
    tx: &mut Transaction<'_, Postgres>,
    p: &Project,
    kind: &str,
    event: Option<Uuid>,
    endpoint: Option<&NotificationEndpoint>,
    user: Option<Uuid>,
    dedupe: &str,
    payload: Value,
) -> AppResult<Uuid> {
    let id = new_id();
    let job = new_id();
    sqlx::query("INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES($1,'notification_delivery',$2,$3,$4,1)").bind(job).bind(p.organization_id).bind(p.id).bind(json!({"delivery_id":id})).execute(&mut **tx).await.map_err(internal)?;
    sqlx::query("INSERT INTO notification_deliveries(id,project_id,event_id,endpoint_id,endpoint_revision,user_id,kind,domain,country,language,dedupe_key,payload,job_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)").bind(id).bind(p.id).bind(event).bind(endpoint.map(|v|v.id)).bind(endpoint.map(|v|v.revision)).bind(user).bind(kind).bind(&p.domain).bind(&p.country_code).bind(&p.language_code).bind(dedupe).bind(payload).bind(job).execute(&mut **tx).await.map_err(internal)?;
    Ok(id)
}
pub async fn events(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<NotificationEvent>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT e.* FROM notification_events e JOIN projects p ON p.id=e.project_id WHERE p.id=$1 AND p.organization_id=$2 ORDER BY e.created_at DESC,e.id DESC LIMIT $3 OFFSET $4").bind(pid).bind(org).bind(limit).bind(offset).fetch_all(pool).await.map_err(internal)
}
pub async fn acknowledge(pool: &PgPool, org: Uuid, pid: Uuid, id: Uuid) -> AppResult<()> {
    let r=sqlx::query("UPDATE notification_events e SET acknowledged_at=COALESCE(acknowledged_at,now()) FROM projects p WHERE e.project_id=p.id AND p.organization_id=$1 AND p.id=$2 AND e.id=$3").bind(org).bind(pid).bind(id).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("alert"));
    }
    Ok(())
}
pub async fn endpoints(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
) -> AppResult<Vec<NotificationEndpoint>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT e.* FROM notification_endpoints e JOIN projects p ON p.id=e.project_id WHERE p.organization_id=$1 AND p.id=$2 ORDER BY e.created_at,e.id LIMIT 10").bind(org).bind(pid).fetch_all(pool).await.map_err(internal)
}
pub async fn create_endpoint(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    name: &str,
    kind: &str,
    host: &str,
    secret: Vec<u8>,
) -> AppResult<NotificationEndpoint> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let _:Uuid=sqlx::query_scalar("SELECT id FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notification_endpoints WHERE project_id=$1")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    if count >= 10 {
        return Err(AppError::Conflict(
            "At most ten delivery endpoints per project".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO notification_endpoints(id,project_id,name,kind,host,secret_url) VALUES($1,$2,$3,$4,$5,$6) RETURNING *").bind(new_id()).bind(pid).bind(name).bind(kind).bind(host).bind(secret).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn endpoint_enabled(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    id: Uuid,
    revision: i64,
    enabled: bool,
) -> AppResult<()> {
    let r=sqlx::query("UPDATE notification_endpoints e SET enabled=$5,revision=e.revision+1 FROM projects p WHERE e.project_id=p.id AND p.id=$2 AND p.organization_id=$1 AND e.id=$3 AND e.revision=$4").bind(org).bind(pid).bind(id).bind(revision).bind(enabled).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Endpoint missing or changed; reload".into(),
        ));
    }
    Ok(())
}
pub async fn delete_endpoint(pool: &PgPool, org: Uuid, pid: Uuid, id: Uuid) -> AppResult<()> {
    let r=sqlx::query("DELETE FROM notification_endpoints e USING projects p WHERE e.project_id=p.id AND p.id=$2 AND p.organization_id=$1 AND e.id=$3").bind(org).bind(pid).bind(id).execute(pool).await.map_err(internal)?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("endpoint"));
    }
    Ok(())
}
pub async fn deliveries(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
    admin: bool,
) -> AppResult<Vec<NotificationDelivery>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query("UPDATE notification_deliveries d SET status='failed',error='Worker stopped; outcome may be unknown. No automatic replay.',finished_at=now() FROM jobs j WHERE d.project_id=$1 AND d.job_id=j.id AND j.status='failed' AND d.status IN ('queued','sending')").bind(pid).execute(pool).await.map_err(internal)?;
    sqlx::query_as("SELECT d.* FROM notification_deliveries d JOIN projects p ON p.id=d.project_id WHERE p.id=$1 AND p.organization_id=$2 AND ($4 OR d.user_id=$3) ORDER BY d.created_at DESC,d.id DESC LIMIT 100").bind(pid).bind(org).bind(user).bind(admin).fetch_all(pool).await.map_err(internal)
}
pub async fn claim_delivery(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    id: Uuid,
) -> AppResult<Option<NotificationDelivery>> {
    sqlx::query_as("UPDATE notification_deliveries d SET status='sending',attempted_at=now() FROM projects p WHERE d.project_id=p.id AND p.id=$1 AND p.organization_id=$2 AND d.id=$3 AND d.status='queued' AND d.attempted_at IS NULL RETURNING d.*").bind(pid).bind(org).bind(id).fetch_optional(pool).await.map_err(internal)
}
pub async fn endpoint_secret(
    pool: &PgPool,
    pid: Uuid,
    id: Uuid,
    revision: i64,
) -> AppResult<Option<Vec<u8>>> {
    sqlx::query_scalar("SELECT secret_url FROM notification_endpoints WHERE project_id=$1 AND id=$2 AND revision=$3 AND enabled").bind(pid).bind(id).bind(revision).fetch_optional(pool).await.map_err(internal)
}
pub async fn finish_delivery(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE notification_deliveries SET status=$2,error=$3,finished_at=now() WHERE id=$1 AND status='sending'").bind(id).bind(status).bind(error).execute(pool).await.map_err(internal)?;
    Ok(())
}
pub async fn subscription(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    user: Uuid,
) -> AppResult<Option<DigestSubscription>> {
    crate::projects::get(pool, org, pid).await?;
    sqlx::query_as("SELECT * FROM digest_subscriptions WHERE project_id=$1 AND user_id=$2")
        .bind(pid)
        .bind(user)
        .fetch_optional(pool)
        .await
        .map_err(internal)
}
#[allow(clippy::too_many_arguments)]
pub async fn save_subscription(
    pool: &PgPool,
    p: &Project,
    user: Uuid,
    revision: i64,
    enabled: bool,
    weekday: i16,
    hour: i16,
    next: Option<chrono::DateTime<chrono::Utc>>,
) -> AppResult<DigestSubscription> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let current:Project=sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(p.id).bind(p.organization_id).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if current.domain != p.domain
        || current.country_code != p.country_code
        || current.language_code != p.language_code
    {
        return Err(AppError::Conflict("Project scope changed; reload".into()));
    }
    let rev: i64 = sqlx::query_scalar(
        "SELECT revision FROM digest_subscriptions WHERE project_id=$1 AND user_id=$2",
    )
    .bind(p.id)
    .bind(user)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .unwrap_or(0);
    if rev != revision {
        return Err(AppError::Conflict("Digest settings changed; reload".into()));
    }
    let row=sqlx::query_as("INSERT INTO digest_subscriptions(project_id,user_id,enabled,domain,country,language,weekday,hour_utc,next_run_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(project_id,user_id) DO UPDATE SET revision=digest_subscriptions.revision+1,enabled=excluded.enabled,domain=excluded.domain,country=excluded.country,language=excluded.language,weekday=excluded.weekday,hour_utc=excluded.hour_utc,next_run_at=excluded.next_run_at,last_error=NULL RETURNING *").bind(p.id).bind(user).bind(enabled).bind(&p.domain).bind(&p.country_code).bind(&p.language_code).bind(weekday).bind(hour).bind(next).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
pub async fn digest_due(pool: &PgPool) -> AppResult<Vec<(Uuid, Uuid, Uuid)>> {
    sqlx::query_as("SELECT p.organization_id,s.project_id,s.user_id FROM digest_subscriptions s JOIN projects p ON p.id=s.project_id WHERE s.enabled AND s.next_run_at<=now() ORDER BY s.next_run_at LIMIT 100").fetch_all(pool).await.map_err(internal)
}
pub async fn queue_digest(
    pool: &PgPool,
    p: &Project,
    user: Uuid,
    expected: chrono::DateTime<chrono::Utc>,
    next: chrono::DateTime<chrono::Utc>,
    payload: Value,
) -> AppResult<bool> {
    let original = p.clone();
    let mut tx = pool.begin().await.map_err(internal)?;
    let p:Project=sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(p.id).bind(p.organization_id).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if p.domain != original.domain
        || p.country_code != original.country_code
        || p.language_code != original.language_code
    {
        return Err(AppError::Conflict(
            "Project changed while preparing digest".into(),
        ));
    }
    let eligible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM digest_subscriptions s JOIN members m ON m.user_id=s.user_id AND m.organization_id=$5 WHERE s.project_id=$1 AND s.user_id=$2 AND s.enabled AND s.next_run_at=$3 AND s.domain=$4 AND s.country=$6 AND s.language=$7 AND s.revision=$8)").bind(p.id).bind(user).bind(expected).bind(&p.domain).bind(p.organization_id).bind(&p.country_code).bind(&p.language_code).bind(payload["subscription_revision"].as_i64()).fetch_one(&mut *tx).await.map_err(internal)?;
    if !eligible {
        return Ok(false);
    }
    queue(
        &mut tx,
        &p,
        "email",
        None,
        None,
        Some(user),
        &format!("digest:{}:{user}:{expected}", p.id),
        payload,
    )
    .await?;
    sqlx::query("UPDATE digest_subscriptions SET next_run_at=$3,last_error=NULL WHERE project_id=$1 AND user_id=$2").bind(p.id).bind(user).bind(next).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(true)
}

pub async fn queue_endpoint_test(pool: &PgPool, org: Uuid, pid: Uuid, id: Uuid) -> AppResult<Uuid> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let p:Project=sqlx::query_as("SELECT * FROM projects WHERE id=$1 AND organization_id=$2 AND archived_at IS NULL FOR UPDATE").bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("active project"))?;
    if p.domain.is_none() {
        return Err(AppError::Validation("Set project website first".into()));
    }
    let endpoint: NotificationEndpoint = sqlx::query_as(
        "SELECT * FROM notification_endpoints WHERE id=$1 AND project_id=$2 AND enabled",
    )
    .bind(id)
    .bind(pid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .ok_or(AppError::NotFound("enabled endpoint"))?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM notification_deliveries WHERE endpoint_id=$1 AND created_at>now()-interval '1 minute'").bind(id).fetch_one(&mut *tx).await.map_err(internal)?;
    if count >= 3 {
        return Err(AppError::Conflict(
            "Wait a minute before testing this endpoint again".into(),
        ));
    }
    let key = new_id();
    let payload = json!({"id":key,"type":"test","project_id":pid,"data":{"message":"SeoSeo endpoint delivery test"}});
    let id = queue(
        &mut tx,
        &p,
        &endpoint.kind,
        None,
        Some(&endpoint),
        None,
        &format!("test:{key}"),
        payload,
    )
    .await?;
    tx.commit().await.map_err(internal)?;
    Ok(id)
}
