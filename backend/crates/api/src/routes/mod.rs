mod api_keys;
mod audits;
mod auth;
mod jobs;
mod keywords;
mod projects;
mod providers;
mod rank;
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
        .merge(team::router())
        .merge(providers::router())
        .merge(keywords::router())
        .merge(rank::router())
}
