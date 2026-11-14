//! Opt-in diagnostics. No timer, identifiers, content, or built-in receiver.
use crate::{Actor, ActorVia};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
async fn admin(pool: &PgPool, actor: &Actor) -> AppResult<()> {
    if !matches!(actor.via, ActorVia::Session | ActorVia::NoAuth)
        || !db::users::find(pool, actor.user_id)
            .await?
            .is_some_and(|u| u.is_instance_admin)
    {
        return Err(AppError::Forbidden(
            "instance administrator browser session required".into(),
        ));
    }
    Ok(())
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryInput {
    pub enabled: bool,
}
#[derive(Serialize, ToSchema)]
pub struct TelemetryView {
    pub enabled: bool,
    pub receiver_configured: bool,
    pub payload: Value,
    pub note: String,
}
pub async fn status(pool: &PgPool, actor: &Actor, cfg: &Config) -> AppResult<TelemetryView> {
    admin(pool, actor).await?;
    let enabled = db::settings::get(pool, "telemetry_opt_in")
        .await?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(TelemetryView{enabled,receiver_configured:cfg.telemetry_url.as_ref().is_some_and(|s|!s.is_empty()),payload:json!({"schema":1,"product":"seoseo","version":env!("CARGO_PKG_VERSION")}),note:"Off by default. Send manually after opt-in to the operator-configured receiver. Payload contains only schema, product and version; the receiver also sees the server IP. No project, account, query or usage data. No automatic sends.".into()})
}
pub async fn save(
    pool: &PgPool,
    actor: &Actor,
    cfg: &Config,
    v: TelemetryInput,
) -> AppResult<TelemetryView> {
    admin(pool, actor).await?;
    db::settings::set(pool, "telemetry_opt_in", json!(v.enabled)).await?;
    status(pool, actor, cfg).await
}
pub async fn send(pool: &PgPool, actor: &Actor, cfg: &Config) -> AppResult<TelemetryView> {
    let view = status(pool, actor, cfg).await?;
    if !view.enabled {
        return Err(AppError::Conflict(
            "opt in before sending diagnostics".into(),
        ));
    }
    let url = url::Url::parse(cfg.telemetry_url.as_deref().unwrap_or("")).map_err(|_| {
        AppError::Conflict("operator must set TELEMETRY_URL to a public HTTPS receiver".into())
    })?;
    if url.scheme() != "https" {
        return Err(AppError::Conflict(
            "telemetry receiver must use HTTPS".into(),
        ));
    }
    let policy = crawler::UrlPolicy::default();
    policy
        .check_url(&url)
        .map_err(|e| AppError::Conflict(e.to_string()))?;
    let r = crawler::fetch::guarded_client(policy, std::time::Duration::from_secs(10))
        .map_err(AppError::internal)?
        .post(url)
        .json(&view.payload)
        .send()
        .await
        .map_err(|_| {
            AppError::Unavailable("diagnostic delivery failed; no automatic retry".into())
        })?;
    if !r.status().is_success() {
        return Err(AppError::Unavailable(format!(
            "diagnostic receiver returned HTTP {}; no automatic retry",
            r.status().as_u16()
        )));
    }
    Ok(view)
}
