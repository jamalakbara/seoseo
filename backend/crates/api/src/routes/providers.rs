use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use db::models::Job;
use domain::providers::{
    AuthorityView, DomainAuthority, ImportAuthorityInput, ProviderTest, ProviderView,
    UpdateProviderInput,
};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_providers))
        .routes(routes!(update_provider))
        .routes(routes!(test_provider))
        .routes(routes!(get_authority))
        .routes(routes!(import_authority))
        .routes(routes!(lookup_authority))
}

/// Data providers with their status for the active workspace. Free providers are on by
/// default; API keys are never returned.
#[utoipa::path(get, path = "/providers", tag = "providers",
    responses((status = OK, body = Vec<ProviderView>)), security(("session" = []), ("bearer" = [])))]
async fn list_providers(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<Vec<ProviderView>>> {
    Ok(Json(
        domain::providers::list(&state.pool, &actor, &state.providers).await?,
    ))
}

/// Switch a provider on or off, or set/remove its API key (admins).
#[utoipa::path(put, path = "/providers/{provider}", tag = "providers",
    params(("provider" = String, Path)), request_body = UpdateProviderInput,
    responses((status = OK, body = ProviderView), (status = NOT_FOUND, body = Problem),
              (status = CONFLICT, body = Problem), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_provider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(provider): Path<String>,
    Json(input): Json<UpdateProviderInput>,
) -> ApiResult<Json<ProviderView>> {
    Ok(Json(
        domain::providers::update(&state.pool, &actor, &state.providers, &provider, input).await?,
    ))
}

/// Make one small live request to check a provider works (admins).
#[utoipa::path(post, path = "/providers/{provider}/test", tag = "providers",
    params(("provider" = String, Path)),
    responses((status = OK, body = ProviderTest), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn test_provider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(provider): Path<String>,
) -> ApiResult<Json<ProviderTest>> {
    Ok(Json(
        domain::providers::test(&state.pool, &actor, &state.providers, &provider).await?,
    ))
}

/// Status of the domain authority data (SeoSeo Rank from the Common Crawl web graph).
#[utoipa::path(get, path = "/authority", tag = "providers",
    responses((status = OK, body = AuthorityView)), security(("session" = []), ("bearer" = [])))]
async fn get_authority(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> ApiResult<Json<AuthorityView>> {
    Ok(Json(
        domain::providers::authority(&state.pool, &actor).await?,
    ))
}

/// Import the top domains of the latest Common Crawl web graph (owners). Runs in the
/// background; follow it with `GET /jobs/{id}`.
#[utoipa::path(post, path = "/authority/import", tag = "providers",
    request_body = ImportAuthorityInput,
    responses((status = ACCEPTED, body = Job), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn import_authority(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(input): Json<ImportAuthorityInput>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(domain::providers::import_authority(&state.pool, &actor, input).await?),
    ))
}

#[derive(Deserialize, IntoParams)]
struct LookupQuery {
    /// Domain or URL, e.g. `example.com`.
    domain: String,
}

/// SeoSeo Rank of a domain.
#[utoipa::path(get, path = "/authority/lookup", tag = "providers", params(LookupQuery),
    responses((status = OK, body = DomainAuthority), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn lookup_authority(
    State(state): State<AppState>,
    Auth(_actor): Auth,
    Query(q): Query<LookupQuery>,
) -> ApiResult<Json<DomainAuthority>> {
    Ok(Json(
        domain::providers::lookup_authority(&state.pool, &q.domain).await?,
    ))
}
