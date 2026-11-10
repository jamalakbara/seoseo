//! Projects: one website + default market per project, scoped to an organization.

use crate::actor::{Actor, Role};
use crate::validate;
use common::{AppResult, new_id};
use db::models::Project;
use db::projects::{NewProject, ProjectPatch};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectInput {
    pub name: String,
    /// Root domain, e.g. `example.com`. URLs are accepted and reduced to the host.
    pub domain: Option<String>,
    pub brand_name: Option<String>,
    /// ISO 3166-1 alpha-2 (default `US`).
    pub country_code: Option<String>,
    /// ISO 639-1 (default `en`).
    pub language_code: Option<String>,
    pub location_name: Option<String>,
}

/// Partial update. Send `null` for nullable fields to clear them; omit to keep.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    #[serde(default, with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub domain: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub brand_name: Option<Option<String>>,
    pub country_code: Option<String>,
    pub language_code: Option<String>,
    #[serde(default, with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub location_name: Option<Option<String>>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(d).map(Some)
    }
}

fn opt_domain(v: Option<&str>) -> AppResult<Option<String>> {
    match v.map(str::trim) {
        None | Some("") => Ok(None),
        Some(d) => validate::domain(d).map(Some),
    }
}

pub async fn create(pool: &PgPool, actor: &Actor, input: CreateProjectInput) -> AppResult<Project> {
    actor.require(Role::Admin)?;
    let name = validate::non_empty("name", &input.name, 120)?;
    let domain = opt_domain(input.domain.as_deref())?;
    let brand = validate::optional("brand name", input.brand_name.as_deref(), 120)?;
    let country = validate::country(input.country_code.as_deref().unwrap_or("US"))?;
    let language = validate::language(input.language_code.as_deref().unwrap_or("en"))?;
    let location = validate::optional("location", input.location_name.as_deref(), 200)?;
    db::projects::insert(
        pool,
        NewProject {
            id: new_id(),
            org_id: actor.org_id,
            name: &name,
            domain: domain.as_deref(),
            brand_name: brand.as_deref(),
            country_code: &country,
            language_code: &language,
            location_name: location.as_deref(),
        },
    )
    .await
}

pub async fn list(pool: &PgPool, actor: &Actor, archived: bool) -> AppResult<Vec<Project>> {
    db::projects::list(pool, actor.org_id, archived).await
}

pub async fn get(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<Project> {
    db::projects::get(pool, actor.org_id, id).await
}

pub async fn update(
    pool: &PgPool,
    actor: &Actor,
    id: Uuid,
    input: UpdateProjectInput,
) -> AppResult<Project> {
    actor.require(Role::Admin)?;
    let name = input
        .name
        .as_deref()
        .map(|n| validate::non_empty("name", n, 120))
        .transpose()?;
    let domain = input
        .domain
        .as_ref()
        .map(|d| opt_domain(d.as_deref()))
        .transpose()?;
    let brand = input
        .brand_name
        .as_ref()
        .map(|b| validate::optional("brand name", b.as_deref(), 120))
        .transpose()?;
    let country = input
        .country_code
        .as_deref()
        .map(validate::country)
        .transpose()?;
    let language = input
        .language_code
        .as_deref()
        .map(validate::language)
        .transpose()?;
    let location = input
        .location_name
        .as_ref()
        .map(|l| validate::optional("location", l.as_deref(), 200))
        .transpose()?;
    db::projects::update(
        pool,
        actor.org_id,
        id,
        ProjectPatch {
            name: name.as_deref(),
            domain: domain.as_ref().map(|d| d.as_deref()),
            brand_name: brand.as_ref().map(|b| b.as_deref()),
            country_code: country.as_deref(),
            language_code: language.as_deref(),
            location_name: location.as_ref().map(|l| l.as_deref()),
        },
    )
    .await
}

pub async fn set_archived(
    pool: &PgPool,
    actor: &Actor,
    id: Uuid,
    archived: bool,
) -> AppResult<Project> {
    actor.require(Role::Admin)?;
    db::projects::set_archived(pool, actor.org_id, id, archived).await
}

pub async fn delete(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<()> {
    actor.require(Role::Admin)?;
    db::projects::delete_archived(pool, actor.org_id, id).await
}
