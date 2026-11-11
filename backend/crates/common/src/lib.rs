//! Shared building blocks: configuration, errors, ids, token helpers.

pub mod config;
pub mod crypto;
pub mod error;
pub mod token;

pub use config::{AuthMode, Config};
pub use error::{AppError, AppResult};

/// Time-ordered UUID (v7) for primary keys.
pub fn new_id() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}
