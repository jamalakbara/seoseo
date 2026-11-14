//! Data provider settings (per workspace) and the instance-wide domain authority data.

use crate::actor::{Actor, Role};
use chrono::{DateTime, Utc};
use common::{AppError, AppResult};
use db::models::Job;
use db::providers::SecretChange;
use jobs::ProviderCtx;
use jobs::cc_ranks::{self, AuthorityStatus};
use providers::catalog::{self, CATALOG, ProviderInfo, ProviderKind};
use providers::registry::availability;
use providers::{Market, ProviderState, SerpQuery, SuggestProvider};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Instant;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderView {
    #[serde(flatten)]
    pub info: ProviderInfo,
    pub enabled: bool,
    /// Enabled and fully configured: research will use it.
    pub ready: bool,
    /// An API key is stored (the key itself is never returned).
    pub has_key: bool,
    /// What is missing before it can be used.
    pub missing: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

fn view(
    info: &ProviderInfo,
    pctx: &ProviderCtx,
    state: Option<&ProviderState>,
    updated_at: Option<DateTime<Utc>>,
) -> ProviderView {
    let a = availability(info, &pctx.env, state);
    ProviderView {
        info: *info,
        enabled: a.enabled,
        ready: a.ready,
        has_key: state.is_some_and(|s| s.secret.is_some()),
        missing: a.missing,
        updated_at,
    }
}

pub async fn list(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
) -> AppResult<Vec<ProviderView>> {
    let states = pctx.states(pool, actor.org_id).await?;
    let rows = db::providers::list_settings(pool, actor.org_id).await?;
    Ok(CATALOG
        .iter()
        .map(|info| {
            view(
                info,
                pctx,
                states.iter().find(|s| s.id == info.id),
                rows.iter()
                    .find(|r| r.provider == info.id)
                    .map(|r| r.updated_at),
            )
        })
        .collect())
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateProviderInput {
    pub enabled: Option<bool>,
    /// New API key. Send an empty string to remove the stored key; omit to keep it.
    pub api_key: Option<String>,
}

pub async fn update(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
    provider: &str,
    input: UpdateProviderInput,
) -> AppResult<ProviderView> {
    actor.require(Role::Admin)?;
    let info = catalog::info(provider).ok_or(AppError::NotFound("provider"))?;
    let secret = match input.api_key.as_deref().map(str::trim) {
        None => SecretChange::Keep,
        Some("") => SecretChange::Clear,
        Some(key) => {
            if !info.needs_key {
                return Err(AppError::Validation(format!(
                    "{} does not use an API key",
                    info.name
                )));
            }
            if key.len() > 512 || key.chars().any(char::is_whitespace) {
                return Err(AppError::Validation(
                    "the API key must be a single token of at most 512 characters".into(),
                ));
            }
            let sb = pctx.secrets.as_ref().ok_or_else(|| {
                AppError::Conflict(
                    "API keys cannot be stored until the server operator sets APP_ENCRYPTION_KEY \
                     (openssl rand -base64 32) and restarts the stack"
                        .into(),
                )
            })?;
            SecretChange::Set(sb.encrypt(key).map_err(AppError::internal)?)
        }
    };
    db::providers::upsert_setting(
        pool,
        actor.org_id,
        info.id,
        input.enabled,
        secret,
        actor.user_id,
    )
    .await?;
    list(pool, actor, pctx)
        .await?
        .into_iter()
        .find(|v| v.info.id == info.id)
        .ok_or(AppError::NotFound("provider"))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderTest {
    pub ok: bool,
    /// What happened, phrased for a person.
    pub message: String,
    pub latency_ms: u64,
}

/// Make one small live request to check the provider works with the saved settings.
pub async fn test(
    pool: &PgPool,
    actor: &Actor,
    pctx: &ProviderCtx,
    provider: &str,
) -> AppResult<ProviderTest> {
    actor.require(Role::Admin)?;
    let info = catalog::info(provider).ok_or(AppError::NotFound("provider"))?;
    let states = pctx.states(pool, actor.org_id).await?;
    // Test this provider even if it is switched off, as long as it is configured.
    let mut forced: Vec<ProviderState> = states
        .into_iter()
        .filter(|s| s.id == info.id)
        .map(|s| ProviderState {
            enabled: Some(true),
            ..s
        })
        .collect();
    if forced.is_empty() {
        forced.push(ProviderState {
            id: info.id.to_string(),
            enabled: Some(true),
            secret: None,
        });
    }
    // Everything else off, so the registry holds only the provider under test.
    forced.extend(
        CATALOG
            .iter()
            .filter(|p| p.id != info.id)
            .map(|p| ProviderState {
                id: p.id.to_string(),
                enabled: Some(false),
                secret: None,
            }),
    );
    if let Some(missing) = availability(info, &pctx.env, forced.first()).missing {
        return Ok(ProviderTest {
            ok: false,
            message: format!("Not set up yet: {missing}."),
            latency_ms: 0,
        });
    }
    let registry = providers::Registry::build(&pctx.env, &forced);
    let market = Market::new("US", "en");
    let started = Instant::now();
    let result: Result<String, providers::ProviderError> = match info.kind {
        ProviderKind::Observations => {
            let secret = forced
                .first()
                .and_then(|s| s.secret.as_deref())
                .unwrap_or_default();
            providers::serpapi::request(&pctx.serpapi_url,secret,&[("engine","google".into()),("q","open source software".into())])
                .await.map(|_|"Provider accepted one test search. Maps and AI Overview availability depend on the query.".into())
        }
        ProviderKind::Suggest => match registry.suggest.first() {
            Some(p) => SuggestProvider::suggest(p.as_ref(), "seo", &market)
                .await
                .map(|l| format!("Returned {} suggestions.", l.len())),
            None => Ok("Provider is not available.".into()),
        },
        ProviderKind::Serp => registry
            .search(&SerpQuery {
                keyword: "open source software".into(),
                market,
            })
            .await
            .map(|p| format!("Returned {} results.", p.items.len())),
        ProviderKind::Demand => providers::wikipedia::resolve_titles("en", &["Earth".to_string()])
            .await
            .map(|m| format!("Resolved {} of 1 test article.", m.len())),
        ProviderKind::Authority => {
            providers::commoncrawl::latest_release(pctx.cc_graphinfo_url.as_deref())
                .await
                .map(|r| format!("Latest release {} with {} domains.", r.id, r.total_domains))
        }
    };
    let latency_ms = started.elapsed().as_millis() as u64;
    Ok(match result {
        Ok(message) => ProviderTest {
            ok: true,
            message,
            latency_ms,
        },
        Err(e) => ProviderTest {
            ok: false,
            message: e.to_string(),
            latency_ms,
        },
    })
}

// ---------------------------------------------------------------- domain authority

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthorityView {
    /// Last finished import; null when nothing was imported yet.
    pub status: Option<AuthorityStatus>,
    /// Running or queued import job, if any (poll `/jobs/{id}` for progress).
    pub active_job: Option<Job>,
}

pub async fn authority(pool: &PgPool, actor: &Actor) -> AppResult<AuthorityView> {
    let status = db::settings::get(pool, cc_ranks::STATUS_KEY)
        .await?
        .and_then(|v| serde_json::from_value(v).ok());
    Ok(AuthorityView {
        status,
        active_job: db::jobs::active_of_kind(pool, cc_ranks::KIND)
            .await?
            // Progress is not sensitive, but other workspaces' job rows are not ours to show.
            .filter(|j| j.organization_id == Some(actor.org_id)),
    })
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct ImportAuthorityInput {
    /// How many top domains to import (1,000–20,000,000; default 1,000,000 ≈ 30 MB download).
    pub limit: Option<u32>,
}

pub async fn import_authority(
    pool: &PgPool,
    actor: &Actor,
    input: ImportAuthorityInput,
) -> AppResult<Job> {
    actor.require(Role::Owner)?;
    let limit = input.limit.unwrap_or(cc_ranks::DEFAULT_LIMIT);
    if !(1_000..=cc_ranks::MAX_LIMIT).contains(&limit) {
        return Err(AppError::Validation(format!(
            "limit must be between 1000 and {}",
            cc_ranks::MAX_LIMIT
        )));
    }
    if db::jobs::active_of_kind(pool, cc_ranks::KIND)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "a domain authority import is already running".into(),
        ));
    }
    db::jobs::enqueue(
        pool,
        common::new_id(),
        cc_ranks::KIND,
        Some(actor.org_id),
        None,
        serde_json::json!({ "limit": limit }),
        3,
    )
    .await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DomainAuthority {
    /// Registrable domain that was looked up.
    pub domain: String,
    /// SeoSeo Rank 0–100; null when the domain is outside the imported list.
    pub sr: Option<i32>,
    /// Position by harmonic centrality in the Common Crawl domain graph.
    pub harmonic_rank: Option<i64>,
    pub pagerank_rank: Option<i64>,
    pub source: &'static str,
    pub release: Option<String>,
    /// False until ranks were imported; then a null `sr` means "weaker than every imported domain".
    pub authority_loaded: bool,
}

pub async fn lookup_authority(pool: &PgPool, input: &str) -> AppResult<DomainAuthority> {
    let host = crate::validate::domain(input)?;
    let domain = scoring::registrable_domain(&host).unwrap_or(host);
    let row = db::providers::domain_ranks(pool, std::slice::from_ref(&domain))
        .await?
        .into_iter()
        .next();
    let loaded = db::providers::domain_rank_stats(pool).await?.is_some();
    Ok(DomainAuthority {
        domain,
        sr: row.as_ref().map(|r| i32::from(r.sr)),
        harmonic_rank: row.as_ref().map(|r| r.harmonic_rank),
        pagerank_rank: row.as_ref().map(|r| r.pagerank_rank),
        source: providers::commoncrawl::ID,
        release: row.map(|r| r.release),
        authority_loaded: loaded,
    })
}
