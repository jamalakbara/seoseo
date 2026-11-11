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
        assert!(tools.len() >= 29, "only {} tools", tools.len());
        for name in [
            "whoami",
            "research_keywords",
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
