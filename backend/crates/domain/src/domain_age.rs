//! On-demand registration age. Cache only minimal public metadata, never RDAP contacts.
use crate::{Actor, competitive::normalize_domain};
use chrono::Utc;
use common::{AppError, AppResult};
use providers::rdap::{self, RdapRecord, RdapStatus};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct DomainAgeInput {
    /// Defaults to the project's website; subdomains normalize to the registrable name.
    pub domain: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DomainAge {
    pub record: RdapRecord,
    /// Whole days since the registry's registration event, computed at read time. Unknown = null.
    pub age_days: Option<i64>,
    pub source: &'static str,
    pub confidence: Option<&'static str>,
    pub cached: bool,
}

fn view(record: RdapRecord, cached: bool) -> DomainAge {
    let age_days = record.registered_at.and_then(|at| {
        let elapsed = Utc::now().signed_duration_since(at);
        (elapsed.num_seconds() >= 0).then(|| elapsed.num_days())
    });
    DomainAge {
        record,
        age_days,
        source: "rdap",
        confidence: age_days.map(|_| "measured"),
        cached,
    }
}

pub async fn lookup(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    input: DomainAgeInput,
) -> AppResult<DomainAge> {
    // Authorization always precedes shared-cache access or external network requests.
    let p = db::projects::get(pool, actor.org_id, project_id).await?;
    let domain = normalize_domain(input.domain.as_deref().or(p.domain.as_deref()).ok_or_else(
        || AppError::Validation("enter a domain or set the project's website".into()),
    )?)?;
    // One day for all responses, including unsupported/404; no user refresh bypass.
    if let Some(value) = db::providers::cache_get(pool, "rdap_registration_v1", &domain, 1).await?
        && let Ok(record) = serde_json::from_value::<RdapRecord>(value)
        && record.domain == domain
    {
        return Ok(view(record, true));
    }
    let bootstrap = match db::providers::cache_get(pool, "rdap_bootstrap_v1", "iana_dns", 1).await?
    {
        Some(value) => value,
        None => {
            let value = rdap::bootstrap()
                .await
                .map_err(jobs::provider_ctx::provider_err)?;
            db::providers::cache_put(pool, "rdap_bootstrap_v1", "iana_dns", &value).await?;
            value
        }
    };
    let record = rdap::lookup(&domain, &bootstrap)
        .await
        .map_err(jobs::provider_ctx::provider_err)?;
    debug_assert!(record.registered_at.is_none() || record.status == RdapStatus::Found);
    db::providers::cache_put(
        pool,
        "rdap_registration_v1",
        &domain,
        &serde_json::to_value(&record).map_err(AppError::internal)?,
    )
    .await?;
    Ok(view(record, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn age_is_recomputed_from_event_and_unknown_stays_null() {
        let record = |date| RdapRecord {
            domain: "example.com".into(),
            status: RdapStatus::Found,
            registered_at: date,
            source_url: None,
            checked_at: Utc::now() - Duration::days(7),
            warning: None,
        };
        let known = view(record(Some(Utc::now() - Duration::days(100))), true);
        assert_eq!(known.age_days, Some(100));
        assert_eq!(known.confidence, Some("measured"));
        assert!(view(record(None), true).age_days.is_none());
        assert!(
            view(record(Some(Utc::now() + Duration::days(1))), true)
                .age_days
                .is_none()
        );
    }
}
