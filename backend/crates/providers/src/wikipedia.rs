//! Wikipedia: resolve keywords to articles and read monthly pageviews (official Wikimedia APIs).
//! Pageviews of a matching article are a public, measured demand signal for entity keywords.

use crate::ProviderError;
use crate::http::{bad_json, check, client, net_err, pace, text, url};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use utoipa::ToSchema;

pub const ID: &str = "wikipedia_pageviews";
const GAP: Duration = Duration::from_millis(150);
/// The MediaWiki API accepts up to 50 titles per request.
pub const TITLES_PER_REQUEST: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MonthViews {
    /// `YYYY-MM`
    pub month: String,
    pub views: i64,
}

fn wiki_lang(lang: &str) -> Option<&str> {
    let l = lang.split('-').next().unwrap_or_default();
    (2..=3)
        .contains(&l.len())
        .then_some(l)
        .filter(|l| l.chars().all(|c| c.is_ascii_lowercase()))
}

/// Map each input keyword to the article it resolves to (following normalization and
/// redirects). Missing pages and disambiguation pages are left out.
pub fn parse_titles(
    body: &str,
    inputs: &[String],
) -> Result<HashMap<String, String>, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let q = v
        .get("query")
        .ok_or_else(|| ProviderError::BadResponse("missing query".into()))?;
    let hop = |key: &str| -> HashMap<String, String> {
        q.get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        Some((
                            x.get("from")?.as_str()?.to_string(),
                            x.get("to")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let normalized = hop("normalized");
    let redirects = hop("redirects");
    let existing: std::collections::HashSet<&str> = q
        .get("pages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|p| {
                    p.get("missing").is_none()
                        && p.get("invalid").is_none()
                        && p.pointer("/pageprops/disambiguation").is_none()
                })
                .filter_map(|p| p.get("title").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let mut out = HashMap::new();
    for input in inputs {
        let t = normalized.get(input).unwrap_or(input);
        let t = redirects.get(t).unwrap_or(t);
        if existing.contains(t.as_str()) {
            out.insert(input.clone(), t.clone());
        }
    }
    Ok(out)
}

pub fn parse_views(body: &str) -> Result<Vec<MonthViews>, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("missing items".into()))?;
    Ok(items
        .iter()
        .filter_map(|i| {
            let ts = i.get("timestamp")?.as_str()?;
            Some(MonthViews {
                month: format!("{}-{}", ts.get(0..4)?, ts.get(4..6)?),
                views: i.get("views")?.as_i64()?,
            })
        })
        .collect())
}

/// `(start, end)` in `YYYYMMDD00` covering the last 12 complete months before `today`.
pub fn last_12_months(today: NaiveDate) -> (String, String) {
    let first_this = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let end = first_this.pred_opt().unwrap_or(first_this);
    let start =
        NaiveDate::from_ymd_opt(first_this.year() - 1, first_this.month(), 1).unwrap_or(first_this);
    (
        start.format("%Y%m%d00").to_string(),
        end.format("%Y%m%d00").to_string(),
    )
}

/// Resolve up to [`TITLES_PER_REQUEST`] keywords to article titles.
pub async fn resolve_titles(
    lang: &str,
    keywords: &[String],
) -> Result<HashMap<String, String>, ProviderError> {
    let Some(l) = wiki_lang(lang) else {
        return Ok(HashMap::new());
    };
    let clean: Vec<String> = keywords
        .iter()
        .filter(|k| !k.contains(['|', '#', '<', '>', '[', ']', '{', '}']) && k.len() <= 200)
        .take(TITLES_PER_REQUEST)
        .cloned()
        .collect();
    if clean.is_empty() {
        return Ok(HashMap::new());
    }
    let u = url(
        &format!("https://{l}.wikipedia.org/w/api.php"),
        &[
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("redirects", "1"),
            ("prop", "pageprops"),
            ("ppprop", "disambiguation"),
            ("titles", &clean.join("|")),
        ],
    )?;
    pace(ID, GAP).await;
    let res = check(client().get(u).send().await.map_err(net_err)?)?;
    parse_titles(&text(res, 1024 * 1024).await?, &clean)
}

/// Monthly human pageviews of an article for the last 12 complete months.
pub async fn monthly_views(lang: &str, title: &str) -> Result<Vec<MonthViews>, ProviderError> {
    let Some(l) = wiki_lang(lang) else {
        return Ok(Vec::new());
    };
    let (start, end) = last_12_months(Utc::now().date_naive());
    let mut u = url(
        "https://wikimedia.org/api/rest_v1/metrics/pageviews/per-article",
        &[],
    )?;
    u.path_segments_mut()
        .map_err(|_| ProviderError::NotConfigured("invalid provider URL".into()))?
        .extend([
            &format!("{l}.wikipedia"),
            "all-access",
            "user",
            &title.replace(' ', "_"),
            "monthly",
            &start,
            &end,
        ]);
    u.set_query(None);
    pace(ID, GAP).await;
    let res = client().get(u).send().await.map_err(net_err)?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    parse_views(&text(check(res)?, 256 * 1024).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_titles_through_normalization_and_redirects() {
        let body = r#"{"batchcomplete":true,"query":{
            "normalized":[{"fromencoded":false,"from":"standing desk","to":"Standing desk"},{"from":"nyc","to":"Nyc"}],
            "redirects":[{"from":"Nyc","to":"New York City"}],
            "pages":[{"pageid":1,"ns":0,"title":"Standing desk"},
                     {"pageid":2,"ns":0,"title":"New York City"},
                     {"ns":0,"title":"Zzqq","missing":true},
                     {"pageid":3,"ns":0,"title":"Mercury","pageprops":{"disambiguation":""}}]}}"#;
        let inputs: Vec<String> = ["standing desk", "nyc", "Zzqq", "Mercury"]
            .map(String::from)
            .to_vec();
        let m = parse_titles(body, &inputs).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m["standing desk"], "Standing desk");
        assert_eq!(m["nyc"], "New York City");
    }

    #[test]
    fn parses_views_and_range() {
        let body = r#"{"items":[{"timestamp":"2026090100","views":1095},{"timestamp":"2026100100","views":1500}]}"#;
        let v = parse_views(body).unwrap();
        assert_eq!(v[1].month, "2026-10");
        assert_eq!(v[1].views, 1500);
        let (s, e) = last_12_months(NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
        assert_eq!((s.as_str(), e.as_str()), ("2025110100", "2026103100"));
        let (s, e) = last_12_months(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap());
        assert_eq!((s.as_str(), e.as_str()), ("2025010100", "2025123100"));
    }
}
