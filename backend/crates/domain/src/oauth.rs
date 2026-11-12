//! OAuth for the MCP resource only; these tokens never authenticate the REST API.
use crate::{Actor, ActorVia, Role};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::{AppError, AppResult, Config, token};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use utoipa::ToSchema;
pub fn issuer(cfg: &Config) -> String {
    cfg.web_base_url.trim_end_matches('/').into()
}
pub fn resource(cfg: &Config) -> String {
    format!("{}/mcp", issuer(cfg))
}
fn redirect_valid(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|u| {
        u.fragment().is_none()
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str().is_some()
            && (u.scheme() == "https"
                || (u.scheme() == "http"
                    && matches!(u.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"))))
    })
}
#[derive(Deserialize)]
pub struct Registration {
    pub client_name: Option<String>,
    pub redirect_uris: Vec<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
}
pub async fn register(pool: &PgPool, r: Registration) -> AppResult<serde_json::Value> {
    if r.redirect_uris.is_empty()
        || r.redirect_uris.len() > 10
        || r.redirect_uris
            .iter()
            .any(|u| u.len() > 2048 || !redirect_valid(u))
        || r.token_endpoint_auth_method
            .as_deref()
            .is_some_and(|s| s != "none")
        || r.grant_types.as_ref().is_some_and(|g| {
            g.iter()
                .any(|s| !matches!(s.as_str(), "authorization_code" | "refresh_token"))
        })
        || r.response_types.as_ref().is_some_and(|v| v != &["code"])
    {
        return Err(AppError::Validation("public clients require exact HTTPS or loopback redirect URIs, code/refresh grants and token_endpoint_auth_method=none".into()));
    }
    let name = crate::validate::non_empty(
        "client name",
        r.client_name.as_deref().unwrap_or("MCP client"),
        120,
    )?;
    let c = db::oauth::register(pool, &name, serde_json::json!(r.redirect_uris)).await?;
    Ok(
        serde_json::json!({"client_id":c.id,"client_name":c.name,"redirect_uris":c.redirect_uris,"token_endpoint_auth_method":"none","grant_types":["authorization_code","refresh_token"],"response_types":["code"]}),
    )
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Authorization {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub resource: String,
    pub scope: Option<String>,
    pub state: Option<String>,
}
#[derive(Serialize, ToSchema)]
pub struct ConsentView {
    pub client_name: String,
    pub redirect_uri: String,
    pub resource: String,
    pub scope: String,
}
pub async fn preview(pool: &PgPool, cfg: &Config, q: &Authorization) -> AppResult<ConsentView> {
    let c = db::oauth::client(pool, &q.client_id).await?;
    let allowed = c.redirect_uris.as_array().is_some_and(|a| {
        a.iter()
            .any(|v| v.as_str() == Some(q.redirect_uri.as_str()))
    });
    if !allowed
        || q.response_type != "code"
        || q.code_challenge_method != "S256"
        || q.code_challenge.len() != 43
        || !q
            .code_challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || q.resource != resource(cfg)
        || q.scope.as_deref().is_some_and(|s| s != "mcp")
        || q.state.as_ref().is_some_and(|s| s.len() > 2048)
    {
        return Err(AppError::Validation(
            "invalid authorization request, resource, redirect, scope or S256 challenge".into(),
        ));
    }
    Ok(ConsentView {
        client_name: c.name,
        redirect_uri: q.redirect_uri.clone(),
        resource: q.resource.clone(),
        scope: "mcp".into(),
    })
}
#[derive(Deserialize, ToSchema)]
pub struct ConsentInput {
    pub authorization: Authorization,
    pub allow: bool,
}
#[derive(Serialize, ToSchema)]
pub struct ConsentResult {
    pub redirect_uri: String,
}
pub async fn consent(
    pool: &PgPool,
    actor: &Actor,
    cfg: &Config,
    input: ConsentInput,
) -> AppResult<ConsentResult> {
    if actor.via != ActorVia::Session {
        return Err(AppError::Forbidden(
            "Sign in with a browser session to authorize this client".into(),
        ));
    }
    preview(pool, cfg, &input.authorization).await?;
    let q = input.authorization;
    let mut uri = url::Url::parse(&q.redirect_uri).map_err(AppError::internal)?;
    if input.allow {
        let code = token::random_token(32);
        db::oauth::save_code(
            pool,
            db::oauth::NewCode {
                hash: &token::sha256_hex(&code),
                client: &q.client_id,
                redirect: &q.redirect_uri,
                challenge: &q.code_challenge,
                resource: &q.resource,
                user: actor.user_id,
                org: actor.org_id,
            },
        )
        .await?;
        uri.query_pairs_mut().append_pair("code", &code);
    } else {
        uri.query_pairs_mut().append_pair("error", "access_denied");
    }
    if let Some(state) = q.state {
        uri.query_pairs_mut().append_pair("state", &state);
    }
    Ok(ConsentResult {
        redirect_uri: uri.into(),
    })
}
#[derive(Deserialize)]
pub struct TokenInput {
    pub grant_type: String,
    pub client_id: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub resource: String,
}
pub async fn exchange(
    pool: &PgPool,
    cfg: &Config,
    input: TokenInput,
) -> AppResult<db::oauth::TokenPair> {
    if input.resource != resource(cfg) {
        return Err(AppError::Unauthorized);
    }
    match input.grant_type.as_str() {
        "authorization_code" => {
            let verifier = input
                .code_verifier
                .filter(|v| {
                    (43..=128).contains(&v.len())
                        && v.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
                })
                .ok_or(AppError::Unauthorized)?;
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            db::oauth::exchange(
                pool,
                &token::sha256_hex(&input.code.ok_or(AppError::Unauthorized)?),
                &input.client_id,
                &input.redirect_uri.ok_or(AppError::Unauthorized)?,
                &challenge,
                &input.resource,
            )
            .await
        }
        "refresh_token" => {
            db::oauth::refresh(
                pool,
                &token::sha256_hex(&input.refresh_token.ok_or(AppError::Unauthorized)?),
                &input.client_id,
                &input.resource,
            )
            .await
        }
        _ => Err(AppError::Validation("unsupported_grant_type".into())),
    }
}
pub async fn authenticate(pool: &PgPool, cfg: &Config, secret: &str) -> AppResult<Option<Actor>> {
    if !secret.starts_with("mcp_") {
        return Ok(None);
    }
    let row = db::oauth::authenticate(pool, &token::sha256_hex(secret), &resource(cfg)).await?;
    Ok(row.and_then(|(user_id, org_id, r)| {
        Role::parse(&r).map(|role| Actor {
            user_id,
            org_id,
            role,
            via: ActorVia::OAuth,
        })
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirect_policy() {
        assert!(redirect_valid("https://client.example/callback"));
        assert!(redirect_valid("http://127.0.0.1:5111/callback"));
        for s in [
            "http://public.example/callback",
            "https://user:pass@client.example/",
            "javascript:alert(1)",
            "https://client.example/#fragment",
        ] {
            assert!(!redirect_valid(s), "{s}");
        }
    }
}
