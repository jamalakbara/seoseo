//! Loads an organization's provider registry: saved settings + decrypted keys + environment.

use common::crypto::SecretBox;
use common::{AppError, AppResult, Config};
use providers::{EnvCfg, ProviderState, Registry};
use sqlx::PgPool;
use uuid::Uuid;

/// Operator-level provider configuration shared by the API and the worker.
#[derive(Clone, Default)]
pub struct ProviderCtx {
    pub serpapi_url: String,
    pub env: EnvCfg,
    /// `None` when `APP_ENCRYPTION_KEY` is not set: keys cannot be stored or read.
    pub secrets: Option<SecretBox>,
    pub cc_graphinfo_url: Option<String>,
}

impl ProviderCtx {
    pub fn from_config(cfg: &Config) -> anyhow::Result<Self> {
        let secrets = match cfg
            .app_encryption_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
        {
            Some(k) => Some(SecretBox::from_base64(k).map_err(|e| anyhow::anyhow!("{e}"))?),
            None => None,
        };
        Ok(Self {
            serpapi_url: cfg.serpapi_url.clone(),
            env: EnvCfg {
                renderer_url: cfg.renderer_url.clone().filter(|u| !u.is_empty()),
                searxng_url: cfg
                    .searxng_url
                    .clone()
                    .map(|u| u.trim().to_string())
                    .filter(|u| !u.is_empty()),
            },
            secrets,
            cc_graphinfo_url: cfg.cc_graphinfo_url.clone().filter(|u| !u.is_empty()),
        })
    }

    /// Saved provider states of an organization with secrets decrypted. A key that cannot be
    /// decrypted (rotated encryption key) is treated as missing rather than failing everything.
    pub async fn states(&self, pool: &PgPool, org_id: Uuid) -> AppResult<Vec<ProviderState>> {
        let rows = db::providers::list_settings(pool, org_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let secret = match (&self.secrets, r.secret_enc.as_deref()) {
                    (Some(sb), Some(enc)) => sb
                        .decrypt(enc)
                        .map_err(|e| {
                            tracing::warn!(provider = %r.provider, error = %e, "provider key unreadable")
                        })
                        .ok(),
                    _ => None,
                };
                ProviderState {
                    id: r.provider,
                    enabled: r.enabled,
                    secret,
                }
            })
            .collect())
    }

    pub async fn registry(&self, pool: &PgPool, org_id: Uuid) -> AppResult<Registry> {
        Ok(Registry::build(
            &self.env,
            &self.states(pool, org_id).await?,
        ))
    }
}

/// Map a provider failure to an application error with an actionable message.
pub fn provider_err(e: providers::ProviderError) -> AppError {
    use providers::ProviderError as P;
    match e {
        P::NotConfigured(m) => AppError::Conflict(m),
        other => AppError::Unavailable(other.to_string()),
    }
}
