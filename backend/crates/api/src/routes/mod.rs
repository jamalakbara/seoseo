mod api_keys;
mod auth;
mod jobs;
mod projects;
mod system;

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
}
