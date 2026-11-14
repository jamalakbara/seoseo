//! Frozen, explicitly budgeted local scans. Discovery coordinates never imply rankings.
use crate::{Actor, Role};
use common::{AppError, AppResult, Config};
use db::local_scans::{self as repo, LocalScanRun, NewScan};
use providers::serpapi::LocalResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

const MAX_CELLS: usize = 250;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanConfig {
    pub tracker: db::local::LocalTracker,
    pub keywords: Vec<String>,
    /// Exact provider place_id or data_id, explicitly reviewed; never fuzzy business-name matching.
    pub target_business_id: String,
    pub country: String,
    pub depth: i16,
    pub created_by: Uuid,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub source_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LocalScanCell {
    pub keyword: String,
    pub row: i16,
    pub column: i16,
    pub measured: bool,
    pub results: Vec<LocalResult>,
    pub error: Option<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartLocalScanInput {
    pub target_business_id: String,
    pub keywords: Vec<String>,
    /// Must equal selected keywords × grid points. Live scans may incur provider charges.
    pub expected_requests: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportLocalScanInput {
    pub target_business_id: String,
    pub keywords: Vec<String>,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub source_note: String,
    /// 1–20. Non-returned targets are penalized as depth+1 only in ATRP.
    pub depth: i16,
    /// Omitted cells remain unchecked; measured false cells must have no results.
    pub cells: Vec<LocalScanCell>,
}

fn selected_keywords(raw: &[String], saved: &[String], size: i16) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for k in raw {
        let k = scoring::normalize_keyword(k);
        if !saved.contains(&k) {
            return Err(AppError::Validation(
                "select only saved tracker keywords".into(),
            ));
        }
        if !out.contains(&k) {
            out.push(k);
        }
    }
    if out.is_empty() || out.len() * (size * size) as usize > MAX_CELLS {
        return Err(AppError::Validation(
            "select keywords for 1–250 grid cells per scan".into(),
        ));
    }
    Ok(out)
}

pub(crate) async fn provider_key(pool: &PgPool, a: &Actor, cfg: &Config) -> AppResult<String> {
    let pctx = jobs::ProviderCtx::from_config(cfg).map_err(AppError::internal)?;
    pctx.states(pool,a.org_id).await?.into_iter()
        .find(|s| s.id == "serpapi_observations" && s.enabled == Some(true))
        .and_then(|s| s.secret)
        .ok_or_else(|| AppError::Conflict("enable SerpApi and add a key in Settings → Data providers, or import observations without a key".into()))
}

pub async fn start(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    cfg: &Config,
    input: StartLocalScanInput,
) -> AppResult<LocalScanRun> {
    a.require(Role::Member)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    let tracker = db::local::get(pool, pid, tid).await?;
    let keywords = selected_keywords(
        &input.keywords,
        &db::local::keywords(pool, tid).await?,
        tracker.grid_size,
    )?;
    if input.expected_requests != keywords.len() * (tracker.grid_size * tracker.grid_size) as usize
    {
        return Err(AppError::Conflict(
            "request budget changed; review the selected keywords and grid".into(),
        ));
    }
    let target = crate::validate::non_empty("target_business_id", &input.target_business_id, 160)?;
    provider_key(pool, a, cfg).await?;
    let config = ScanConfig {
        tracker,
        keywords,
        target_business_id: target,
        country: p.country_code,
        depth: 20,
        created_by: a.user_id,
        observed_at: chrono::Utc::now(),
        source_note: "SerpApi Google Maps, first returned page, 14z".into(),
    };
    repo::create(
        pool,
        NewScan {
            org_id: a.org_id,
            project_id: pid,
            tracker_id: tid,
            provider: "serpapi_google_maps".into(),
            status: "queued".into(),
            config: json!(config),
            cells: None,
        },
    )
    .await
}

pub async fn import(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    input: ImportLocalScanInput,
) -> AppResult<LocalScanRun> {
    a.require(Role::Member)?;
    let p = db::projects::get(pool, a.org_id, pid).await?;
    let tracker = db::local::get(pool, pid, tid).await?;
    let keywords = selected_keywords(
        &input.keywords,
        &db::local::keywords(pool, tid).await?,
        tracker.grid_size,
    )?;
    let target = crate::validate::non_empty("target_business_id", &input.target_business_id, 160)?;
    let source_note = crate::validate::non_empty("source_note", &input.source_note, 300)?;
    if !(1..=20).contains(&input.depth)
        || input.cells.is_empty()
        || input.cells.len() > MAX_CELLS
        || input.observed_at > chrono::Utc::now() + chrono::Duration::minutes(5)
    {
        return Err(AppError::Validation(
            "use depth 1–20, 1–250 observations and a non-future date".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for c in &input.cells {
        if !keywords.contains(&c.keyword)
            || c.row < 0
            || c.column < 0
            || c.row >= tracker.grid_size
            || c.column >= tracker.grid_size
            || !seen.insert((&c.keyword, c.row, c.column))
            || c.results.len() > input.depth as usize
            || (!c.measured && !c.results.is_empty())
            || (c.measured && c.error.is_some())
            || c.checked_at > chrono::Utc::now() + chrono::Duration::minutes(5)
            || c.error.as_ref().is_some_and(|e| e.len() > 300)
        {
            return Err(AppError::Validation(
                "invalid, duplicate or out-of-grid cell".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        let mut ranks = std::collections::HashSet::new();
        for r in &c.results {
            if r.position < 1
                || r.position > input.depth
                || r.business_id.is_empty()
                || r.business_id.len() > 160
                || r.name.is_empty()
                || r.name.len() > 500
                || !ids.insert(&r.business_id)
                || !ranks.insert(r.position)
                || r.address.as_ref().is_some_and(|s| s.len() > 500)
                || r.website
                    .as_ref()
                    .is_some_and(|s| s.len() > 2048 || providers::serpapi::public_link(s).is_none())
            {
                return Err(AppError::Validation(
                    "invalid or duplicate competitor result".into(),
                ));
            }
        }
    }
    let config = ScanConfig {
        tracker,
        keywords,
        target_business_id: target,
        country: p.country_code,
        depth: input.depth,
        created_by: a.user_id,
        observed_at: input.observed_at,
        source_note,
    };
    let measured = input.cells.iter().filter(|c| c.measured).count();
    let expected =
        config.keywords.len() * (config.tracker.grid_size * config.tracker.grid_size) as usize;
    let status = if measured == expected {
        "completed"
    } else if measured > 0 {
        "partial"
    } else {
        "failed"
    };
    repo::create(
        pool,
        NewScan {
            org_id: a.org_id,
            project_id: pid,
            tracker_id: tid,
            provider: "imported_observations".into(),
            status: status.into(),
            config: json!(config),
            cells: Some(input.cells.iter().map(|c| json!(c)).collect()),
        },
    )
    .await
}

pub async fn list(pool: &PgPool, a: &Actor, pid: Uuid, tid: Uuid) -> AppResult<Vec<LocalScanRun>> {
    db::projects::get(pool, a.org_id, pid).await?;
    db::local::get(pool, pid, tid).await?;
    repo::list(pool, tid).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalMetrics {
    pub keyword: String,
    pub expected: usize,
    pub measured: usize,
    pub found: usize,
    pub arp: Option<f64>,
    pub atrp: Option<f64>,
    pub solv: Option<f64>,
}
fn metrics(config: &ScanConfig, cells: &[LocalScanCell]) -> Vec<LocalMetrics> {
    config
        .keywords
        .iter()
        .map(|k| {
            let measured: Vec<_> = cells
                .iter()
                .filter(|c| c.keyword == *k && c.measured)
                .collect();
            let ranks: Vec<_> = measured
                .iter()
                .filter_map(|c| {
                    c.results
                        .iter()
                        .find(|r| r.business_id == config.target_business_id)
                        .map(|r| r.position)
                })
                .collect();
            let n = measured.len();
            let sum: i32 = ranks.iter().map(|r| i32::from(*r)).sum();
            LocalMetrics {
                keyword: k.clone(),
                expected: (config.tracker.grid_size * config.tracker.grid_size) as usize,
                measured: n,
                found: ranks.len(),
                arp: (!ranks.is_empty()).then(|| f64::from(sum) / ranks.len() as f64),
                atrp: (n > 0).then(|| {
                    (f64::from(sum) + (n - ranks.len()) as f64 * f64::from(config.depth + 1))
                        / n as f64
                }),
                solv: (n > 0)
                    .then(|| ranks.iter().filter(|r| **r <= 3).count() as f64 / n as f64 * 100.0),
            }
        })
        .collect()
}
#[derive(Debug, Serialize, ToSchema)]
pub struct LocalScanView {
    pub run: LocalScanRun,
    pub config: ScanConfig,
    pub cells: Vec<LocalScanCell>,
    pub metrics: Vec<LocalMetrics>,
    pub points: Vec<crate::local::GridPoint>,
    pub note: String,
}
pub async fn get(
    pool: &PgPool,
    a: &Actor,
    pid: Uuid,
    tid: Uuid,
    rid: Uuid,
) -> AppResult<LocalScanView> {
    db::projects::get(pool, a.org_id, pid).await?;
    db::local::get(pool, pid, tid).await?;
    let run = repo::get(pool, tid, rid).await?;
    let config: ScanConfig =
        serde_json::from_value(run.config.clone()).map_err(AppError::internal)?;
    let cells: Vec<LocalScanCell> = repo::cells(pool, rid)
        .await?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(AppError::internal)?;
    Ok(LocalScanView { metrics:metrics(&config,&cells),points:crate::local::points(&config.tracker),run,config,cells,
        note:"ARP averages found ranks; ATRP includes non-returned targets as depth+1; SoLV is the percentage of measured points in the top 3. Failed and unchecked points are excluded; compare coverage and identical source/grid/keyword/depth before comparing runs. Results are provider samples, not exhaustive Maps rankings.".into() })
}
pub async fn delete(pool: &PgPool, a: &Actor, pid: Uuid, tid: Uuid, rid: Uuid) -> AppResult<()> {
    a.require(Role::Member)?;
    db::projects::get(pool, a.org_id, pid).await?;
    db::local::get(pool, pid, tid).await?;
    repo::delete(pool, tid, rid).await
}

pub fn register(reg: jobs::Registry, cfg: Config) -> jobs::Registry {
    reg.register("local_grid_scan", move |ctx: jobs::JobCtx| {
        let cfg = cfg.clone();
        async move {
            let id: Uuid = serde_json::from_value(ctx.job.payload["run_id"].clone())?;
            let run = repo::get_any(&ctx.pool, id).await?;
            if !matches!(run.status.as_str(), "queued" | "running") {
                return Ok(());
            }
            let result = collect(&ctx, &cfg, &run).await;
            if let Err(e) = result {
                let message = match e {
                    AppError::Internal(_) => "local scan failed; check server logs".into(),
                    other => other.to_string(),
                };
                repo::finish(&ctx.pool, id, "failed", Some(&message)).await?;
            }
            Ok(())
        }
    })
}
async fn collect(ctx: &jobs::JobCtx, cfg: &Config, run: &LocalScanRun) -> AppResult<()> {
    let config: ScanConfig =
        serde_json::from_value(run.config.clone()).map_err(AppError::internal)?;
    let org = ctx.job.organization_id.ok_or(AppError::Unauthorized)?;
    let pid = ctx.job.project_id.ok_or(AppError::Unauthorized)?;
    let role = db::orgs::role(&ctx.pool, org, config.created_by)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let a = Actor {
        org_id: org,
        user_id: config.created_by,
        role: Role::parse(&role).ok_or(AppError::Unauthorized)?,
        via: crate::ActorVia::ApiKey,
    };
    db::projects::get(&ctx.pool, org, pid).await?;
    db::local::get(&ctx.pool, pid, run.tracker_id).await?;
    let key = provider_key(&ctx.pool, &a, cfg).await?;
    repo::finish(&ctx.pool, run.id, "running", None).await?;
    let done: Vec<LocalScanCell> = repo::cells(&ctx.pool, run.id)
        .await?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(AppError::internal)?;
    let geometry = crate::local::points(&config.tracker);
    for keyword in &config.keywords {
        for p in &geometry {
            if done
                .iter()
                .any(|c| c.keyword == *keyword && c.row == p.row && c.column == p.column)
            {
                continue;
            }
            // Durable global pacing across replicas; no burst loops while another request owns the slot.
            while !db::providers::claim_lookup_seconds(&ctx.pool, "serpapi_global_v1", "global", 1)
                .await?
            {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            // Persist intent before spending a request. A crash leaves this point unknown;
            // recovery skips it rather than potentially charging the same cell twice.
            let pending = LocalScanCell {
                keyword: keyword.clone(),
                row: p.row,
                column: p.column,
                measured: false,
                results: vec![],
                error: Some("collection interrupted or still in progress".into()),
                checked_at: chrono::Utc::now(),
            };
            if !repo::claim_cell(&ctx.pool, run.id, &json!(pending)).await? {
                continue;
            }
            let result = providers::serpapi::request(
                &cfg.serpapi_url,
                &key,
                &[
                    ("engine", "google_maps".into()),
                    ("type", "search".into()),
                    ("q", keyword.clone()),
                    ("ll", format!("@{},{},14z", p.latitude, p.longitude)),
                    ("hl", config.tracker.language.clone()),
                    ("gl", config.country.to_lowercase()),
                ],
            )
            .await
            .and_then(|v| providers::serpapi::maps(&v));
            let stop = result.is_err();
            let cell = match result {
                Ok(results) => LocalScanCell {
                    keyword: keyword.clone(),
                    row: p.row,
                    column: p.column,
                    measured: true,
                    results,
                    error: None,
                    checked_at: chrono::Utc::now(),
                },
                Err(_) => LocalScanCell {
                    keyword: keyword.clone(),
                    row: p.row,
                    column: p.column,
                    measured: false,
                    results: vec![],
                    error: Some(
                        "provider could not measure this point; remaining scan stopped".into(),
                    ),
                    checked_at: chrono::Utc::now(),
                },
            };
            repo::put_cell(&ctx.pool, run.id, &json!(cell)).await?;
            ctx.progress(json!({"keyword":keyword,"row":p.row,"column":p.column}))
                .await
                .map_err(AppError::internal)?;
            if stop {
                let any = repo::cells(&ctx.pool, run.id)
                    .await?
                    .iter()
                    .any(|c| c["measured"] == true);
                return repo::finish(
                    &ctx.pool,
                    run.id,
                    if any { "partial" } else { "failed" },
                    Some("provider failure; scan stopped without retries around a block"),
                )
                .await;
            }
        }
    }
    let cells = repo::cells(&ctx.pool, run.id).await?;
    let measured = cells.iter().filter(|c| c["measured"] == true).count();
    let expected = config.keywords.len() * geometry.len();
    repo::finish(
        &ctx.pool,
        run.id,
        if measured == expected {
            "completed"
        } else if measured > 0 {
            "partial"
        } else {
            "failed"
        },
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ScanConfig {
        serde_json::from_value(json!({
            "tracker":{"id":Uuid::nil(),"project_id":Uuid::nil(),"name":"Cafe","business_source":"manual",
              "business_id":"osm","business_name":"Cafe","address":"City","latitude":0.0,"longitude":0.0,
              "category":null,"grid_size":3,"spacing_km":1.0,"language":"en","created_at":chrono::Utc::now(),"keyword_count":1},
            "keywords":["coffee"],"target_business_id":"target","country":"US","depth":20,"created_by":Uuid::nil(),
            "observed_at":chrono::Utc::now(),"source_note":"test"})).unwrap()
    }
    #[test]
    fn metrics_preserve_missing_and_measured_absence() {
        let cfg = config();
        let empty = metrics(&cfg, &[]);
        assert_eq!(empty[0].measured, 0);
        assert!(empty[0].arp.is_none() && empty[0].atrp.is_none() && empty[0].solv.is_none());
        let hit = LocalScanCell {
            keyword: "coffee".into(),
            row: 0,
            column: 0,
            measured: true,
            results: vec![LocalResult {
                position: 2,
                business_id: "target".into(),
                name: "Cafe".into(),
                address: None,
                website: None,
            }],
            error: None,
            checked_at: chrono::Utc::now(),
        };
        let missed = LocalScanCell {
            row: 1,
            results: vec![],
            ..hit.clone()
        };
        let failed = LocalScanCell {
            row: 2,
            measured: false,
            results: vec![],
            error: Some("failed".into()),
            ..hit.clone()
        };
        let m = metrics(&cfg, &[hit, missed, failed]);
        assert_eq!((m[0].expected, m[0].measured, m[0].found), (9, 2, 1));
        assert_eq!(m[0].arp, Some(2.0));
        assert_eq!(m[0].atrp, Some(11.5));
        assert_eq!(m[0].solv, Some(50.0));
    }
    #[test]
    fn scan_budget_uses_unique_saved_keywords() {
        let saved = vec!["coffee".into(), "tea".into(), "cake".into(), "water".into()];
        assert!(selected_keywords(&["foreign".into()], &saved, 3).is_err());
        assert_eq!(
            selected_keywords(&["Coffee".into(), "coffee".into()], &saved, 9)
                .unwrap()
                .len(),
            1
        );
        assert!(selected_keywords(&saved, &saved, 9).is_err());
    }
}
