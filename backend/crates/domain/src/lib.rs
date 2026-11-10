//! Business logic. Routers and MCP tools call services here; services call `db` repositories.

pub mod actor;
pub mod api_keys;
pub mod auth;
pub mod projects;
pub mod validate;

pub use actor::{Actor, ActorVia, Role};
