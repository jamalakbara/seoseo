//! Request authentication: `AUTH_MODE=none`, bearer API keys, or session cookies.

use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::{Method, request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use common::{AppError, AuthMode};
use domain::{Actor, ActorVia};

pub const SESSION_COOKIE: &str = "seoseo_session";
/// Cookie-authenticated writes must carry this header. Browsers cannot add it cross-site
/// without a CORS preflight, which our CORS policy only allows for the web origin.
pub const CSRF_HEADER: &str = "x-seoseo-csrf";

/// Extractor for an authenticated caller.
pub struct Auth(pub Actor);

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.cfg.auth_mode == AuthMode::None {
            let actor = state
                .noauth_actor
                .get_or_try_init(|| domain::auth::ensure_noauth_actor(&state.pool))
                .await?;
            return Ok(Auth(actor.clone()));
        }

        if let Some(value) = parts.headers.get(axum::http::header::AUTHORIZATION) {
            let secret = value
                .to_str()
                .ok()
                .and_then(|v| v.strip_prefix("Bearer "))
                .ok_or(AppError::Unauthorized)?;
            let actor = domain::api_keys::authenticate(&state.pool, secret.trim())
                .await?
                .ok_or(AppError::Unauthorized)?;
            return Ok(Auth(actor));
        }

        let token = session_token(parts).ok_or(AppError::Unauthorized)?;
        let actor = domain::auth::actor_from_session(&state.pool, &token)
            .await?
            .ok_or(AppError::Unauthorized)?;
        require_csrf(parts, &actor)?;
        Ok(Auth(actor))
    }
}

pub fn session_token(parts: &Parts) -> Option<String> {
    CookieJar::from_headers(&parts.headers)
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
}

fn require_csrf(parts: &Parts, actor: &Actor) -> Result<(), AppError> {
    let safe = matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
    if actor.via == ActorVia::Session && !safe && !parts.headers.contains_key(CSRF_HEADER) {
        return Err(AppError::Forbidden(format!(
            "missing {CSRF_HEADER} header on a cookie-authenticated request"
        )));
    }
    Ok(())
}

pub fn session_cookie(token: String, secure: bool, ttl_days: i64) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(time_days(ttl_days))
        .build()
}

pub fn clear_cookie() -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .max_age(time_days(0))
        .build()
}

fn time_days(days: i64) -> time::Duration {
    time::Duration::days(days)
}
