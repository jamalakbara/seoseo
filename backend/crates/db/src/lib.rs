//! Postgres access: pool, embedded migrations, models and repositories.
//! Repositories take explicit tenant ids (organization / project) on every call.

pub mod api_keys;
pub mod audits;
pub mod competitive;
pub mod dashboard;
pub mod gsc;
pub mod invitations;
pub mod jobs;
pub mod keywords;
pub mod memory;
pub mod models;
pub mod orgs;
pub mod projects;
pub mod providers;
pub mod rank;
pub mod reports;
pub mod serp;
pub mod sessions;
pub mod settings;
pub mod users;

use common::AppError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub use sqlx::PgPool as Pool;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn connect(url: &str, max_connections: u32) -> sqlx::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(url)
        .await?;
    Ok(pool)
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

/// Map a sqlx error to an application error, turning unique violations into conflicts.
pub fn map_err(e: sqlx::Error, conflict_msg: &str) -> AppError {
    if let sqlx::Error::Database(db) = &e
        && db.code().as_deref() == Some("23505")
    {
        return AppError::Conflict(conflict_msg.to_string());
    }
    AppError::internal(e)
}

pub(crate) fn internal(e: sqlx::Error) -> AppError {
    AppError::internal(e)
}

/// Escape `%`, `_` and `\` so user input is matched literally by `LIKE`/`ILIKE`.
pub(crate) fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub mod ga4;

pub mod oauth;

pub mod integrations;
