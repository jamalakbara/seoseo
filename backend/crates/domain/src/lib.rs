//! Business logic. Routers and MCP tools call services here; services call `db` repositories.

pub mod actor;
pub mod api_keys;
pub mod audits;
pub mod auth;
pub mod keywords;
pub mod projects;
pub mod providers;
pub mod rank;
pub mod team;
pub mod validate;

pub use actor::{Actor, ActorVia, Role};
