use crate::internal;
use common::{AppError, AppResult, new_id, token};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(sqlx::FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub redirect_uris: serde_json::Value,
}
pub async fn client(pool: &PgPool, id: &str) -> AppResult<Client> {
    sqlx::query_as("SELECT id,name,redirect_uris FROM mcp_oauth_clients WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or(AppError::Validation("unknown client_id".into()))
}
pub async fn register(pool: &PgPool, name: &str, uris: serde_json::Value) -> AppResult<Client> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SELECT pg_advisory_xact_lock(504010)")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM mcp_oauth_clients")
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM mcp_oauth_clients WHERE created_at>now()-interval '1 hour'",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if recent >= 100 {
        return Err(AppError::Unavailable(
            "Registration rate limit reached; try again later".into(),
        ));
    }
    if count >= 10000 {
        return Err(AppError::Unavailable(
            "client registration capacity reached; contact the operator".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO mcp_oauth_clients(id,name,redirect_uris) VALUES($1,$2,$3) RETURNING id,name,redirect_uris").bind(new_id().to_string()).bind(name).bind(uris).fetch_one(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(row)
}
#[derive(sqlx::FromRow)]
pub struct Code {
    pub client_id: String,
    pub redirect_uri: String,
    pub challenge: String,
    pub resource: String,
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub grant_id: Option<Uuid>,
}
pub struct NewCode<'a> {
    pub hash: &'a str,
    pub client: &'a str,
    pub redirect: &'a str,
    pub challenge: &'a str,
    pub resource: &'a str,
    pub user: Uuid,
    pub org: Uuid,
}
pub async fn save_code(pool: &PgPool, c: NewCode<'_>) -> AppResult<()> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("DELETE FROM mcp_oauth_codes WHERE expires_at<now()-interval '1 hour'")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(c.user)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM mcp_oauth_codes WHERE user_id=$1 AND expires_at>now()",
    )
    .bind(c.user)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if count > 100 {
        return Err(AppError::Conflict(
            "too many pending authorizations; wait five minutes".into(),
        ));
    }
    sqlx::query("INSERT INTO mcp_oauth_codes(hash,client_id,redirect_uri,challenge,resource,user_id,organization_id) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(c.hash).bind(c.client).bind(c.redirect).bind(c.challenge).bind(c.resource).bind(c.user).bind(c.org).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)
}
#[derive(Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u32,
    pub scope: &'static str,
}
async fn pair(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, grant: Uuid) -> AppResult<TokenPair> {
    let access = format!("mcp_{}", token::random_token(32));
    let refresh = format!("mcr_{}", token::random_token(32));
    sqlx::query("INSERT INTO mcp_oauth_tokens(access_hash,refresh_hash,grant_id) VALUES($1,$2,$3)")
        .bind(token::sha256_hex(&access))
        .bind(token::sha256_hex(&refresh))
        .bind(grant)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(TokenPair {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: 3600,
        scope: "mcp",
    })
}
// Invalid proof consumes the short-lived code; replay revokes any grant issued from it.
pub async fn exchange(
    pool: &PgPool,
    hash: &str,
    client: &str,
    redirect: &str,
    challenge: &str,
    resource: &str,
) -> AppResult<TokenPair> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let c:Code=sqlx::query_as("SELECT client_id,redirect_uri,challenge,resource,user_id,organization_id,grant_id FROM mcp_oauth_codes WHERE hash=$1 AND expires_at>now() FOR UPDATE").bind(hash).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::Unauthorized)?;
    if c.client_id != client || c.redirect_uri != redirect || c.resource != resource {
        return Err(AppError::Unauthorized);
    }
    if let Some(grant) = c.grant_id {
        sqlx::query("UPDATE mcp_oauth_grants SET revoked_at=now() WHERE id=$1")
            .bind(grant)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        return Err(AppError::Unauthorized);
    }
    if c.challenge != challenge {
        sqlx::query("DELETE FROM mcp_oauth_codes WHERE hash=$1")
            .bind(hash)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        return Err(AppError::Unauthorized);
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM members WHERE user_id=$1 AND organization_id=$2)",
    )
    .bind(c.user_id)
    .bind(c.organization_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if !exists {
        return Err(AppError::Unauthorized);
    }
    let grant = new_id();
    sqlx::query("INSERT INTO mcp_oauth_grants(id,client_id,user_id,organization_id,resource) VALUES($1,$2,$3,$4,$5)").bind(grant).bind(client).bind(c.user_id).bind(c.organization_id).bind(resource).execute(&mut *tx).await.map_err(internal)?;
    sqlx::query("UPDATE mcp_oauth_codes SET grant_id=$2 WHERE hash=$1")
        .bind(hash)
        .bind(grant)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let p = pair(&mut tx, grant).await?;
    tx.commit().await.map_err(internal)?;
    Ok(p)
}
pub async fn refresh(
    pool: &PgPool,
    hash: &str,
    client: &str,
    resource: &str,
) -> AppResult<TokenPair> {
    let mut tx = pool.begin().await.map_err(internal)?;
    let row:Option<(Uuid,bool)>=sqlx::query_as("SELECT g.id,t.refresh_used FROM mcp_oauth_tokens t JOIN mcp_oauth_grants g ON g.id=t.grant_id JOIN members m ON m.user_id=g.user_id AND m.organization_id=g.organization_id WHERE t.refresh_hash=$1 AND g.client_id=$2 AND g.resource=$3 AND g.revoked_at IS NULL AND g.created_at>now()-interval '90 days' AND t.refresh_expires_at>now() FOR UPDATE OF g,t").bind(hash).bind(client).bind(resource).fetch_optional(&mut *tx).await.map_err(internal)?;
    let (grant, used) = row.ok_or(AppError::Unauthorized)?;
    if used {
        sqlx::query("UPDATE mcp_oauth_grants SET revoked_at=now() WHERE id=$1")
            .bind(grant)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        return Err(AppError::Unauthorized);
    }
    sqlx::query("UPDATE mcp_oauth_tokens SET refresh_used=true WHERE refresh_hash=$1")
        .bind(hash)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let p = pair(&mut tx, grant).await?;
    tx.commit().await.map_err(internal)?;
    Ok(p)
}
pub async fn authenticate(
    pool: &PgPool,
    hash: &str,
    resource: &str,
) -> AppResult<Option<(Uuid, Uuid, String)>> {
    sqlx::query_as("SELECT g.user_id,g.organization_id,m.role FROM mcp_oauth_tokens t JOIN mcp_oauth_grants g ON g.id=t.grant_id JOIN members m ON m.user_id=g.user_id AND m.organization_id=g.organization_id WHERE t.access_hash=$1 AND t.access_expires_at>now() AND g.resource=$2 AND g.revoked_at IS NULL AND g.created_at>now()-interval '90 days'").bind(hash).bind(resource).fetch_optional(pool).await.map_err(internal)
}
pub async fn revoke_token(pool: &PgPool, hash: &str, client: &str) -> AppResult<()> {
    sqlx::query("UPDATE mcp_oauth_grants g SET revoked_at=now() FROM mcp_oauth_tokens t WHERE t.grant_id=g.id AND g.client_id=$2 AND (t.access_hash=$1 OR t.refresh_hash=$1)").bind(hash).bind(client).execute(pool).await.map_err(internal)?;
    Ok(())
}
#[derive(sqlx::FromRow, Serialize, utoipa::ToSchema)]
pub struct GrantView {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
pub async fn grants(pool: &PgPool, user: Uuid, org: Uuid) -> AppResult<Vec<GrantView>> {
    sqlx::query_as("SELECT g.id,c.name,g.created_at FROM mcp_oauth_grants g JOIN mcp_oauth_clients c ON c.id=g.client_id WHERE g.user_id=$1 AND g.organization_id=$2 AND g.revoked_at IS NULL ORDER BY g.created_at DESC LIMIT 100").bind(user).bind(org).fetch_all(pool).await.map_err(internal)
}
pub async fn revoke_grant(pool: &PgPool, user: Uuid, org: Uuid, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE mcp_oauth_grants SET revoked_at=now() WHERE id=$1 AND user_id=$2 AND organization_id=$3").bind(id).bind(user).bind(org).execute(pool).await.map_err(internal)?;
    Ok(())
}
