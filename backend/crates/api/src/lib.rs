//! HTTP API (axum). Routers stay thin: extract → call `domain` service → serialize.

mod auth;
mod error;
mod middleware;
mod routes;
mod state;

pub use state::AppState;

use axum::Router;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "SeoSeo API",
        description = "Free, open-source SEO platform API. Authenticate with a session cookie (web app) \
                       or `Authorization: Bearer seo_…` API key.",
        license(name = "MIT")
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "system", description = "Health and metadata"),
        (name = "auth", description = "Accounts, sessions and workspaces"),
        (name = "api-keys", description = "Organization-bound API keys"),
        (name = "projects", description = "Projects and their default market"),
        (name = "jobs", description = "Background jobs"),
        (name = "audits", description = "Site audits: crawl, issues, pages"),
        (name = "team", description = "Workspace members and invitations"),
        (name = "providers", description = "Data providers, API keys and domain authority"),
        (name = "keywords", description = "Keyword research, saved keywords and SERP analysis")
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{
            ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme,
        };
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(auth::SESSION_COOKIE))),
        );
    }
}

/// Build the full application router.
pub fn router(state: AppState) -> Router {
    let (api_router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/api/v1", routes::v1())
        .split_for_parts();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(
            state
                .cfg
                .web_base_url
                .parse()
                .expect("WEB_BASE_URL must be a valid origin"),
        ))
        .allow_credentials(true)
        .allow_methods(tower_http::cors::AllowMethods::mirror_request())
        .allow_headers(tower_http::cors::AllowHeaders::mirror_request());

    let openapi_json = openapi.clone();
    Router::new()
        .merge(api_router)
        .merge(Scalar::with_url("/api/docs", openapi))
        .route(
            "/api/v1/openapi.json",
            axum::routing::get(move || async move { axum::Json(openapi_json) }),
        )
        .merge(routes::system())
        .layer(axum::middleware::from_fn(middleware::track_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60),
        ))
        .layer(cors)
        .with_state(state)
}

/// Serve until `shutdown` resolves.
pub async fn serve(
    state: AppState,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let addr = state.cfg.bind_addr.clone();
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// The OpenAPI document as JSON (used to generate the frontend client).
pub fn openapi_json() -> String {
    let (_, openapi) = OpenApiRouter::<AppState>::with_openapi(ApiDoc::openapi())
        .nest("/api/v1", routes::v1())
        .split_for_parts();
    openapi.to_pretty_json().expect("serialize openapi")
}

#[cfg(test)]
mod tests {
    /// openapi-typescript keys generated operations by id, so duplicates silently clobber types.
    #[test]
    fn operation_ids_are_unique() {
        let doc: serde_json::Value = serde_json::from_str(&super::openapi_json()).unwrap();
        let mut seen = std::collections::HashSet::new();
        for (path, item) in doc["paths"].as_object().unwrap() {
            for (method, op) in item.as_object().unwrap() {
                let id = op["operationId"].as_str().unwrap_or_default();
                assert!(
                    seen.insert(id.to_string()),
                    "duplicate operationId `{id}` at {method} {path}"
                );
            }
        }
    }
}
