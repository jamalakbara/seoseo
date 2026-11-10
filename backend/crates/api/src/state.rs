use common::Config;
use domain::Actor;
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub valkey: Option<redis::aio::ConnectionManager>,
    pub cfg: Arc<Config>,
    pub metrics: Option<PrometheusHandle>,
    /// Cached built-in admin for `AUTH_MODE=none`.
    pub(crate) noauth_actor: Arc<OnceCell<Actor>>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        valkey: Option<redis::aio::ConnectionManager>,
        cfg: Config,
        metrics: Option<PrometheusHandle>,
    ) -> Self {
        Self {
            pool,
            valkey,
            cfg: Arc::new(cfg),
            metrics,
            noauth_actor: Arc::new(OnceCell::new()),
        }
    }
}
