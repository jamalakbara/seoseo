use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Serialize, ToSchema)]
pub struct Health {
    status: &'static str,
    version: &'static str,
    database: bool,
    cache: bool,
}

#[derive(Serialize, ToSchema)]
pub struct InstanceInfo {
    version: &'static str,
    auth_mode: &'static str,
}

/// Undocumented operational endpoints (liveness, readiness, Prometheus).
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
}

pub fn documented() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_info))
}

async fn check(state: &AppState) -> Health {
    let database = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let cache = match state.valkey.clone() {
        Some(mut conn) => redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .is_ok(),
        None => false,
    };
    Health {
        status: if database { "ok" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        database,
        cache,
    }
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let h = check(&state).await;
    let code = if h.database {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(h))
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    match &state.metrics {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (StatusCode::NOT_FOUND, String::new()),
    }
}

/// Instance metadata used by the web app before sign-in.
#[utoipa::path(get, path = "/info", tag = "system", responses((status = OK, body = InstanceInfo)))]
async fn get_info(State(state): State<AppState>) -> Json<InstanceInfo> {
    Json(InstanceInfo {
        version: env!("CARGO_PKG_VERSION"),
        auth_mode: match state.cfg.auth_mode {
            common::AuthMode::Password => "password",
            common::AuthMode::None => "none",
        },
    })
}
