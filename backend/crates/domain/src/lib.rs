//! Business logic. Routers and MCP tools call services here; services call `db` repositories.

pub mod actor;
pub mod api_keys;
pub mod audits;
pub mod auth;
pub mod dashboard;
pub mod gsc;
pub mod keywords;
pub mod memory;
pub mod onboarding;
pub mod projects;
pub mod providers;
pub mod rank;
pub mod reports;
pub mod team;
pub mod validate;

pub use actor::{Actor, ActorVia, Role};

pub mod ga4;

pub mod oauth;

pub mod integrations;
