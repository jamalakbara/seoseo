//! Comparable evidence alerts, weekly digests and single-attempt outbound delivery.
use crate::{Actor, Role};
use common::{AppError, AppResult, Config};
use db::notifications as repo;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
pub async fn acknowledge(pool: &PgPool, a: &Actor, pid: Uuid, id: Uuid) -> AppResult<()> {
    a.require(Role::Member)?;
    repo::acknowledge(pool, a.org_id, pid, id).await
}
pub async fn endpoints(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Vec<repo::NotificationEndpoint>> {
    a.require(Role::Admin)?;
    repo::endpoints(pool, a.org_id, pid).await
}
pub async fn toggle_endpoint(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    id: Uuid,
    v: EndpointToggle,
) -> AppResult<()> {
    a.require(Role::Admin)?;
    repo::endpoint_enabled(pool, a.org_id, pid, id, v.revision, v.enabled).await
}
pub async fn delete_endpoint(pool: &PgPool, a: &Actor, pid: Uuid, id: Uuid) -> AppResult<()> {
    a.require(Role::Admin)?;
    repo::delete_endpoint(pool, a.org_id, pid, id).await
}
pub async fn test_endpoint(pool: &PgPool, a: &Actor, pid: Uuid, id: Uuid) -> AppResult<Uuid> {
    a.require(Role::Admin)?;
    repo::queue_endpoint_test(pool, a.org_id, pid, id).await
}
pub async fn deliveries(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Vec<repo::NotificationDelivery>> {
    repo::deliveries(pool, a.org_id, pid, a.user_id, a.role >= Role::Admin).await
}
pub async fn settings(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Option<repo::NotificationSettings>> {
    repo::settings(pool, a.org_id, pid).await
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AlertSettingsInput {
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
    a: &Actor,
    pid: Uuid,
    v: AlertSettingsInput,
) -> AppResult<repo::NotificationSettings> {
    a.require(Role::Admin)?;
    if !(1..=100).contains(&v.min_change) || v.key_urls.len() > 100 || v.revision < 0 {
        return Err(AppError::Validation(
            "Use a 1-100 movement threshold, at most 100 monitored URLs and a nonnegative revision"
                .into(),
        ));
    }
    let mut urls = vec![];
    for raw in v.key_urls {
        let u = url::Url::parse(raw.trim()).map_err(|_| {
            AppError::Validation(
                "Monitored URLs must be absolute HTTP(S) URLs on the project website".into(),
            )
        })?;
        if !matches!(u.scheme(), "http" | "https")
            || u.host_str() != Some(v.expected_domain.as_str())
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
            || u.as_str().len() > 2048
        {
            return Err(AppError::Validation("Monitored URLs must match the project website exactly, without credentials/fragments".into()));
        }
        if !urls.contains(&u.to_string()) {
            urls.push(u.to_string());
        }
    }
    if v.page_enabled && urls.is_empty() {
        return Err(AppError::Validation(
            "Choose at least one key page for change monitoring".into(),
        ));
    }
    repo::save_settings(
        pool,
        a.org_id,
        pid,
        repo::SettingsWrite {
            revision: v.revision,
            expected_domain: v.expected_domain,
            rank_enabled: v.rank_enabled,
            page_enabled: v.page_enabled,
            min_change: v.min_change,
            include_depth_changes: v.include_depth_changes,
            key_urls: urls,
        },
    )
    .await
}
#[derive(Default, Deserialize, IntoParams)]
pub struct AlertQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
pub async fn events(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    q: AlertQuery,
) -> AppResult<Vec<repo::NotificationEvent>> {
    let limit = q.limit.unwrap_or(25);
    let offset = q.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || offset < 0 {
        return Err(AppError::Validation(
            "limit must be 1-100 and offset nonnegative".into(),
        ));
    }
    repo::events(pool, a.org_id, pid, limit, offset).await
}
pub async fn evaluate(pool: &PgPool, a: &Actor, pid: Uuid) -> AppResult<usize> {
    a.require(Role::Admin)?;
    crate::projects::get(pool, a, pid).await?;
    evaluate_project(pool, a.org_id, pid).await
}
async fn evaluate_project(pool: &PgPool, org: Uuid, pid: Uuid) -> AppResult<usize> {
    let mut count = 0;
    for (kind, id) in repo::candidates(pool, org, pid).await? {
        if let Some(e) = repo::prepare(pool, org, pid, &kind, id).await? {
            let (items, comparable) = if kind == "rank_movement" {
                rank_changes(
                    &e.rows,
                    e.settings.min_change,
                    e.settings.include_depth_changes,
                )
            } else {
                page_changes(&e.rows)
            };
            let total = items.len();
            let summary = json!({"source_id":id,"baseline_id":e.baseline,"settings_revision":e.settings.revision,"evaluated":e.rows.len(),"comparable":comparable,"unavailable_or_incomparable":e.rows.len()-comparable,"matched":total,"truncated":total>100,"items":items.into_iter().take(100).collect::<Vec<_>>(),"note":"Compared stored completed observations only. Missing/failed pages and incompatible provider, exactness or depth are not measured changes. No live provider calls."});
            let event = if total > 0 {
                Some(summary.clone())
            } else {
                None
            };
            repo::commit_evaluation(e, summary, event).await?;
            count += 1;
        }
    }
    Ok(count)
}
fn rank_changes(rows: &[Value], threshold: i32, depth_changes: bool) -> (Vec<Value>, usize) {
    let mut items = vec![];
    let mut comparable = 0;
    for row in rows {
        let depth = row["depth"].as_i64().unwrap_or(0);
        if row["previous_measured"] != true
            || depth <= 0
            || row["previous_depth"].as_i64() != Some(depth)
            || row["provider"] != row["previous_provider"]
            || row["exact"] != row["previous_exact"]
        {
            continue;
        }
        let before = row["before"].as_i64();
        let after = row["after"].as_i64();
        if [before, after]
            .iter()
            .flatten()
            .any(|p| *p < 1 || *p > depth)
        {
            continue;
        }
        comparable += 1;
        let (kind, delta) = match (before, after) {
            (Some(b), Some(a)) if (b - a).abs() >= i64::from(threshold) => {
                (if b > a { "gain" } else { "loss" }, Some(b - a))
            }
            (None, Some(_)) if depth_changes => ("entered_depth", None),
            (Some(_), None) if depth_changes => ("left_depth", None),
            _ => continue,
        };
        items.push(json!({"keyword":row["keyword"],"kind":kind,"before":before,"after":after,"change":delta,"depth":depth,"provider":row["provider"],"exact":row["exact"]}));
    }
    (items, comparable)
}
fn page_changes(rows: &[Value]) -> (Vec<Value>, usize) {
    let mut items = vec![];
    let mut comparable = 0;
    for row in rows {
        if row["comparable"] != true {
            continue;
        }
        comparable += 1;
        let mut changes = vec![];
        for field in [
            "title",
            "meta_description",
            "canonical",
            "header_canonical",
            "meta_robots",
            "x_robots_tag",
            "indexable",
        ] {
            if row["before"][field] != row["after"][field] {
                let display = |v: &Value| match v.as_str() {
                    Some(s) => {
                        json!({"value":s.chars().take(300).collect::<String>(),"truncated":s.chars().count()>300})
                    }
                    None => json!({"value":v,"truncated":false}),
                };
                changes.push(json!({"field":field,"before":display(&row["before"][field]),"after":display(&row["after"][field])}));
            }
        }
        if !changes.is_empty() {
            items.push(json!({"url":row["url"],"changes":changes}));
        }
    }
    (items, comparable)
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EndpointInput {
    pub name: String,
    pub kind: String,
    pub url: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EndpointToggle {
    pub revision: i64,
    pub enabled: bool,
}
fn endpoint_url(raw: &str, kind: &str, allow_private: bool) -> AppResult<url::Url> {
    if raw.len() > 2048 {
        return Err(AppError::Validation("Webhook URL is too long".into()));
    }
    let url =
        url::Url::parse(raw).map_err(|_| AppError::Validation("Invalid webhook URL".into()))?;
    crawler::UrlPolicy { allow_private }
        .check_url(&url)
        .map_err(|_| {
            AppError::Validation(
                "Webhook destination must be a public HTTPS address without credentials".into(),
            )
        })?;
    if (url.scheme() != "https" && !allow_private) || url.fragment().is_some() {
        return Err(AppError::Validation(
            "Webhook destination requires HTTPS and no fragment".into(),
        ));
    }
    match kind {
        "webhook" => {}
        "slack"
            if url.scheme() == "https"
                && url.host_str() == Some("hooks.slack.com")
                && url.path().starts_with("/services/")
                && url.port().is_none() => {}
        "discord"
            if url.scheme() == "https"
                && url.host_str() == Some("discord.com")
                && url.path().starts_with("/api/webhooks/")
                && url.port().is_none() => {}
        _ => {
            return Err(AppError::Validation(
                "Use webhook, or an official Slack/Discord incoming-webhook URL".into(),
            ));
        }
    }
    Ok(url)
}
fn secret_box(cfg: &Config) -> AppResult<common::crypto::SecretBox> {
    common::crypto::SecretBox::from_base64(cfg.app_encryption_key.as_deref().unwrap_or("")).map_err(
        |_| {
            AppError::Conflict(
                "Configure APP_ENCRYPTION_KEY before saving delivery endpoints".into(),
            )
        },
    )
}
pub async fn create_endpoint(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    v: EndpointInput,
) -> AppResult<repo::NotificationEndpoint> {
    a.require(Role::Admin)?;
    crate::projects::get(pool, a, pid).await?;
    let name = crate::validate::non_empty("endpoint name", &v.name, 120)?;
    let url = endpoint_url(&v.url, &v.kind, cfg.notify_allow_private)?;
    let secret = secret_box(cfg)?
        .encrypt(url.as_str())
        .map_err(AppError::internal)?;
    repo::create_endpoint(
        pool,
        a.org_id,
        pid,
        &name,
        &v.kind,
        url.host_str().unwrap_or(""),
        secret,
    )
    .await
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DigestInput {
    pub revision: i64,
    pub enabled: bool,
    pub weekday: i16,
    pub hour_utc: i16,
}
#[derive(Serialize, ToSchema)]
pub struct DigestView {
    pub smtp_ready: bool,
    pub subscription: Option<repo::DigestSubscription>,
    pub note: String,
}
pub fn smtp_ready(cfg: &Config) -> bool {
    cfg.smtp_host.as_ref().is_some_and(|s| !s.is_empty())
        && cfg
            .smtp_from
            .as_ref()
            .is_some_and(|s| s.parse::<lettre::message::Mailbox>().is_ok())
        && matches!(cfg.smtp_security.as_str(), "starttls" | "tls" | "none")
}
pub async fn digest(pool: &PgPool, a: &Actor, pid: Uuid, cfg: &Config) -> AppResult<DigestView> {
    Ok(DigestView{smtp_ready:smtp_ready(cfg),subscription:repo::subscription(pool,a.org_id,pid,a.user_id).await?,note:"Weekly digest goes only to your account email. Uses stored evidence from the last seven days, no fresh provider calls. Schedule is UTC. Delivery failures are visible and never automatically replayed.".into()})
}
pub async fn save_digest(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    v: DigestInput,
) -> AppResult<DigestView> {
    a.require(Role::Member)?;
    if v.revision < 0 || !(1..=7).contains(&v.weekday) || !(0..=23).contains(&v.hour_utc) {
        return Err(AppError::Validation(
            "Use a nonnegative revision, weekday 1-7 and UTC hour 0-23".into(),
        ));
    }
    if v.enabled && !smtp_ready(cfg) {
        return Err(AppError::Conflict(
            "Operator must configure SMTP before enabling a weekly digest".into(),
        ));
    }
    let p = crate::projects::get(pool, a, pid).await?;
    if p.domain.is_none() {
        return Err(AppError::Validation("Set the project website first".into()));
    }
    let next = if v.enabled {
        jobs::rank_check::next_run(
            "weekly",
            v.hour_utc as u32,
            v.weekday as u32,
            chrono::Utc::now(),
        )
    } else {
        None
    };
    repo::save_subscription(
        pool, &p, a.user_id, v.revision, v.enabled, v.weekday, v.hour_utc, next,
    )
    .await?;
    digest(pool, a, pid, cfg).await
}
pub async fn tick(pool: &PgPool, cfg: &Config) -> AppResult<()> {
    for (org, pid) in repo::projects_due(pool).await? {
        if let Err(e) = evaluate_project(pool, org, pid).await {
            tracing::warn!(project=%pid,error=%e,"alert evaluation failed");
        }
    }
    if smtp_ready(cfg) {
        for (org, pid, user) in repo::digest_due(pool).await? {
            let result=async {
            let p=db::projects::get(pool,org,pid).await?;
            let Some(s)=repo::subscription(pool,org,pid,user).await? else{return Ok::<_,AppError>(())};
            let Some(expected)=s.next_run_at else{return Ok(())};
            let next=jobs::rank_check::next_run("weekly",s.hour_utc as u32,s.weekday as u32,chrono::Utc::now()).ok_or_else(||AppError::Validation("Invalid digest schedule".into()))?;
            let evidence=db::operations::report_evidence(pool,&p,"audit").await?;
            let events=repo::events(pool,org,pid,100,0).await?.into_iter().filter(|e|e.created_at>=chrono::Utc::now()-chrono::Duration::days(7)&&p.domain.as_deref()==Some(&e.domain)&&p.country_code==e.country&&p.language_code==e.language).map(|e|json!({"kind":e.kind,"created_at":e.created_at,"changes":e.data["matched"],"source_id":e.source_id})).collect::<Vec<_>>();
            let payload=json!({"type":"weekly_digest","subscription_revision":s.revision,"project":p.name,"domain":p.domain,"country":p.country_code,"language":p.language_code,"window_start":chrono::Utc::now()-chrono::Duration::days(7),"window_end":chrono::Utc::now(),"audit":evidence,"alerts":events,"note":"At most 100 recent alerts; stored evidence only. Latest audit may predate the window. No inferred traffic or missing metrics."});
            repo::queue_digest(pool,&p,user,expected,next,payload).await?;Ok(())
        }.await;
            if let Err(e) = result {
                tracing::warn!(project=%pid,error=%e,"digest scheduling failed");
            }
        }
    }
    Ok(())
}
pub fn register(registry: jobs::Registry, cfg: Config) -> jobs::Registry {
    registry.register("notification_delivery",move|ctx:jobs::JobCtx|{let cfg=cfg.clone();async move{
        let org=ctx.job.organization_id.ok_or_else(||anyhow::anyhow!("missing workspace"))?;let pid=ctx.job.project_id.ok_or_else(||anyhow::anyhow!("missing project"))?;
        let id:Uuid=serde_json::from_value(ctx.job.payload["delivery_id"].clone())?;
        let Some(d)=repo::claim_delivery(&ctx.pool,org,pid,id).await? else{return Ok(())};
        let result=deliver(&ctx.pool,org,&d,&cfg).await;
        match result{Ok(true)=>repo::finish_delivery(&ctx.pool,id,"delivered",None).await?,Ok(false)=>repo::finish_delivery(&ctx.pool,id,"cancelled",Some("Project scope, membership, subscription or endpoint changed before delivery")).await?,Err(e)=>repo::finish_delivery(&ctx.pool,id,"failed",Some(&e.to_string())).await?,}Ok(())
    }})
}
async fn deliver(
    pool: &PgPool,
    org: Uuid,
    d: &repo::NotificationDelivery,
    cfg: &Config,
) -> AppResult<bool> {
    let p = db::projects::get(pool, org, d.project_id).await?;
    if p.archived_at.is_some()
        || p.domain.as_deref() != Some(&d.domain)
        || p.country_code != d.country
        || p.language_code != d.language
    {
        return Ok(false);
    }
    if d.kind == "email" {
        let user = d.user_id.ok_or(AppError::NotFound("digest user"))?;
        if db::orgs::role(pool, org, user).await?.is_none() {
            return Ok(false);
        }
        let Some(s) = repo::subscription(pool, org, p.id, user).await? else {
            return Ok(false);
        };
        if !s.enabled
            || s.domain != d.domain
            || s.country != d.country
            || s.language != d.language
            || Some(s.revision) != d.payload["subscription_revision"].as_i64()
        {
            return Ok(false);
        }
        let user = db::users::find(pool, user)
            .await?
            .ok_or(AppError::NotFound("digest user"))?;
        send_mail(
            cfg,
            &user.email,
            d.id,
            &format!("SeoSeo weekly digest — {}", p.name),
            &digest_text(&d.payload),
        )
        .await?;
    } else {
        let Some(secret) = repo::endpoint_secret(
            pool,
            p.id,
            d.endpoint_id.ok_or(AppError::NotFound("endpoint"))?,
            d.endpoint_revision.unwrap_or(0),
        )
        .await?
        else {
            return Ok(false);
        };
        let raw = secret_box(cfg)?.decrypt(&secret).map_err(|_| {
            AppError::Conflict(
                "Delivery secret cannot be decrypted; check the operator encryption key".into(),
            )
        })?;
        let url = endpoint_url(&raw, &d.kind, cfg.notify_allow_private)?;
        let text = format!(
            "SeoSeo {}: {} ({}, {})\n{}",
            d.payload["type"].as_str().unwrap_or("alert"),
            p.name,
            p.country_code,
            p.language_code,
            serde_json::to_string(&d.payload["data"]).map_err(AppError::internal)?
        );
        let payload = webhook_payload(&d.kind, &text, &d.payload);
        let client = crawler::fetch::guarded_client(
            crawler::UrlPolicy {
                allow_private: cfg.notify_allow_private,
            },
            Duration::from_secs(20),
        )
        .map_err(AppError::internal)?;
        let response=client.post(url).header("X-SeoSeo-Delivery-Id",d.id.to_string()).json(&payload).send().await.map_err(|_|AppError::Unavailable("Webhook request failed or timed out. Outcome may be unknown; no automatic retry.".into()))?;
        if !response.status().is_success() {
            return Err(AppError::Unavailable(format!(
                "Webhook returned HTTP {}; stopped without redirects or retries",
                response.status().as_u16()
            )));
        }
    }
    Ok(true)
}
fn webhook_payload(kind: &str, text: &str, data: &Value) -> Value {
    match kind {
        "slack" => {
            json!({"text":text.chars().take(3000).collect::<String>().replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;"),"parse":"none","link_names":false,"unfurl_links":false,"unfurl_media":false})
        }
        "discord" => {
            json!({"content":text.chars().take(1900).collect::<String>(),"allowed_mentions":{"parse":[]}})
        }
        _ => data.clone(),
    }
}
fn digest_text(v: &Value) -> String {
    let text = |key: &str| v[key].as_str().unwrap_or("Unknown");
    let audit = &v["audit"]["evidence"];
    let mut body = format!(
        "SeoSeo weekly digest\n\n{} — {}\nMarket: {} / {}\nWindow: {} to {}\n\nLatest completed audit\nHealth: {}\nPages crawled: {}\nAudit observed: {}\n\nRecent alerts (up to 100)\n",
        text("project"),
        text("domain"),
        text("country"),
        text("language"),
        text("window_start"),
        text("window_end"),
        audit["health_score"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".into()),
        audit["pages_crawled"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".into()),
        audit["finished_at"]
            .as_str()
            .unwrap_or("No completed audit")
    );
    let rows = v["alerts"].as_array();
    if rows.is_none_or(|r| r.is_empty()) {
        body.push_str("No stored alerts in this sample/window. This does not establish that rankings or pages were checked.\n");
    }
    if let Some(rows) = rows {
        for r in rows {
            body.push_str(&format!(
                "- {}: {} changes, observed {}\n",
                r["kind"].as_str().unwrap_or("Alert"),
                r["changes"].as_u64().unwrap_or(0),
                r["created_at"].as_str().unwrap_or("Unknown")
            ));
        }
    }
    body.push_str("\nStored evidence only; no new provider collection. Missing metrics stay unknown. The latest audit may predate the window. Open your project's Alerts page for source evidence.\n");
    body
}
async fn send_mail(cfg: &Config, to: &str, id: Uuid, subject: &str, body: &str) -> AppResult<()> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    let host = cfg
        .smtp_host
        .as_deref()
        .ok_or_else(|| AppError::Conflict("SMTP is not configured".into()))?;
    let parse = || AppError::Validation("Invalid SMTP sender or recipient mailbox".into());
    let email = Message::builder()
        .from(
            cfg.smtp_from
                .as_deref()
                .unwrap_or("")
                .parse()
                .map_err(|_| parse())?,
        )
        .to(to.parse().map_err(|_| parse())?)
        .subject(subject)
        .message_id(Some(format!("<{id}@seoseo.local>")))
        .body(body.to_string())
        .map_err(|_| parse())?;
    let builder = match cfg.smtp_security.as_str() {
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|_| AppError::Validation("Invalid SMTP TLS relay".into()))?,
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .map_err(|_| AppError::Validation("Invalid SMTP TLS relay".into()))?,
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
        _ => {
            return Err(AppError::Validation(
                "SMTP_SECURITY must be starttls, tls or none (trusted relay only)".into(),
            ));
        }
    };
    let mut builder = builder
        .port(cfg.smtp_port)
        .timeout(Some(Duration::from_secs(20)));
    if let Some(user) = &cfg.smtp_username {
        builder = builder.credentials(Credentials::new(
            user.clone(),
            cfg.smtp_password.clone().unwrap_or_default(),
        ));
    }
    builder.build().send(email).await.map_err(|_|AppError::Unavailable("SMTP delivery failed. Check operator settings; outcome may be unknown and is not automatically retried.".into()))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_formats_disable_mentions_and_preserve_unknowns() {
        let slack = webhook_payload("slack", "<@everyone>&", &json!({}));
        assert_eq!(slack["text"], "&lt;@everyone&gt;&amp;");
        assert_eq!(slack["parse"], "none");
        assert_eq!(
            webhook_payload("discord", "@everyone", &json!({}))["allowed_mentions"]["parse"],
            json!([])
        );
        assert!(digest_text(&json!({"alerts":[]})).contains("Health: Unknown"));
    }
    #[test]
    fn movement_preserves_missing_and_method_boundaries() {
        let row = json!({"keyword":"seo","previous_measured":true,"before":10,"after":5,"provider":"fixture","previous_provider":"fixture","exact":true,"previous_exact":true,"depth":10,"previous_depth":10});
        assert_eq!(rank_changes(std::slice::from_ref(&row), 5, true).0.len(), 1);
        for (field, value) in [
            ("previous_measured", json!(false)),
            ("previous_exact", json!(false)),
            ("previous_depth", json!(9)),
            ("previous_provider", json!("other")),
        ] {
            let mut r = row.clone();
            r[field] = value;
            assert_eq!(rank_changes(&[r], 1, true).1, 0);
        }
        let mut r = row;
        r["before"] = Value::Null;
        let changes = rank_changes(&[r], 5, true);
        assert_eq!(changes.0[0]["kind"], "entered_depth");
        assert!(changes.0[0]["change"].is_null());
        assert!(endpoint_url("https://127.0.0.1/", "webhook", false).is_err());
        assert!(
            endpoint_url(
                "https://hooks.slack.com.evil.example/services/a",
                "slack",
                false
            )
            .is_err()
        );
    }
}
