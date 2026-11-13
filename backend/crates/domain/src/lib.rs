//! Business logic. Routers and MCP tools call services here; services call `db` repositories.

pub mod actor;
pub mod ai_index;
pub mod ai_visibility;
pub mod api_keys;
pub mod audit_compare;
pub mod audit_schedule;
pub mod audits;
pub mod auth;
pub mod backlinks;
pub mod cannibalization;
pub mod clustering;
pub mod competitive;
pub mod crux;
pub mod dashboard;
pub mod domain_age;
pub mod gsc;
pub mod internal_links;
pub mod keywords;
pub mod link_intersect;
pub mod memory;
pub mod onboarding;
pub mod projects;
pub mod providers;
pub mod rank;
pub mod reports;
pub mod team;
pub mod technology;
pub mod validate;

pub use actor::{Actor, ActorVia, Role};

pub mod ga4;

pub mod oauth;

pub mod integrations;
