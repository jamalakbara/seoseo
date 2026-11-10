use crate::auth::{Auth, SESSION_COOKIE, clear_cookie, session_cookie};
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum_extra::extract::CookieJar;
use common::{AppError, AuthMode};
use domain::auth::{MeView, SignInInput, SignUpInput, SignUpStatus, UserView};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(auth_status))
        .routes(routes!(sign_up))
        .routes(routes!(sign_in))
        .routes(routes!(sign_out))
        .routes(routes!(get_me))
        .routes(routes!(switch_org))
}

fn require_password_mode(state: &AppState) -> Result<(), AppError> {
    if state.cfg.auth_mode == AuthMode::None {
        return Err(AppError::BadRequest(
            "sign-in is disabled on this instance (AUTH_MODE=none)".into(),
        ));
    }
    Ok(())
}

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

/// Whether sign-up is open and whether the instance still needs its first (admin) account.
#[utoipa::path(get, path = "/auth/status", tag = "auth", responses((status = OK, body = SignUpStatus)))]
async fn auth_status(State(state): State<AppState>) -> ApiResult<Json<SignUpStatus>> {
    Ok(Json(
        domain::auth::signup_status(&state.pool, state.cfg.allow_signup).await?,
    ))
}

/// Create an account (the first account becomes the instance admin) and sign in.
#[utoipa::path(post, path = "/auth/sign-up", tag = "auth", request_body = SignUpInput,
    responses((status = CREATED, body = UserView), (status = FORBIDDEN, body = Problem), (status = CONFLICT, body = Problem)))]
async fn sign_up(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(input): Json<SignUpInput>,
) -> ApiResult<(StatusCode, CookieJar, Json<UserView>)> {
    require_password_mode(&state)?;
    let user = domain::auth::sign_up(&state.pool, state.cfg.allow_signup, input).await?;
    let token = domain::auth::create_session(
        &state.pool,
        user.id,
        user_agent(&headers),
        state.cfg.session_ttl_days,
    )
    .await?;
    let jar = jar.add(session_cookie(
        token,
        state.cfg.cookie_secure,
        state.cfg.session_ttl_days,
    ));
    Ok((StatusCode::CREATED, jar, Json(user)))
}

#[utoipa::path(post, path = "/auth/sign-in", tag = "auth", request_body = SignInInput,
    responses((status = OK, body = UserView), (status = BAD_REQUEST, body = Problem)))]
async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(input): Json<SignInInput>,
) -> ApiResult<(CookieJar, Json<UserView>)> {
    require_password_mode(&state)?;
    let user = domain::auth::sign_in(&state.pool, input).await?;
    let token = domain::auth::create_session(
        &state.pool,
        user.id,
        user_agent(&headers),
        state.cfg.session_ttl_days,
    )
    .await?;
    let jar = jar.add(session_cookie(
        token,
        state.cfg.cookie_secure,
        state.cfg.session_ttl_days,
    ));
    Ok((jar, Json(user)))
}

#[utoipa::path(post, path = "/auth/sign-out", tag = "auth", responses((status = NO_CONTENT)))]
async fn sign_out(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<(StatusCode, CookieJar)> {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        domain::auth::end_session(&state.pool, c.value()).await?;
    }
    Ok((StatusCode::NO_CONTENT, jar.remove(clear_cookie())))
}

/// The signed-in user, active workspace and role.
#[utoipa::path(get, path = "/me", tag = "auth",
    responses((status = OK, body = MeView), (status = UNAUTHORIZED, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_me(State(state): State<AppState>, Auth(actor): Auth) -> ApiResult<Json<MeView>> {
    Ok(Json(domain::auth::me(&state.pool, &actor).await?))
}

#[derive(Deserialize, ToSchema)]
pub struct SwitchOrgInput {
    organization_id: Uuid,
}

/// Change the session's active workspace.
#[utoipa::path(post, path = "/orgs/switch", tag = "auth", request_body = SwitchOrgInput,
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = Problem)),
    security(("session" = [])))]
async fn switch_org(
    State(state): State<AppState>,
    jar: CookieJar,
    Auth(actor): Auth,
    Json(input): Json<SwitchOrgInput>,
) -> ApiResult<StatusCode> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            AppError::BadRequest("switching workspaces needs a browser session".into())
        })?;
    domain::auth::switch_org(&state.pool, &token, actor.user_id, input.organization_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
