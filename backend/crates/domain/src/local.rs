//! Local business discovery and provider-neutral geo-grid tracker configuration.

use crate::actor::{Actor, Role};
use crate::validate;
use chrono::{DateTime, Utc};
use common::{AppError, AppResult, Config, new_id};
use db::local::{self, LocalTracker, NewLocalTracker};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_TRACKERS_PER_PROJECT: i64 = 10;
pub const MAX_KEYWORDS_PER_TRACKER: usize = 50;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BusinessCategory {
    pub id: &'static str,
    pub label: &'static str,
}

const CATEGORIES: &[BusinessCategory] = &[
    BusinessCategory {
        id: "restaurant",
        label: "Restaurant",
    },
    BusinessCategory {
        id: "cafe",
        label: "Cafe",
    },
    BusinessCategory {
        id: "dentist",
        label: "Dentist",
    },
    BusinessCategory {
        id: "doctor",
        label: "Doctor",
    },
    BusinessCategory {
        id: "pharmacy",
        label: "Pharmacy",
    },
    BusinessCategory {
        id: "lawyer",
        label: "Lawyer",
    },
    BusinessCategory {
        id: "plumber",
        label: "Plumber",
    },
    BusinessCategory {
        id: "electrician",
        label: "Electrician",
    },
    BusinessCategory {
        id: "hairdresser",
        label: "Hairdresser",
    },
    BusinessCategory {
        id: "hotel",
        label: "Hotel",
    },
    BusinessCategory {
        id: "supermarket",
        label: "Supermarket",
    },
    BusinessCategory {
        id: "fitness",
        label: "Fitness centre",
    },
    BusinessCategory {
        id: "school",
        label: "School",
    },
    BusinessCategory {
        id: "car_repair",
        label: "Car repair",
    },
    BusinessCategory {
        id: "real_estate",
        label: "Real estate",
    },
];

pub fn categories() -> &'static [BusinessCategory] {
    CATEGORIES
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BusinessSearchInput {
    /// Business name or distinguishing phrase. Search only runs on explicit submission.
    pub query: String,
    /// City, district or address context.
    pub near: String,
    /// Optional category id from `GET /local/categories`.
    pub category: Option<String>,
    /// 1-10 results; defaults to 5.
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LocalBusiness {
    pub source: String,
    pub source_id: String,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub category: Option<String>,
    pub website: Option<String>,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BusinessSearchResult {
    pub businesses: Vec<LocalBusiness>,
    pub cached: bool,
    pub attribution: String,
    pub note: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct NominatimRow {
    osm_type: String,
    osm_id: u64,
    display_name: String,
    name: Option<String>,
    lat: String,
    lon: String,
    category: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    extratags: Option<HashMap<String, String>>,
}

fn checked_text(name: &str, value: &str, max: usize) -> AppResult<String> {
    validate::non_empty(name, value, max)
}

pub async fn search_businesses(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    cfg: &Config,
    input: BusinessSearchInput,
) -> AppResult<BusinessSearchResult> {
    actor.require(Role::Member)?;
    let project = db::projects::get(pool, actor.org_id, project_id).await?;
    let query = checked_text("query", &input.query, 120)?;
    let near = checked_text("near", &input.near, 160)?;
    let limit = input.limit.unwrap_or(5);
    if !(1..=10).contains(&limit) {
        return Err(AppError::Validation(
            "limit must be between 1 and 10".into(),
        ));
    }
    let category = input
        .category
        .as_deref()
        .map(|id| {
            CATEGORIES
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| AppError::Validation("unknown local business category".into()))
        })
        .transpose()?;
    let phrase = match category {
        Some(c) => format!("{query}, {}, {near}", c.label),
        None => format!("{query}, {near}"),
    };
    let cache_key = common::token::sha256_hex(&format!(
        "{}|{}|{}|{}",
        cfg.nominatim_url,
        phrase.to_lowercase(),
        project.country_code.to_lowercase(),
        limit
    ));
    if let Some(value) =
        db::providers::cache_get(pool, "nominatim_business_v1", &cache_key, 30).await?
        && let Ok(mut cached) = serde_json::from_value::<BusinessSearchResult>(value)
    {
        cached.cached = true;
        return Ok(cached);
    }
    if !db::providers::claim_lookup(pool, "nominatim_query_claim_v1", &cache_key).await? {
        return Err(AppError::Conflict(
            "this uncached business lookup was just requested; retry shortly".into(),
        ));
    }
    if !db::providers::claim_lookup_seconds(pool, "nominatim_global_claim_v1", "global", 1).await? {
        return Err(AppError::Conflict(
            "business discovery is globally limited to one uncached request per second".into(),
        ));
    }
    let mut endpoint = url::Url::parse(&cfg.nominatim_url)
        .map_err(|_| AppError::Unavailable("invalid operator Nominatim endpoint".into()))?;
    endpoint
        .query_pairs_mut()
        .append_pair("q", &phrase)
        .append_pair("format", "jsonv2")
        .append_pair("addressdetails", "1")
        .append_pair("extratags", "1")
        .append_pair("namedetails", "1")
        .append_pair("layer", "poi")
        .append_pair("countrycodes", &project.country_code.to_lowercase())
        .append_pair("limit", &limit.to_string());
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(providers::USER_AGENT)
        .build()
        .map_err(AppError::internal)?;
    let mut response = client.get(endpoint).send().await.map_err(|_| {
        AppError::Unavailable("OpenStreetMap business discovery could not be reached".into())
    })?;
    if response.status().as_u16() == 429 {
        return Err(AppError::Unavailable(
            "OpenStreetMap business discovery is busy; retry later".into(),
        ));
    }
    if !response.status().is_success() {
        return Err(AppError::Unavailable(
            "OpenStreetMap business discovery returned an error".into(),
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("business discovery response was interrupted".into()))?
    {
        if bytes.len() + chunk.len() > 524_288 {
            return Err(AppError::Unavailable(
                "business discovery response exceeds the size limit".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let raw: Vec<NominatimRow> = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Unavailable("business discovery returned invalid data".into()))?;
    let businesses = raw
        .into_iter()
        .filter_map(|r| {
            let latitude = r
                .lat
                .parse::<f64>()
                .ok()
                .filter(|n| (-85.0..=85.0).contains(n))?;
            let longitude = r
                .lon
                .parse::<f64>()
                .ok()
                .filter(|n| (-180.0..=180.0).contains(n))?;
            if r.display_name.is_empty() || r.display_name.len() > 500 {
                return None;
            }
            let name = r
                .name
                .filter(|s| !s.is_empty() && s.len() <= 200)
                .unwrap_or_else(|| {
                    r.display_name
                        .split(',')
                        .next()
                        .unwrap_or("Business")
                        .trim()
                        .to_string()
                });
            let website = r
                .extratags
                .as_ref()
                .and_then(|t| t.get("website").or_else(|| t.get("contact:website")))
                .filter(|s| {
                    s.len() <= 2048
                        && url::Url::parse(s).is_ok_and(|u| {
                            matches!(u.scheme(), "http" | "https")
                                && u.host_str().is_some()
                                && u.username().is_empty()
                                && u.password().is_none()
                        })
                })
                .cloned();
            let prefix = match r.osm_type.as_str() {
                "node" => "node",
                "way" => "way",
                "relation" => "relation",
                _ => return None,
            };
            Some(LocalBusiness {
                source: "openstreetmap".into(),
                source_id: format!("{prefix}:{}", r.osm_id),
                name,
                address: r.display_name,
                latitude,
                longitude,
                category: r.kind.or(r.category),
                website,
                source_url: format!("https://www.openstreetmap.org/{prefix}/{}", r.osm_id),
            })
        })
        .take(limit.into())
        .collect();
    let result = BusinessSearchResult {
        businesses, cached: false, checked_at: Utc::now(),
        attribution: "© OpenStreetMap contributors, ODbL 1.0".into(),
        note: "Discovery matches from OpenStreetMap/Nominatim. Order and distance are not Google Maps or local-pack rankings.".into(),
    };
    db::providers::cache_put(
        pool,
        "nominatim_business_v1",
        &cache_key,
        &serde_json::to_value(&result).map_err(AppError::internal)?,
    )
    .await?;
    Ok(result)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLocalTrackerInput {
    pub name: Option<String>,
    pub business_source: String,
    pub business_id: String,
    pub business_name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub category: Option<String>,
    /// Odd grid dimension: 3, 5, 7 or 9.
    pub grid_size: Option<i16>,
    /// Distance between adjacent points, 0.1-10 km.
    pub spacing_km: Option<f64>,
    pub keywords: Vec<String>,
}

fn normalized_keywords(raw: &[String]) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for value in raw {
        let value = scoring::normalize_keyword(value);
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 120 || value.split_whitespace().count() > 12 {
            return Err(AppError::Validation(
                "local keywords can have at most 120 characters and 12 words".into(),
            ));
        }
        if !out.contains(&value) {
            out.push(value);
        }
    }
    if out.is_empty() || out.len() > MAX_KEYWORDS_PER_TRACKER {
        return Err(AppError::Validation(format!(
            "pass 1 to {MAX_KEYWORDS_PER_TRACKER} unique keywords"
        )));
    }
    Ok(out)
}

pub async fn create_tracker(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: CreateLocalTrackerInput,
) -> AppResult<LocalTracker> {
    actor.require(Role::Member)?;
    let project = db::projects::get(pool, actor.org_id, project_id).await?;
    if !["openstreetmap", "manual"].contains(&input.business_source.as_str()) {
        return Err(AppError::Validation(
            "business_source must be openstreetmap or manual".into(),
        ));
    }
    let grid_size = input.grid_size.unwrap_or(5);
    let spacing_km = input.spacing_km.unwrap_or(2.0);
    if ![3, 5, 7, 9].contains(&grid_size)
        || !spacing_km.is_finite()
        || !(0.1..=10.0).contains(&spacing_km)
        || !input.latitude.is_finite()
        || !(-85.0..=85.0).contains(&input.latitude)
        || !input.longitude.is_finite()
        || !(-180.0..=180.0).contains(&input.longitude)
    {
        return Err(AppError::Validation(
            "use a 3/5/7/9 grid, 0.1-10 km spacing and valid coordinates within ±85° latitude"
                .into(),
        ));
    }
    let business_name = checked_text("business_name", &input.business_name, 200)?;
    let name = checked_text("name", input.name.as_deref().unwrap_or(&business_name), 120)?;
    let business_id = checked_text("business_id", &input.business_id, 160)?;
    let address = checked_text("address", &input.address, 500)?;
    let category = input
        .category
        .as_deref()
        .map(|c| checked_text("category", c, 120))
        .transpose()?;
    let keywords = normalized_keywords(&input.keywords)?;
    let id = new_id();
    local::insert(
        pool,
        &NewLocalTracker {
            id,
            project_id,
            name: &name,
            business_source: &input.business_source,
            business_id: &business_id,
            business_name: &business_name,
            address: &address,
            latitude: input.latitude,
            longitude: input.longitude,
            category: category.as_deref(),
            grid_size,
            spacing_km,
            language: &project.language_code,
            created_by: actor.user_id,
        },
        &keywords,
        MAX_TRACKERS_PER_PROJECT,
    )
    .await?;
    local::get(pool, project_id, id).await
}

pub async fn list_trackers(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
) -> AppResult<Vec<LocalTracker>> {
    db::projects::get(pool, actor.org_id, project_id).await?;
    local::list(pool, project_id).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GridPoint {
    pub row: i16,
    pub column: i16,
    pub latitude: f64,
    pub longitude: f64,
    pub rank: Option<i16>,
    pub measured: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalGridPreview {
    pub tracker: LocalTracker,
    pub keywords: Vec<String>,
    pub points: Vec<GridPoint>,
    pub status: String,
    pub note: String,
}

pub(crate) fn points(t: &LocalTracker) -> Vec<GridPoint> {
    let half = t.grid_size / 2;
    let lat_step = t.spacing_km / 110.574;
    let lon_step = t.spacing_km / (111.320 * t.latitude.to_radians().cos());
    let mut points = Vec::with_capacity((t.grid_size * t.grid_size) as usize);
    for row in 0..t.grid_size {
        for column in 0..t.grid_size {
            points.push(GridPoint {
                row,
                column,
                latitude: t.latitude + f64::from(half - row) * lat_step,
                longitude: {
                    let lon = t.longitude + f64::from(column - half) * lon_step;
                    if (-180.0..=180.0).contains(&lon) {
                        lon
                    } else {
                        (lon + 180.0).rem_euclid(360.0) - 180.0
                    }
                },
                rank: None,
                measured: false,
            });
        }
    }
    points
}

pub async fn grid_preview(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<LocalGridPreview> {
    db::projects::get(pool, actor.org_id, project_id).await?;
    let tracker = local::get(pool, project_id, tracker_id).await?;
    let keywords = local::keywords(pool, tracker_id).await?;
    Ok(LocalGridPreview { points: points(&tracker), tracker, keywords, status: "unmeasured".into(),
        note: "Geometry preview only. No Google Maps or local-pack provider has measured these points, so rank remains unknown.".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_is_centered_and_unmeasured() {
        let t = LocalTracker {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            name: "x".into(),
            business_source: "manual".into(),
            business_id: "x".into(),
            business_name: "x".into(),
            address: "x".into(),
            latitude: -6.2,
            longitude: 106.8,
            category: None,
            grid_size: 5,
            spacing_km: 2.0,
            language: "id".into(),
            created_at: Utc::now(),
            keyword_count: 1,
        };
        let p = points(&t);
        assert_eq!(p.len(), 25);
        assert_eq!((p[12].latitude, p[12].longitude), (t.latitude, t.longitude));
        assert!(p.iter().all(|p| !p.measured && p.rank.is_none()));
    }
}
