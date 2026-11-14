//! MCP server (Streamable HTTP). Tools are thin: authenticate → call a `domain` service →
//! wrap the result. Tool names follow OpenSEO's where the feature exists, so agent skills
//! written for it keep working.
//!
//! Every tool returns one JSON text block: `{"ok":true,"data":…}` or
//! `{"ok":false,"error":{"code","message"}}` with a message that says what to do next.
//! The HTTP layer (see the `api` crate) authenticates the request and stores the [`Actor`]
//! in the request extensions; tools never see credentials.

mod params;

use axum::http::request::Parts;
use common::{AppError, AppResult};
use crawler::UrlPolicy;
use domain::Actor;
use jobs::ProviderCtx;
use params::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

const INSTRUCTIONS: &str = "SeoSeo is a self-hosted SEO platform that works without paid API keys. \
Start with `list_projects` (or `create_project`); most tools need a `project_id`. \
Long tasks (site audits, keyword research, rank checks) run in the background: start them, then \
poll the matching get/status tool until `status` is `completed`. \
Metrics that cannot be determined are null, never 0: treat null as unknown. Positions and \
difficulty come from the configured search provider (`provider`/`exact` fields say which and \
whether it is a real engine ranking). There are no credits: 'cost' means provider requests.";

/// Everything a tool needs. Cheap to clone; one instance is built per request.
#[derive(Clone)]
pub struct SeoSeoMcp {
    cfg: Arc<common::Config>,
    pool: PgPool,
    providers: Arc<ProviderCtx>,
    policy: UrlPolicy,
    renderer_available: bool,
}

type ToolResult = Result<CallToolResult, ErrorData>;

/// The authenticated caller, placed in the HTTP request extensions by the API layer.
fn actor(ctx: &RequestContext<RoleServer>) -> Result<Actor, ErrorData> {
    ctx.extensions
        .get::<Parts>()
        .and_then(|p| p.extensions.get::<Actor>())
        .cloned()
        .ok_or_else(|| ErrorData::invalid_request("not authenticated", None))
}

fn code(e: &AppError) -> &'static str {
    match e {
        AppError::BadRequest(_) => "bad_request",
        AppError::Unauthorized => "unauthorized",
        AppError::Forbidden(_) => "forbidden",
        AppError::NotFound(_) => "not_found",
        AppError::Conflict(_) => "conflict",
        AppError::Validation(_) => "invalid_input",
        AppError::Unavailable(_) => "provider_unavailable",
        AppError::Internal(_) => "internal",
    }
}

fn reply<T: Serialize>(result: AppResult<T>) -> ToolResult {
    Ok(match result {
        Ok(data) => CallToolResult::success(vec![ContentBlock::text(
            json!({ "ok": true, "data": data }).to_string(),
        )]),
        Err(e) => {
            let message = match &e {
                AppError::Internal(err) => {
                    tracing::error!(error = ?err, "mcp tool failed");
                    "something went wrong on the server; try again or check the server logs"
                        .to_string()
                }
                other => other.to_string(),
            };
            CallToolResult::error(vec![ContentBlock::text(
                json!({ "ok": false, "error": { "code": code(&e), "message": message } })
                    .to_string(),
            )])
        }
    })
}

fn id(field: &str, value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        AppError::Validation(format!(
            "{field} must be a UUID; get it from the matching list tool"
        ))
    })
}

/// Convert tool parameters into a service input with the same field names.
fn conv<T: DeserializeOwned>(params: &impl Serialize) -> AppResult<T> {
    serde_json::to_value(params)
        .and_then(serde_json::from_value)
        .map_err(|e| AppError::Validation(format!("invalid arguments: {e}")))
}

#[tool_router]
impl SeoSeoMcp {
    #[tool(
        description = "List curated local business categories for explicit OpenStreetMap business discovery."
    )]
    async fn list_local_categories(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        let _actor = actor(&ctx)?;
        reply(Ok(domain::local::categories()))
    }

    #[tool(
        description = "Explicit cached OpenStreetMap business discovery by name and location. Member role. Results are discovery matches, never local rankings. Do not use for autocomplete or grid scans."
    )]
    async fn search_local_businesses(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<LocalBusinessSearch>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::local::search_businesses(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Save a project local tracker with 1-50 keywords and a 3/5/7/9 geometry grid. Member role. This does not run a rank scan."
    )]
    async fn create_local_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CreateLocalTracker>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::local::create_tracker(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "List saved local business trackers in an accessible project. Grid configurations do not imply measured rankings."
    )]
    async fn list_local_trackers(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::local::list_trackers(&self.pool, &actor, id("project_id", &p.project_id)?)
                    .await
            }
            .await,
        )
    }

    #[tool(
        description = "Preview saved local tracker geometry and keywords. All ranks are unknown and measured is false; no provider requests or rank scans occur."
    )]
    async fn get_local_grid_preview(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<TrackerRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::local::grid_preview(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Look up Chrome UX Report real-user field p75 metrics for a project page or origin. Requires operator CRUX_API_KEY; not configured or insufficient coverage stays unknown. Explicit scope url/origin and device PHONE/DESKTOP/TABLET/ALL; no silent origin fallback. Cached 24 hours, uncached lookups paced per project/target. Does not start a crawl or change audit health/Lighthouse scores."
    )]
    async fn get_crux_field_data(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CruxParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::crux::lookup(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Inspect public homepage technology fingerprints from meta generators and asset markup. One explicit robots-respecting bounded lookup, no redirect following or bot-check bypass; results cached 24 hours, uncached calls paced per project/URL. Returns observed hints or unknown, never a verified full stack/version/security assessment. Optional homepage URL defaults to the project HTTPS root."
    )]
    async fn detect_technologies(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<TechnologyParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::technology::inspect(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    self.policy,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Find link-intersect prospects from current-website backlink samples of 1-5 accessible competitor projects. Read-only, no discovery/contact lookup/outreach. Uses latest snapshot per dataset/source, at most ten datasets per project; excludes source domains already observed in this project's selected sample. Absence is not proof of no live link. Returns scope/provenance and paged prospects."
    )]
    async fn find_link_intersect(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<LinkIntersectParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::link_intersect::inspect(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Flag project keywords where multiple distinct URLs of the project registrable domain appear in the same recent cached results page. Read-only, no search requests. Returns positions/provider/exactness and coverage. These are review candidates, not proof of harmful cannibalization or a recommendation to merge pages."
    )]
    async fn get_cannibalization_flags(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CannibalizationParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::cannibalization::flags(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Look up a literal brand term in retained successful model answers for this project's current website and selected market. Read-only, no model calls. Includes matched excerpts, prompt/model/engine/grounding, successful/failed/unchecked coverage and rates over saved successful cells only. Not global prompt demand or consumer chat visibility."
    )]
    async fn lookup_ai_brand(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BrandLookupParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let mut q: domain::ai_index::AiIndexQuery = conv(&p)?;
                q.view = Some("answers".into());
                domain::ai_index::inspect(&self.pool, &a, id("project_id", &p.project_id)?, q).await
            }
            .await,
        )
    }
    #[tool(
        description = "Explore prompts in this project's own retained model-answer index, current website and selected market. Optional topic searches saved prompt text; optional literal brand filters prompts with saved matching answers. Reports expected/successful/failed/unchecked coverage. Read-only, no model calls; not a global prompt-demand database."
    )]
    async fn explore_ai_prompts(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiIndexParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let mut q: domain::ai_index::AiIndexQuery = conv(&p)?;
                q.view = Some("prompts".into());
                domain::ai_index::inspect(&self.pool, &a, id("project_id", &p.project_id)?, q).await
            }
            .await,
        )
    }
    #[tool(
        description = "Compare two completed project audits with the same start URL. Read-only, paginated observed page/issue differences. Missing URLs are not proven removed, failed/unvisited pages do not resolve issues. Returns coverage/configuration warnings and unconfirmed absences."
    )]
    async fn compare_site_audits(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CompareAuditParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::audit_compare::compare(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read this project's optional recurring site-audit schedule, frozen baseline crawl settings, UTC next run, latest audit and errors. Returns null if no schedule has been configured. Does not start a crawl."
    )]
    async fn get_audit_schedule(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::audit_schedule::get(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Explicitly configure or disable recurring project audits. Admin required. Enabling copies a reviewed completed baseline of the current website; expected_revision=0 initially, else GET revision. Schedule manual/daily/weekly/monthly, hour_utc 0-23, weekday 1-7. Enabling authorizes future crawls with those frozen budgets; archives, website changes and 100 stored audits pause it."
    )]
    async fn set_audit_schedule(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SetAuditScheduleParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::audit_schedule::set(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    self.policy,
                    self.renderer_available,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Inspect a completed audit's internal link graph and conservative link suggestions from stored evidence. Read-only, no crawl or provider calls. Page counts cover the full stored audit; edges/suggestions use the bounded selected pages (limit 10-500, default 200). Self-links excluded, nofollow distinct, missing pages unknown. Review title/H1 overlap suggestions before editing."
    )]
    async fn get_internal_link_graph(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<InternalLinkParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::internal_links::graph(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Group project keywords by shared top-ten SERP URLs using cached evidence from the last seven days. Read-only, no provider requests. Every pair must share min_shared URLs (2-10, default 3); providers/exactness are never mixed. Returns groups, ungrouped keywords, missing evidence and coverage. Review before assigning pages."
    )]
    async fn cluster_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<KeywordClusterParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::clustering::clusters(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "List accessible GA4 properties with pagination. Needs admin and a separate Analytics grant; pass next_page_token as page_token."
    )]
    async fn list_google_analytics_properties(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4PropertyPage>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::properties(&self.pool, &a, pid, &self.cfg, p.page_token).await
            }
            .await,
        )
    }
    #[tool(
        description = "Bind a project to a freshly verified GA4 property from list_google_analytics_properties. Needs admin; the Google grant is independent of Search Console."
    )]
    async fn set_google_analytics_property(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4PropertySelection>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::set_property(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4PropertyInput {
                        property_id: p.property_id,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Create or reuse a project IndexNow ownership key. Needs admin; host this key at the named domain root before verification or submission."
    )]
    async fn setup_indexnow(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::indexnow_setup(&self.pool, &a, pid).await
            }
            .await,
        )
    }
    #[tool(
        description = "Verify the exact root IndexNow key file with guarded DNS and no redirects. Needs admin. A failed check sends no URLs to engines."
    )]
    async fn verify_indexnow(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::indexnow_verify(&self.pool, &a, pid, &self.cfg).await
            }
            .await,
        )
    }
    #[tool(
        description = "Submit 1-1000 changed URLs on the project domain or subdomains after checking ownership on each host. Needs admin. Engine acceptance is not indexing; one-minute cooldown."
    )]
    async fn submit_indexnow_urls(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<IndexNowUrls>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::indexnow_submit(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::integrations::IndexNowInput { urls: p.urls },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Generate an optional local or compatible-model draft from bounded crawl evidence. Needs admin and configured LLM. This never saves memory: review output with the user before save_project_website_setup."
    )]
    async fn synthesize_project_website(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::synthesize(&self.pool, &a, pid, &self.cfg).await
            }
            .await,
        )
    }
    #[tool(
        description = "List up to 100 most recent project link observations. These are recorded evidence, not an exhaustive backlink index or independently verified live status."
    )]
    async fn list_link_observations(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::links(&self.pool, &a, pid).await
            }
            .await,
        )
    }
    #[tool(
        description = "Record a user-reviewed external link observation (live or lost), targeting this project website. Needs admin. This does not crawl or verify the source URL."
    )]
    async fn save_link_observation(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<LinkObservationInput>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::integrations::save_link(
                    &self.pool,
                    &a,
                    pid,
                    domain::integrations::LinkInput {
                        source_url: p.source_url,
                        target_url: p.target_url,
                        state: p.state,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 organic overview report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_organic_overview(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::OrganicOverview,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 organic landing pages report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_organic_landing_pages(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::OrganicLandingPages,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 page performance report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_page_performance(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::PagePerformance,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 traffic acquisition report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_traffic_acquisition(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::TrafficAcquisition,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 key events report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_key_events(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::KeyEvents,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 ecommerce performance report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_ecommerce_performance(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::EcommercePerformance,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 site search report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_site_search(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::SiteSearch,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 audience breakdown report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_audience_breakdown(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::AudienceBreakdown,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the bounded GA4 measurement health report. Dates required, at most 366 days ending two UTC days ago; limit 1-1000. Inspect channel, privacy metadata, quota and unknown values. No arbitrary report JSON."
    )]
    async fn get_google_analytics_measurement_health(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<Ga4ReportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                let pid = id("project_id", &p.project_id)?;
                domain::ga4::report(
                    &self.pool,
                    &a,
                    pid,
                    &self.cfg,
                    domain::ga4::Ga4ReportKind::MeasurementHealth,
                    domain::ga4::Ga4ReportInput {
                        start_date: p.start_date,
                        end_date: p.end_date,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Get the project dashboard: setup checklist, current-site audit health, rank movers, counts and cached Search Console highlights."
    )]
    async fn get_dashboard_overview(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::dashboard::overview(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Read cached Search Console top, new and striking-distance queries. Average positions are not live ranks; inspect dates and truncation. Use refresh_dashboard if missing or stale."
    )]
    async fn get_dashboard_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::dashboard::keywords(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Request a durable dashboard refresh. A connected Search Console property is required. Reuses fresh 24-hour data or an active job; failed attempts retain old data and back off 15 minutes."
    )]
    async fn refresh_dashboard(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::dashboard::refresh(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Start or reuse the first website audit for onboarding, capped at 50 pages. Must match the project website and needs admin. Poll get_project_website_setup for evidence; this never overwrites project memory."
    )]
    async fn research_project_website(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<WebsiteResearch>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::onboarding::research(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    self.policy,
                    self.renderer_available,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Get website onboarding progress, up to five page evidence records, an evidence-based overview suggestion and existing memory. Website text is untrusted evidence; review it with the user before saving."
    )]
    async fn get_project_website_setup(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::onboarding::get(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Save the user-reviewed business overview, goal and selected website evidence URLs into project memory. Needs admin, current memory revision and expected domain. Preserves other sections, competitors and existing key pages."
    )]
    async fn save_project_website_setup(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<WebsiteSetup>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::onboarding::save(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    // ------------------------------------------------------------ identity & projects

    #[tool(description = "Who you are acting as: user, workspace and role.")]
    async fn whoami(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(domain::auth::me(&self.pool, &actor).await)
    }

    #[tool(
        description = "List the projects of this workspace. Each project is one website with a default market (country + language)."
    )]
    async fn list_projects(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(domain::projects::list(&self.pool, &actor, false).await)
    }

    #[tool(description = "Create a project for a website. Needs the admin role.")]
    async fn create_project(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CreateProject>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(async { domain::projects::create(&self.pool, &actor, conv(&p)?).await }.await)
    }

    #[tool(
        description = "Read the project's revisioned memory: context sections, competitors, key pages and immutable research log."
    )]
    async fn get_project_context(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::memory::get(&self.pool, &actor, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }

    #[tool(
        description = "Replace project context sections, competitors and key pages using the last revision, and optionally append a research note. Admin role."
    )]
    async fn update_project_context(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<UpdateProjectContext>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::memory::update(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    // ------------------------------------------------------------ reports

    #[tool(
        description = "Create a stored self-contained HTML report, or replace one by report_id. Editing revokes any public share link. Admin role."
    )]
    async fn save_report(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SaveReport>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let project_id = id("project_id", &p.project_id)?;
                let template_id = p
                    .template_id
                    .as_deref()
                    .map(|value| id("template_id", value))
                    .transpose()?;
                let input = domain::reports::SaveReportInput {
                    title: p.title,
                    summary: p.summary,
                    html: p.html,
                    skill: p.skill,
                    template_id,
                };
                match p.report_id {
                    Some(report_id) => {
                        domain::reports::update(
                            &self.pool,
                            &actor,
                            project_id,
                            id("report_id", &report_id)?,
                            input,
                        )
                        .await
                    }
                    None => domain::reports::create(&self.pool, &actor, project_id, input).await,
                }
            }
            .await,
        )
    }

    #[tool(description = "Get one stored report, including its self-contained HTML document.")]
    async fn get_report(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ReportRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::reports::get(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("report_id", &p.report_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "List stored reports newest first. HTML is omitted from list rows.")]
    async fn list_reports(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::reports::list(&self.pool, &actor, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }

    #[tool(description = "Permanently delete a stored report. Admin role.")]
    async fn delete_report(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ReportRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::reports::delete(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("report_id", &p.report_id)?,
                )
                .await?;
                Ok(json!({ "deleted": true }))
            }
            .await,
        )
    }

    #[tool(
        description = "Create a reusable report instruction template (maximum 10 per project). Admin role."
    )]
    async fn save_report_template(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SaveReportTemplate>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let project_id = id("project_id", &p.project_id)?;
                let input = conv(&p)?;
                match p.template_id {
                    Some(template_id) => {
                        domain::reports::update_template(
                            &self.pool,
                            &actor,
                            project_id,
                            id("template_id", &template_id)?,
                            input,
                        )
                        .await
                    }
                    None => {
                        domain::reports::create_template(&self.pool, &actor, project_id, input)
                            .await
                    }
                }
            }
            .await,
        )
    }

    #[tool(description = "List reusable report instruction templates for a project.")]
    async fn list_report_templates(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::reports::list_templates(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "Delete a reusable report instruction template. Admin role.")]
    async fn delete_report_template(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ReportTemplateRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::reports::delete_template(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("template_id", &p.template_id)?,
                )
                .await?;
                Ok(json!({ "deleted": true }))
            }
            .await,
        )
    }

    #[tool(
        description = "Enable public sharing with a new secret token, or revoke the current link. Admin role."
    )]
    async fn set_report_sharing(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SetReportSharing>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let project_id = id("project_id", &p.project_id)?;
                let report_id = id("report_id", &p.report_id)?;
                if p.enabled {
                    domain::reports::share(&self.pool, &actor, project_id, report_id)
                        .await
                        .map(|result| json!(result))
                } else {
                    domain::reports::unshare(&self.pool, &actor, project_id, report_id)
                        .await
                        .map(|report| json!({ "report": report, "token": null }))
                }
            }
            .await,
        )
    }

    // ------------------------------------------------------------ keywords

    #[tool(
        description = "Start keyword research for a seed keyword: ideas from search autocomplete with relative popularity, intent, and (for the most popular ideas) difficulty and SERP features. Runs in the background; call get_keyword_research with the returned id until status is completed."
    )]
    async fn research_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ResearchKeywords>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::start(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "List keyword research runs of a project, newest first.")]
    async fn list_keyword_research(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::list(&self.pool, &actor, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }

    #[tool(
        description = "Status of a keyword research and a page of its ideas. popularity is relative within the research (0-100), not a search volume; est_volume/kd are null when unknown. warnings explain missing data."
    )]
    async fn get_keyword_research(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetKeywordResearch>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let (pid, rid) = (
                    id("project_id", &p.project_id)?,
                    id("research_id", &p.research_id)?,
                );
                let research = domain::keywords::get(&self.pool, &actor, pid, rid).await?;
                let ideas =
                    domain::keywords::ideas(&self.pool, &actor, pid, rid, conv(&p)?).await?;
                Ok(json!({ "research": research, "ideas": ideas }))
            }
            .await,
        )
    }

    #[tool(
        description = "Save keywords to the project's list: ideas of a research (research_id + idea_ids, keeps their metrics) or keywords typed by hand."
    )]
    async fn save_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SaveKeywords>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::save(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "List the project's saved keywords with their metrics and tags.")]
    async fn list_saved_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ListSavedKeywords>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::saved(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "Remove saved keywords by id.")]
    async fn remove_saved_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<IdsInProject>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::delete_saved(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Search results page for a keyword: ranking pages with domain authority (sr), SERP features, related searches, questions, keyword difficulty and intent. Cached for 7 days unless refresh is true. Needs a search results provider; the error says how to set one up."
    )]
    async fn get_serp_results(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetSerp>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::keywords::serp_analysis(
                    &self.pool,
                    &actor,
                    &self.providers,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Domain authority (SeoSeo Rank, 0-100) of a domain from the Common Crawl web graph. sr is null when the domain is outside the imported list."
    )]
    async fn get_domain_authority(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<DomainRef>,
    ) -> ToolResult {
        actor(&ctx)?;
        reply(domain::providers::lookup_authority(&self.pool, &p.domain).await)
    }

    #[tool(
        description = "List operator-approved AI visibility models. Uses one configured compatible gateway, including local Ollama. Returns model identifiers and readiness, never credentials or a configurable endpoint. Plain API answers have no web-search tools."
    )]
    async fn list_ai_visibility_models(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        actor(&ctx)?;
        reply(Ok(domain::ai_visibility::models(&self.cfg)))
    }
    #[tool(
        description = "Discover backlink candidates using the configured free/default SERP provider and verify bounded external source HTML, or recheck explicit source URLs without a SERP provider. Needs admin. Obeys robots, guarded DNS, no redirects, per-host delays and blocks. Queues at most 40 source checks. Unknown/blocked sources retain prior pairs as unconfirmed; never infer loss from a changed search-result sample. Poll list_backlink_discoveries; read the completed snapshot."
    )]
    async fn discover_backlinks(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BacklinkDiscoveryParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::discover(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    &self.providers,
                    domain::backlinks::StartDiscoveryInput {
                        expected_domain: p.expected_domain,
                        search: p.search,
                        source_urls: p.source_urls,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "List current-website backlink discovery jobs and their completed snapshot IDs/errors. Search/crawl coverage is bounded; blocked and inconclusive sources are not evidence of loss."
    )]
    async fn list_backlink_discoveries(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::discoveries(&self.pool, &a, id("project_id", &p.project_id)?)
                    .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read the current website's latest backlink snapshot and history for dashboard use. Inspect snapshot source, per-pair verification, coverage and relative heuristic spam flags. Imported pairs are unverified; bounded crawls are not an exhaustive inbound-link index."
    )]
    async fn get_dashboard_backlinks(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::dashboard(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Import user-reported monthly SEARCH volume, not pageviews, for up to 1000 project keyword/market rows. Needs admin. Labels provenance and enables heuristic CTR traffic estimates only for exact observed rankings; unknown/incomplete coverage stays null. Does not validate the source externally."
    )]
    async fn import_competitive_search_volumes(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CompetitiveVolumeParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::competitive::volumes(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    domain::competitive::CompetitiveVolumeInput {
                        country: p.country,
                        language: p.language,
                        source_label: p.source_label,
                        rows: p
                            .rows
                            .into_iter()
                            .map(|r| domain::competitive::CompetitiveVolumeRow {
                                keyword: r.keyword,
                                monthly_search_volume: r.monthly_search_volume,
                            })
                            .collect(),
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Set an AI tracker UTC schedule: manual, daily, weekly or monthly; hour_utc 0-23 and weekday 1-7. Needs admin. Enabling periodic runs authorizes recurring configured-model requests. Busy ticks skip; full retention pauses the schedule without deleting data. Saving clears a paused schedule error."
    )]
    async fn set_ai_visibility_schedule(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiScheduleParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::schedule(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    &self.cfg,
                    domain::ai_visibility::AiScheduleInput {
                        schedule: p.schedule,
                        hour_utc: p.hour_utc,
                        weekday: p.weekday,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Generate draft AI research questions from a topic using one approved model request. Needs admin. Returns questions for review, never saves tracker/context or measures prompt search volume. A cloud engine may use its configured search tool and operator account."
    )]
    async fn research_ai_visibility_prompts(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiPromptResearchParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::prompt_research(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    domain::ai_visibility::AiPromptResearchInput {
                        topic: p.topic,
                        model: p.model,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Summarize retained AI visibility runs: coverage-aware trend, mention rates, share of tracked brand-answer matches, and domains from structured provider citations. Compare gateway identities and inspect answer grounding; no claim of consumer UI market share."
    )]
    async fn get_ai_visibility_summary(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiTrackerRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::summary(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Create an immutable AI visibility tracker, initially manual. Needs admin, expected_domain must match the project website. Select 1-3 approved model IDs, 1-10 prompts (500 chars each), and 1-10 literal brand terms (80 chars each). No model request on creation; explicitly start a run or configure a schedule. Country/language are requested model context, not geographic verification."
    )]
    async fn create_ai_visibility_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CreateAiTrackerParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::create(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    domain::ai_visibility::CreateAiTrackerInput {
                        expected_domain: p.expected_domain,
                        name: p.name,
                        brands: p.brands,
                        prompts: p.prompts,
                        models: p.models,
                        country: p.country,
                        language: p.language,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "List this project's AI visibility tracker configurations, including previous websites for cleanup. Read details and start runs only for the current project website."
    )]
    async fn list_ai_visibility_trackers(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::list(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Read a current-website AI tracker and its last 20 run records. Run status and errors matter; never treat unchecked or failed answers as brand absence."
    )]
    async fn get_ai_visibility_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiTrackerRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::get(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Explicitly start a manual model API comparison (up to 30 requests) using operator-configured models/credentials. Needs admin. Runs in the durable background queue with frozen prompts/brands/models. Poll get_ai_visibility_run. One active run per tracker, two per project, one-minute cooldown. Native Anthropic requests web grounding and preserves provider citations; Gemini/compatible/local models have no web tools. Provider failures stop collection and retain partial results."
    )]
    async fn run_ai_visibility_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<StartAiRunParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::start(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    &self.cfg,
                    domain::ai_visibility::StartAiRunInput {
                        expected_domain: p.expected_domain,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read stored AI answers, literal brand mentions, unverified model-reported URLs and mention rates. Rates use successful answers only, not failed/unattempted cells; they are not share of voice or endorsements. Inspect model/prompt coverage and run errors. JSON data can be exported by the caller."
    )]
    async fn get_ai_visibility_run(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiRunRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::run(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    id("run_id", &p.run_id)?,
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Delete an AI tracker and all its stored runs/answers. Needs admin and active project; cannot delete while this tracker has an active run."
    )]
    async fn delete_ai_visibility_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiTrackerRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::delete(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    None,
                )
                .await?;
                Ok(json!({"deleted":true}))
            }
            .await,
        )
    }
    #[tool(
        description = "Delete one stored AI run and its answers to free retention quota. Needs admin and active project; cannot delete while this tracker has an active run."
    )]
    async fn delete_ai_visibility_run(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AiRunRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::ai_visibility::delete(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    Some(id("run_id", &p.run_id)?),
                )
                .await?;
                Ok(json!({"deleted":true}))
            }
            .await,
        )
    }

    #[tool(
        description = "List imported backlink snapshot metadata for this project, newest first, including prior website identities for cleanup. These are user-provided samples, not verified crawls. Details are readable only for the current project website."
    )]
    async fn list_backlink_snapshots(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::list(&self.pool, &a, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }
    #[tool(
        description = "Import a complete backlink sample (max 1000 unique external source/target pairs, 2 MiB body cap). Needs admin, expected_domain must match the current project website and complete_sample must be true. Same-dataset imports compare sample membership; first import is a baseline. Does not fetch or independently verify URLs. Empty imports mark prior pairs absent in the sample. Keep a stable dataset key for comparable exports."
    )]
    async fn import_backlink_snapshot(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BacklinkImportParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::import(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    domain::backlinks::ImportBacklinksInput {
                        expected_domain: p.expected_domain,
                        dataset: p.dataset,
                        complete_sample: p.complete_sample,
                        links: p
                            .links
                            .into_iter()
                            .map(|l| domain::backlinks::ImportedBacklink {
                                source_url: l.source_url,
                                target_url: l.target_url,
                                anchor: l.anchor,
                                nofollow: l.nofollow,
                            })
                            .collect(),
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Read imported backlink evidence, paginated by state, plus current-sample referring domains, target pages and anchors. Live includes new. New/lost mean presence/absence in this dataset's consecutive imports, not independently verified link status. Spam score is unknown. Aggregates describe live+new only."
    )]
    async fn get_backlink_snapshot(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BacklinkSnapshotParams>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::get(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("snapshot_id", &p.snapshot_id)?,
                    domain::backlinks::BacklinkQuery {
                        state: p.state,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }
    #[tool(
        description = "Delete one imported backlink snapshot and its evidence. Needs admin and an active project. Other snapshots retain their recorded diff and comparison timestamp; future imports compare against the latest remaining snapshot in their dataset/website."
    )]
    async fn delete_backlink_snapshot(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BacklinkSnapshotRef>,
    ) -> ToolResult {
        let a = actor(&ctx)?;
        reply(
            async {
                domain::backlinks::delete(
                    &self.pool,
                    &a,
                    id("project_id", &p.project_id)?,
                    id("snapshot_id", &p.snapshot_id)?,
                )
                .await?;
                Ok(json!({"deleted":true}))
            }
            .await,
        )
    }

    #[tool(
        description = "Look up domain registration age using the IANA-discovered HTTPS registry RDAP service. Public metadata cached for 24 hours. Unknown dates remain null; registration age does not establish website age or continuous ownership. May make two public requests on a cache miss. Registry redirects and private endpoints are not followed."
    )]
    async fn get_domain_age(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<DomainAgeParams>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::domain_age::lookup(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    domain::domain_age::DomainAgeInput { domain: p.domain },
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Research a domain from this project's saved/researched keyword SERPs: observed keywords, top pages, competing domains and gaps against the project website. Latest 30-day snapshots, up to 500 keywords and positions 1-30; not an exhaustive index. No provider requests. Traffic is unknown; inspect coverage, provider and exact flags before drawing conclusions."
    )]
    async fn get_domain_overview(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<DomainOverviewParams>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::competitive::overview(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    domain::competitive::DomainQuery {
                        domain: p.domain,
                        country: p.country,
                        language: p.language,
                        limit: p.limit,
                        offset: p.offset,
                    },
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Data providers of this workspace and whether each is ready. Use it to explain why data is missing."
    )]
    async fn list_data_providers(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(domain::providers::list(&self.pool, &actor, &self.providers).await)
    }

    // ------------------------------------------------------------ site audit

    #[tool(
        description = "Start a technical SEO crawl of the project's website. Runs in the background; poll get_audit_status. Obeys robots.txt by default."
    )]
    async fn run_site_audit(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<RunSiteAudit>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::audits::start(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    self.policy,
                    self.renderer_available,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "List site audits of a project, newest first.")]
    async fn list_site_audits(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::audits::list(&self.pool, &actor, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }

    #[tool(description = "Status, progress, health score and issue counts of a site audit.")]
    async fn get_audit_status(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AuditRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::audits::get(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Issues found by a site audit. Without issue_type: the summary of issue types with affected page counts. With issue_type: the affected pages and details."
    )]
    async fn get_audit_issues(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetAuditIssues>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let (pid, aid) = (
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                );
                if p.issue_type.as_deref().is_none_or(str::is_empty) {
                    let summary =
                        domain::audits::issue_summary(&self.pool, &actor, pid, aid).await?;
                    Ok(json!({ "summary": summary }))
                } else {
                    let issues =
                        domain::audits::issues(&self.pool, &actor, pid, aid, conv(&p)?).await?;
                    Ok(json!({ "issues": issues }))
                }
            }
            .await,
        )
    }

    #[tool(
        description = "Pages crawled by a site audit, with filters (search, status 2xx|3xx|4xx|5xx|error, issue_type, indexable)."
    )]
    async fn get_audit_pages(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetAuditPages>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::audits::pages(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "Delete a finished site audit and its data. Needs the admin role.")]
    async fn delete_site_audit(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AuditRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::audits::delete(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("audit_id", &p.audit_id)?,
                )
                .await
                .map(|()| json!({ "deleted": true }))
            }
            .await,
        )
    }

    // ------------------------------------------------------------ rank tracking

    #[tool(
        description = "Create a rank tracker: follow where a domain ranks for keywords in one market. schedule: manual (default), daily, weekly or monthly (UTC)."
    )]
    async fn create_rank_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CreateRankTracker>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::create(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "List the rank trackers of a project.")]
    async fn list_rank_trackers(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::list(&self.pool, &actor, id("project_id", &p.project_id)?).await
            }
            .await,
        )
    }

    #[tool(
        description = "A rank tracker with the latest position of every keyword (plus previous and best) and recent runs. position null with checked_at set = not in the first `depth` results; checked_at null = never checked."
    )]
    async fn get_rank_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<TrackerRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::get(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "Add keywords to a rank tracker (up to 500 per tracker).")]
    async fn add_rank_tracking_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<AddRankKeywords>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::add_keywords(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Remove keywords (and their position history) from a rank tracker by keyword id."
    )]
    async fn remove_rank_tracking_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<TrackerKeywordIds>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::remove_keywords(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(description = "Pin or unpin rank tracker keywords; pinned keywords are listed first.")]
    async fn pin_rank_tracking_keywords(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<PinRankKeywords>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::pin_keywords(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Request budget of a rank check: provider requests per run and per month, the provider that would be used and a rough duration. SeoSeo has no credits or fees."
    )]
    async fn estimate_rank_tracker_cost(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<TrackerRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::estimate(
                    &self.pool,
                    &actor,
                    &self.providers,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Check positions now for all keywords of a tracker or a subset. Runs in the background; poll get_rank_tracker until the newest run is completed."
    )]
    async fn run_rank_tracker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<RunRankTracker>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::rank::start_run(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Position history of one tracked keyword (newest first) plus the tracker's per-run trend (ranking keywords, top 3, top 10, average position)."
    )]
    async fn get_rank_history(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetRankHistory>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                let (pid, tid) = (
                    id("project_id", &p.project_id)?,
                    id("tracker_id", &p.tracker_id)?,
                );
                let trend = domain::rank::trend(&self.pool, &actor, pid, tid).await?;
                let history = match &p.keyword_id {
                    Some(k) => Some(
                        domain::rank::history(
                            &self.pool,
                            &actor,
                            pid,
                            tid,
                            domain::rank::HistoryQuery {
                                keyword_id: id("keyword_id", k)?,
                                limit: p.limit,
                            },
                        )
                        .await?,
                    ),
                    None => None,
                };
                Ok(json!({ "trend": trend, "history": history }))
            }
            .await,
        )
    }

    // ------------------------------------------------------------ jobs

    #[tool(
        description = "List verified Google Search Console properties for a connected project. Admin role; connect Google in the dashboard first."
    )]
    async fn list_search_console_sites(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::gsc::sites(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Select a verified Search Console property for this project. Admin role; use an exact URL from list_search_console_sites."
    )]
    async fn set_search_console_site(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GscSite>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::gsc::set_site(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "List sites visible to this project's encrypted Bing Webmaster API key. Verification codes are never returned. Configure or replace the key in the dashboard first."
    )]
    async fn list_bing_webmaster_sites(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProjectRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::bing::sites(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Bind a project to an exact verified site from list_bing_webmaster_sites. Admin role; revalidates the site against Bing before saving."
    )]
    async fn set_bing_webmaster_site(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BingSite>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::bing::set_site(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    domain::bing::BingSiteInput {
                        site_url: p.site_url,
                    },
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Read project-scoped Bing Webmaster data. kind is queries, pages, crawl, or inbound_links. Bing top query/page data is weekly, crawl data daily, and inbound links are a paged provider sample; missing rows are unknown, not zero."
    )]
    async fn get_bing_webmaster_report(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BingReport>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        let kind = match p.kind.as_str() {
            "queries" => domain::bing::BingReportKind::Queries,
            "pages" => domain::bing::BingReportKind::Pages,
            "crawl" => domain::bing::BingReportKind::Crawl,
            "inbound_links" => domain::bing::BingReportKind::InboundLinks,
            _ => {
                return reply::<serde_json::Value>(Err(AppError::Validation(
                    "kind must be queries, pages, crawl, or inbound_links".into(),
                )));
            }
        };
        reply(
            async {
                domain::bing::report(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    kind,
                    domain::bing::BingReportInput { page: p.page },
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Read measured Google clicks, impressions, CTR and position. Dates within 16 months, excluding the recent three days; AND filters, up to 1000 rows per page. Empty rows mean no reported data."
    )]
    async fn get_search_console_performance(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GscPerformance>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::gsc::performance(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Inspect 1–10 URLs using the connected Search Console property. Reports the indexed version, not a live test. Individual invalid URLs have partial errors; revoked access requires reconnecting."
    )]
    async fn inspect_urls(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GscInspect>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                domain::gsc::inspect(
                    &self.pool,
                    &actor,
                    id("project_id", &p.project_id)?,
                    &self.cfg,
                    conv(&p)?,
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Recent background jobs of this workspace with status, progress and errors."
    )]
    async fn list_jobs(&self, ctx: RequestContext<RoleServer>) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(db::jobs::list_for_org(&self.pool, actor.org_id, 50).await)
    }

    #[tool(
        description = "One background job: status (queued, running, completed, failed), progress and last error."
    )]
    async fn get_job(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<JobRef>,
    ) -> ToolResult {
        let actor = actor(&ctx)?;
        reply(
            async {
                db::jobs::get_for_org(&self.pool, actor.org_id, id("job_id", &p.job_id)?).await
            }
            .await,
        )
    }
}

#[tool_handler]
impl ServerHandler for SeoSeoMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("seoseo", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Settings for the MCP endpoint.
pub struct McpOptions {
    pub cfg: Arc<common::Config>,
    pub pool: PgPool,
    pub providers: Arc<ProviderCtx>,
    pub policy: UrlPolicy,
    pub renderer_available: bool,
    /// Hostnames accepted in the `Host` header (DNS-rebinding guard). Empty = any.
    pub allowed_hosts: Vec<String>,
}

/// The MCP endpoint as a tower service; mount it with `Router::nest_service("/mcp", …)`
/// behind a layer that authenticates the request and inserts the [`Actor`].
///
/// Stateless on purpose: no in-memory sessions, so any API replica can serve any request.
pub fn service(opts: McpOptions) -> StreamableHttpService<SeoSeoMcp, NeverSessionManager> {
    let server = SeoSeoMcp {
        cfg: opts.cfg,
        pool: opts.pool,
        providers: opts.providers,
        policy: opts.policy,
        renderer_available: opts.renderer_available,
    };
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(opts.allowed_hosts);
    StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    )
}

/// Names of all tools (for docs and tests).
pub fn tool_names() -> Vec<String> {
    SeoSeoMcp::tool_router()
        .list_all()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tools_are_registered_with_object_schemas() {
        let tools = super::SeoSeoMcp::tool_router().list_all();
        assert!(tools.len() >= 66, "only {} tools", tools.len());
        for name in [
            "get_dashboard_overview",
            "get_dashboard_keywords",
            "refresh_dashboard",
            "research_project_website",
            "get_project_website_setup",
            "save_project_website_setup",
            "whoami",
            "research_keywords",
            "cluster_keywords",
            "list_local_categories",
            "search_local_businesses",
            "create_local_tracker",
            "list_local_trackers",
            "get_local_grid_preview",
            "get_internal_link_graph",
            "compare_site_audits",
            "get_audit_schedule",
            "set_audit_schedule",
            "get_cannibalization_flags",
            "lookup_ai_brand",
            "explore_ai_prompts",
            "detect_technologies",
            "get_crux_field_data",
            "find_link_intersect",
            "run_site_audit",
            "run_rank_tracker",
        ] {
            assert!(tools.iter().any(|t| t.name == name), "missing {name}");
        }
        for t in &tools {
            assert!(
                t.description.as_deref().is_some_and(|d| d.len() > 20),
                "{} needs a description",
                t.name
            );
            assert_eq!(
                t.input_schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "{} input schema must be an object",
                t.name
            );
        }
    }
}
