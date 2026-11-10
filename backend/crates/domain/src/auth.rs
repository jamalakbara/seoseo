//! Accounts, sign-in and sessions.

use crate::actor::{Actor, ActorVia, Role};
use crate::validate;
use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use chrono::{Duration, Utc};
use common::token::{random_token, sha256_hex};
use common::{AppError, AppResult, new_id};
use db::models::{Membership, UserRow};
use db::{orgs, sessions, users};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

pub const NOAUTH_EMAIL: &str = "admin@localhost.invalid";

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignUpInput {
    pub email: String,
    pub name: String,
    pub password: String,
    /// Token from an invitation link: joins that workspace even when sign-up is closed.
    pub invite_token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignInInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserView {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_instance_admin: bool,
}

impl From<UserRow> for UserView {
    fn from(u: UserRow) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            is_instance_admin: u.is_instance_admin,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SignUpStatus {
    /// True when no account exists yet: the next sign-up becomes the instance admin.
    pub needs_setup: bool,
    pub signup_open: bool,
}

fn hash_password(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::internal(anyhow::anyhow!("hash password: {e}")))
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

pub async fn signup_status(pool: &PgPool, allow_signup: bool) -> AppResult<SignUpStatus> {
    let needs_setup = users::count(pool).await? == 0;
    Ok(SignUpStatus {
        needs_setup,
        signup_open: needs_setup || allow_signup,
    })
}

/// Create an account with its own workspace. The very first account becomes the instance admin.
pub async fn sign_up(pool: &PgPool, allow_signup: bool, input: SignUpInput) -> AppResult<UserView> {
    let email = validate::email(&input.email)?;
    let name = validate::non_empty("name", &input.name, 120)?;
    validate::password(&input.password)?;
    let hash = hash_password(&input.password)?;

    if let Some(token) = input.invite_token.as_deref().filter(|t| !t.is_empty()) {
        let (user_id, _org) =
            crate::team::sign_up_invited(pool, token, &email, &name, &hash).await?;
        return Ok(UserView {
            id: user_id,
            email,
            name,
            is_instance_admin: false,
        });
    }

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    // Serialize sign-ups so exactly one account can claim the admin seat.
    sqlx::query("SELECT pg_advisory_xact_lock(7461001)")
        .execute(&mut *tx)
        .await
        .map_err(AppError::internal)?;
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::internal)?;
    if existing > 0 && !allow_signup {
        return Err(AppError::Forbidden(
            "sign-up is closed on this instance; ask the admin for an invitation".into(),
        ));
    }
    let user_id = new_id();
    let org_id = new_id();
    users::insert(&mut *tx, user_id, &email, &name, Some(&hash), existing == 0).await?;
    orgs::insert(&mut *tx, org_id, &format!("{name}'s workspace")).await?;
    orgs::add_member(&mut *tx, org_id, user_id, "owner").await?;
    tx.commit().await.map_err(AppError::internal)?;

    Ok(UserView {
        id: user_id,
        email,
        name,
        is_instance_admin: existing == 0,
    })
}

pub async fn sign_in(pool: &PgPool, input: SignInInput) -> AppResult<UserView> {
    let email = validate::email(&input.email).map_err(|_| invalid_credentials())?;
    let user = users::find_by_email(pool, &email).await?;
    match user {
        Some(u)
            if u.password_hash
                .as_deref()
                .is_some_and(|h| verify_password(&input.password, h)) =>
        {
            Ok(u.into())
        }
        Some(_) => Err(invalid_credentials()),
        None => {
            // Spend comparable time so response timing does not reveal which emails exist.
            let _ = hash_password(&input.password);
            Err(invalid_credentials())
        }
    }
}

fn invalid_credentials() -> AppError {
    AppError::BadRequest("email or password is incorrect".into())
}

/// Create a session and return the raw token for the cookie. Only its hash is stored.
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
    ttl_days: i64,
) -> AppResult<String> {
    let org_id = orgs::memberships(pool, user_id)
        .await?
        .first()
        .map(|m| m.organization_id);
    let token = random_token(32);
    let expires = Utc::now() + Duration::days(ttl_days);
    let ua = user_agent.map(|s| s.chars().take(300).collect::<String>());
    sessions::insert(
        pool,
        &sha256_hex(&token),
        user_id,
        org_id,
        ua.as_deref(),
        expires,
    )
    .await?;
    Ok(token)
}

pub async fn end_session(pool: &PgPool, token: &str) -> AppResult<()> {
    sessions::delete(pool, &sha256_hex(token)).await
}

/// Resolve a session cookie to an actor bound to the session's active organization.
pub async fn actor_from_session(pool: &PgPool, token: &str) -> AppResult<Option<Actor>> {
    let Some(session) = sessions::touch(pool, &sha256_hex(token)).await? else {
        return Ok(None);
    };
    let memberships = orgs::memberships(pool, session.user_id).await?;
    let chosen = session
        .organization_id
        .and_then(|id| memberships.iter().find(|m| m.organization_id == id))
        .or_else(|| memberships.first());
    Ok(chosen.and_then(|m| actor_from_membership(session.user_id, m, ActorVia::Session)))
}

fn actor_from_membership(user_id: Uuid, m: &Membership, via: ActorVia) -> Option<Actor> {
    Some(Actor {
        user_id,
        org_id: m.organization_id,
        role: Role::parse(&m.role)?,
        via,
    })
}

/// Switch the session's active organization (must be a member).
pub async fn switch_org(pool: &PgPool, token: &str, user_id: Uuid, org_id: Uuid) -> AppResult<()> {
    if orgs::role(pool, org_id, user_id).await?.is_none() {
        return Err(AppError::NotFound("organization"));
    }
    sessions::set_org(pool, &sha256_hex(token), org_id).await
}

/// `AUTH_MODE=none`: make sure the built-in admin and its workspace exist, and return it.
pub async fn ensure_noauth_actor(pool: &PgPool) -> AppResult<Actor> {
    let user = match users::find_by_email(pool, NOAUTH_EMAIL).await? {
        Some(u) => u,
        None => {
            let mut tx = pool.begin().await.map_err(AppError::internal)?;
            let user_id = new_id();
            let org_id = new_id();
            users::insert(&mut *tx, user_id, NOAUTH_EMAIL, "Admin", None, true).await?;
            orgs::insert(&mut *tx, org_id, "Workspace").await?;
            orgs::add_member(&mut *tx, org_id, user_id, "owner").await?;
            tx.commit().await.map_err(AppError::internal)?;
            users::find(pool, user_id)
                .await?
                .ok_or(AppError::NotFound("user"))?
        }
    };
    let memberships = orgs::memberships(pool, user.id).await?;
    memberships
        .first()
        .and_then(|m| actor_from_membership(user.id, m, ActorVia::NoAuth))
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("no-auth admin has no workspace")))
}

/// Operator command: set a user's password (used by `seoseo admin reset-password`).
pub async fn admin_set_password(pool: &PgPool, email: &str, password: &str) -> AppResult<()> {
    validate::password(password)?;
    let user = users::find_by_email(pool, &validate::email(email)?)
        .await?
        .ok_or(AppError::NotFound("user"))?;
    users::set_password(pool, user.id, &hash_password(password)?).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeView {
    pub user: UserView,
    pub organization_id: Uuid,
    pub role: Role,
    pub via: ActorVia,
    pub memberships: Vec<Membership>,
}

pub async fn me(pool: &PgPool, actor: &Actor) -> AppResult<MeView> {
    let user = users::find(pool, actor.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(MeView {
        user: user.into(),
        organization_id: actor.org_id,
        role: actor.role,
        via: actor.via,
        memberships: orgs::memberships(pool, actor.user_id).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let h = hash_password("correct horse battery").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &h));
        assert!(!verify_password("wrong password!!", &h));
        assert!(!verify_password("x", "not-a-hash"));
    }
}
