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
        cfg: state.cfg.clone(),
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
    // Public browser MCP clients must present a bearer; never expose cross-origin cookies.
    if parts
        .headers
        .get(header::ORIGIN)
        .is_some_and(|o| o.as_bytes() != state.cfg.web_base_url.as_bytes())
        && !parts.headers.contains_key(header::AUTHORIZATION)
    {
        return challenge(&state);
    }

    if let Some(secret) = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|s| s.starts_with("mcp_"))
    {
        match domain::oauth::authenticate(&state.pool, &state.cfg, secret).await {
            Ok(Some(actor)) => {
                parts.extensions.insert(actor);
                return next.run(Request::from_parts(parts, body)).await;
            }
            _ => return challenge(&state),
        }
    }
    match Auth::from_request_parts(&mut parts, &state).await {
        Ok(Auth(actor)) => {
            parts.extensions.insert(actor);
            next.run(Request::from_parts(parts, body)).await
        }
        Err(_) => challenge(&state),
    }
}
fn challenge(state: &AppState) -> Response {
    let mut res = (
        StatusCode::UNAUTHORIZED,
        "MCP requires an OAuth token or a Settings API key",
    )
        .into_response();
    let value = format!(
        "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"mcp\"",
        domain::oauth::issuer(&state.cfg)
    );
    if let Ok(value) = HeaderValue::from_str(&value) {
        res.headers_mut().insert(header::WWW_AUTHENTICATE, value);
    }
    res
}
