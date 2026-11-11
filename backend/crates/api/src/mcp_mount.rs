//! Mounts the MCP endpoint at `/mcp` behind the same authentication as the REST API
//! (API key, session cookie, or the built-in admin in `AUTH_MODE=none`).

use crate::auth::Auth;
use crate::state::AppState;
use axum::Router;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub fn router(state: &AppState) -> Router<AppState> {
    // Accept the public web host plus local and compose-internal names (DNS-rebinding guard).
    let mut hosts: Vec<String> = ["localhost", "127.0.0.1", "::1", "api", "web"]
        .map(String::from)
        .to_vec();
    if let Some(h) = url::Url::parse(&state.cfg.web_base_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
    {
        hosts.push(h);
    }
    let service = mcp::service(mcp::McpOptions {
        pool: state.pool.clone(),
        providers: state.providers.clone(),
        policy: crawler::UrlPolicy {
            allow_private: state.cfg.crawler_allow_private,
        },
        renderer_available: state
            .cfg
            .renderer_url
            .as_deref()
            .is_some_and(|u| !u.is_empty()),
        allowed_hosts: hosts,
    });
    Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            authenticate,
        ))
}

/// Resolve the caller and hand it to the tools through the request extensions.
async fn authenticate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let (mut parts, body) = req.into_parts();
    match Auth::from_request_parts(&mut parts, &state).await {
        Ok(Auth(actor)) => {
            parts.extensions.insert(actor);
            next.run(Request::from_parts(parts, body)).await
        }
        Err(_) => {
            let mut res = (
                StatusCode::UNAUTHORIZED,
                "MCP needs an API key: send `Authorization: Bearer seo_…` (create one under Settings → API keys)",
            )
                .into_response();
            res.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=\"seoseo\""),
            );
            res
        }
    }
}
