//! Built-in evidence reports, isolated PDF export, local business records and account lifecycle.
use crate::{Actor, ActorVia, Role, validate};
use common::{AppError, AppResult, Config};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;
pub async fn profile(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
) -> AppResult<Option<db::operations::BusinessProfile>> {
    db::operations::profile(pool, a.org_id, pid).await
}
pub async fn business_entries(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    q: crate::notifications::AlertQuery,
) -> AppResult<Vec<db::operations::BusinessEntry>> {
    let limit = q.limit.unwrap_or(25);
    let offset = q.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || offset < 0 {
        return Err(AppError::Validation("Invalid pagination".into()));
    }
    db::operations::business_entries(pool, a.org_id, pid, limit, offset).await
}
pub async fn delete_business_entry(pool: &PgPool, a: &Actor, pid: Uuid, id: Uuid) -> AppResult<()> {
    a.require(Role::Admin)?;
    db::operations::delete_business_entry(pool, a.org_id, pid, id).await
}
pub async fn account_deletion(
    pool: &PgPool,
    a: &Actor,
) -> AppResult<Option<db::operations::AccountDeletion>> {
    db::operations::deletion(pool, a.user_id).await
}
pub async fn cancel_account_deletion(pool: &PgPool, a: &Actor) -> AppResult<()> {
    if a.via != ActorVia::Session {
        return Err(AppError::Forbidden(
            "Cancel account deletion from a browser session".into(),
        ));
    }
    db::operations::cancel_deletion(pool, a.user_id).await
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacesDetailsInput {
    pub place_id: String,
    pub include_reviews: bool,
}
#[derive(Serialize, ToSchema)]
pub struct PlacesDetailsView {
    pub place_id: String,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub data: serde_json::Value,
    pub note: String,
}
pub async fn places_details(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    cfg: &Config,
    v: PlacesDetailsInput,
) -> AppResult<PlacesDetailsView> {
    a.require(Role::Admin)?;
    crate::projects::get(pool, a, pid).await?;
    if v.place_id.is_empty()
        || v.place_id.len() > 200
        || !v
            .place_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::Validation("Use an exact Google Place ID".into()));
    }
    let key = cfg
        .google_places_api_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Conflict(
                "Operator must configure GOOGLE_PLACES_API_KEY for optional live Places details"
                    .into(),
            )
        })?;
    let mask = if v.include_reviews {
        "id,displayName,formattedAddress,nationalPhoneNumber,websiteUri,regularOpeningHours,rating,userRatingCount,googleMapsUri,reviews,attributions"
    } else {
        "id,displayName,formattedAddress,nationalPhoneNumber,websiteUri,regularOpeningHours,googleMapsUri,attributions"
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(AppError::internal)?;
    let mut r = client
        .get(format!(
            "{}/{}",
            cfg.places_api_url.trim_end_matches('/'),
            v.place_id
        ))
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", mask)
        .send()
        .await
        .map_err(|_| AppError::Unavailable("Places request failed; no retry attempted".into()))?;
    if !r.status().is_success() {
        return Err(AppError::Unavailable(format!(
            "Places returned HTTP {}; stopped without retry",
            r.status().as_u16()
        )));
    }
    let mut raw = vec![];
    while let Some(chunk) = r
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("Places response interrupted".into()))?
    {
        if raw.len() + chunk.len() > 256 * 1024 {
            return Err(AppError::Unavailable(
                "Places response exceeded 256 KiB".into(),
            ));
        }
        raw.extend_from_slice(&chunk);
    }
    let data: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|_| AppError::Unavailable("Invalid Places response".into()))?;
    Ok(PlacesDetailsView{place_id:v.place_id,observed_at:chrono::Utc::now(),data,note:"Google Maps / Places API. Transient read-only response, not saved by SeoSeo. Reviews are a provider-selected sample (up to five), not all reviews. Q&A, responses and updates below are local records/drafts; this does not publish to Google Business Profile.".into()})
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerateReportInput {
    pub kind: String,
    pub title: Option<String>,
    pub byline: Option<String>,
    pub accent: Option<String>,
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub async fn generate_report(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    v: GenerateReportInput,
) -> AppResult<db::models::Report> {
    a.require(Role::Admin)?;
    let p = crate::projects::get(pool, a, pid).await?;
    if p.archived_at.is_some() {
        return Err(AppError::Conflict(
            "Restore the project before generating reports".into(),
        ));
    }
    let data = db::operations::report_evidence(pool, &p, &v.kind).await?;
    let title = validate::non_empty(
        "title",
        v.title
            .as_deref()
            .unwrap_or(&format!("{} report — {}", v.kind, p.name)),
        200,
    )?;
    let byline = validate::optional("byline", v.byline.as_deref(), 200)?.unwrap_or_default();
    let accent = v.accent.unwrap_or_else(|| "#2563eb".into());
    if accent.len() != 7
        || !accent.starts_with('#')
        || !accent[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(AppError::Validation(
            "Accent must be a six-digit hex color".into(),
        ));
    }
    let evidence = data["evidence"].clone();
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font:14px Arial,sans-serif;color:#172033;margin:36px}}h1,h2{{color:{accent}}}table{{border-collapse:collapse;width:100%;margin:20px 0}}td,th{{padding:8px;border:1px solid #ddd;text-align:left;vertical-align:top;overflow-wrap:anywhere}}pre{{font-size:11px;white-space:pre-wrap;overflow-wrap:anywhere}}small{{color:#526070}}@media print{{body{{margin:0}}details{{display:block}}}}</style></head><body><h1>{}</h1><p>{}</p><p>Website: {} · Market: {} / {}</p><small>Generated {}. Stored evidence only. Unknown values remain unknown. Bounded samples do not establish full-site or total-market coverage.</small><h2>Evidence</h2>",
        escape(&title),
        escape(&title),
        escape(&byline),
        escape(p.domain.as_deref().unwrap_or("Not set")),
        escape(&p.country_code),
        escape(&p.language_code),
        chrono::Utc::now()
    );
    let report_rows = evidence["rows"]
        .as_array()
        .cloned()
        .or_else(|| evidence.is_object().then(|| vec![evidence.clone()]));
    if let Some(rows) = report_rows {
        if rows.is_empty() {
            html.push_str("<p>No matching stored observations. Collect evidence explicitly before drawing conclusions.</p>");
        }
        for row in rows {
            html.push_str("<table>");
            if let Some(obj) = row.as_object() {
                for (key, val) in obj {
                    let text = if val.is_null() {
                        "Unknown".into()
                    } else if let Some(s) = val.as_str() {
                        s.into()
                    } else {
                        val.to_string()
                    };
                    let display = text.chars().take(400).collect::<String>();
                    html.push_str(&format!(
                        "<tr><th>{}</th><td>{}{}</td></tr>",
                        escape(key),
                        escape(&display),
                        if text.chars().count() > 400 {
                            " (truncated; full bounded evidence below)"
                        } else {
                            ""
                        }
                    ));
                }
            }
            html.push_str("</table>");
        }
    } else if evidence.is_null() {
        html.push_str(
            "<p>No completed matching audit is available. Health and issue counts are unknown.</p>",
        );
    }
    let raw = serde_json::to_string_pretty(&data).map_err(AppError::internal)?;
    if raw.len() > 200_000 {
        return Err(AppError::Conflict(
            "Stored evidence exceeds the report budget; narrow the project data first".into(),
        ));
    }
    html.push_str(&format!("<details open><summary>Source evidence, timestamps and limits</summary><pre>{}</pre></details></body></html>",escape(&raw)));
    crate::reports::create(
        pool,
        a,
        pid,
        crate::reports::SaveReportInput {
            title,
            summary: Some(format!(
                "Built-in {} report from bounded, source-labelled stored evidence.",
                v.kind
            )),
            html,
            skill: Some(format!("builtin-{}", v.kind)),
            template_id: None,
        },
    )
    .await
}
pub async fn pdf(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    id: Uuid,
    cfg: &Config,
) -> AppResult<Vec<u8>> {
    let report = crate::reports::get(pool, a, pid, id).await?;
    let endpoint = cfg
        .renderer_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Conflict(
                "Enable the renderer profile and configure RENDERER_URL to export PDF".into(),
            )
        })?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(AppError::internal)?;
    let mut response = client
        .post(format!("{}/pdf", endpoint.trim_end_matches('/')))
        .json(&json!({"html":report.html}))
        .send()
        .await
        .map_err(|_| AppError::Unavailable("PDF renderer is unavailable or timed out".into()))?;
    if !response.status().is_success() {
        return Err(AppError::Unavailable(format!(
            "PDF renderer returned HTTP {}; try later",
            response.status().as_u16()
        )));
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("PDF transfer interrupted".into()))?
    {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err(AppError::Unavailable("PDF exceeds 16 MiB".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(AppError::Unavailable(
            "Renderer did not return a PDF".into(),
        ));
    }
    Ok(bytes)
}
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BusinessProfileInput {
    pub revision: i64,
    pub name: String,
    pub address: String,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub hours: Option<String>,
    pub description: Option<String>,
    pub source_label: String,
}
pub async fn save_profile(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    mut v: BusinessProfileInput,
) -> AppResult<db::operations::BusinessProfile> {
    a.require(Role::Admin)?;
    v.name = validate::non_empty("business name", &v.name, 200)?;
    v.address = validate::non_empty("address", &v.address, 1000)?;
    v.source_label = validate::non_empty("source label", &v.source_label, 200)?;
    v.phone = validate::optional("phone", v.phone.as_deref(), 100)?;
    v.hours = validate::optional("hours", v.hours.as_deref(), 2000)?;
    v.description = validate::optional("description", v.description.as_deref(), 4000)?;
    if let Some(raw) = &v.website {
        let u = url::Url::parse(raw)
            .map_err(|_| AppError::Validation("Website must be an absolute HTTP(S) URL".into()))?;
        if !matches!(u.scheme(), "http" | "https")
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || raw.len() > 2048
        {
            return Err(AppError::Validation("Invalid business website".into()));
        }
    }
    if v.revision < 0 {
        return Err(AppError::Validation("Revision cannot be negative".into()));
    }
    db::operations::save_profile(
        pool,
        a.org_id,
        pid,
        v.revision,
        serde_json::to_value(&v).map_err(AppError::internal)?,
    )
    .await
}
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BusinessEntryInput {
    pub id: Option<Uuid>,
    pub revision: i64,
    pub kind: String,
    pub text: String,
    pub response: Option<String>,
    pub rating: Option<i16>,
    pub source_label: String,
    pub source_url: Option<String>,
}
pub async fn save_business_entry(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    mut v: BusinessEntryInput,
) -> AppResult<db::operations::BusinessEntry> {
    a.require(Role::Admin)?;
    if !["review", "question", "update"].contains(&v.kind.as_str()) || v.revision < 0 {
        return Err(AppError::Validation(
            "Use review, question or update and a nonnegative revision".into(),
        ));
    }
    v.text = validate::non_empty("record text", &v.text, 4000)?;
    v.response = validate::optional("response draft", v.response.as_deref(), 4000)?;
    v.source_label = validate::non_empty("source label", &v.source_label, 200)?;
    if v.rating.is_some_and(|n| !(1..=5).contains(&n)) || (v.kind != "review" && v.rating.is_some())
    {
        return Err(AppError::Validation(
            "Ratings are only valid for reviews and must be 1-5".into(),
        ));
    }
    if let Some(raw) = &v.source_url {
        let u =
            url::Url::parse(raw).map_err(|_| AppError::Validation("Invalid source URL".into()))?;
        if !matches!(u.scheme(), "http" | "https")
            || raw.len() > 2048
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err(AppError::Validation(
                "Source must be an HTTP(S) URL without credentials".into(),
            ));
        }
    }
    db::operations::save_business_entry(
        pool,
        a.org_id,
        pid,
        v.id,
        v.revision,
        &v.kind,
        serde_json::to_value(&v).map_err(AppError::internal)?,
    )
    .await
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteAccountInput {
    pub password: String,
    pub confirmation: String,
}
pub async fn request_account_deletion(
    pool: &PgPool,
    a: &Actor,
    v: DeleteAccountInput,
) -> AppResult<db::operations::AccountDeletion> {
    if a.via != ActorVia::Session {
        return Err(AppError::Forbidden(
            "Account deletion requires a password-authenticated browser session".into(),
        ));
    }
    if v.confirmation != "DELETE MY ACCOUNT" {
        return Err(AppError::Validation("Type DELETE MY ACCOUNT to request permanent account and solo-workspace deletion after seven days".into()));
    }
    let user = db::users::find(pool, a.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !user
        .password_hash
        .as_deref()
        .is_some_and(|hash| crate::auth::verify_password(&v.password, hash))
    {
        return Err(AppError::Forbidden("Current password is incorrect".into()));
    }
    db::operations::request_deletion(pool, a.user_id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_strings_are_html_escaped() {
        assert_eq!(
            escape("<img onerror='x'>&\""),
            "&lt;img onerror=&#39;x&#39;&gt;&amp;&quot;"
        );
    }
}
