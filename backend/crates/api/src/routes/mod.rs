mod api_keys;
mod audits;
mod auth;
mod dashboard;
mod gsc;
mod jobs;
mod keywords;
mod memory;
mod projects;
mod providers;
mod rank;
mod reports;
mod system;
mod team;

use crate::state::AppState;
use utoipa_axum::router::OpenApiRouter;

pub use system::router as system;

pub fn v1() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(system::documented())
        .merge(auth::router())
        .merge(api_keys::router())
        .merge(projects::router())
        .merge(jobs::router())
        .merge(audits::router())
        .merge(dashboard::router())
        .merge(team::router())
        .merge(providers::router())
        .merge(keywords::router())
        .merge(memory::router())
        .merge(rank::router())
        .merge(reports::router())
        .merge(gsc::router())
}
