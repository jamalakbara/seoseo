//! Tool input types. Field names match the service inputs they are converted into.
//! Ids are strings in the schema (UUIDs) so every MCP client can produce them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DomainOverviewParams {
    pub project_id: String,
    /// Defaults to the project website. Subdomains roll up to the registrable domain.
    pub domain: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebsiteResearch {
    pub project_id: String,
    pub start_url: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebsiteSetup {
    pub project_id: String,
    pub revision: i64,
    pub expected_domain: String,
    pub business_overview: String,
    pub current_goal: String,
    #[serde(default)]
    pub key_pages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GscSite {
    pub project_id: String,
    /// Exact property URL from list_search_console_sites.
    pub site_url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GscFilter {
    pub dimension: String,
    pub operator: String,
    pub expression: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GscPerformance {
    pub project_id: String,
    /// YYYY-MM-DD, within the last 16 months.
    pub start_date: String,
    /// YYYY-MM-DD, at least three days ago.
    pub end_date: String,
    /// query, page, country, device, date, searchAppearance. Empty for totals.
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default)]
    pub filters: Vec<GscFilter>,
    pub search_type: Option<String>,
    /// 1–1000; defaults to 100.
    pub row_limit: Option<u32>,
    pub start_row: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GscInspect {
    pub project_id: String,
    /// 1–10 URLs under the connected property. Reports Google's indexed version, not a live test.
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectRef {
    /// Project id from list_projects.
    pub project_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateProject {
    pub name: String,
    /// Website domain, e.g. example.com.
    pub domain: Option<String>,
    pub brand_name: Option<String>,
    /// ISO 3166-1 alpha-2 country of the default market (default US).
    pub country_code: Option<String>,
    /// ISO 639-1 language of the default market (default en).
    pub language_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MemorySection {
    pub key: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MemoryCompetitor {
    pub domain: String,
    pub name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MemoryKeyPage {
    pub url: String,
    /// hub, spoke, money, or other.
    pub role: String,
    pub topic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateProjectContext {
    pub project_id: String,
    /// Revision returned by get_project_context. Use 0 before the first save.
    pub revision: i64,
    #[serde(default)]
    pub sections: Vec<MemorySection>,
    #[serde(default)]
    pub competitors: Vec<MemoryCompetitor>,
    #[serde(default)]
    pub key_pages: Vec<MemoryKeyPage>,
    /// Optional immutable research note to append.
    pub research_note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveReport {
    pub project_id: String,
    /// Omit to create; provide an existing report id to replace it.
    pub report_id: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    /// Complete self-contained HTML document, at most 500 KiB.
    pub html: String,
    pub skill: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReportRef {
    pub project_id: String,
    pub report_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveReportTemplate {
    pub project_id: String,
    /// Omit to create; provide an existing template id to replace it.
    pub template_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReportTemplateRef {
    pub project_id: String,
    pub template_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetReportSharing {
    pub project_id: String,
    pub report_id: String,
    /// Enable with a new secret token, or revoke the current token.
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchKeywords {
    pub project_id: String,
    /// Seed keyword, e.g. "standing desk".
    pub seed: String,
    /// ISO country; defaults to the project's market.
    pub country: Option<String>,
    /// ISO language; defaults to the project's market.
    pub language: Option<String>,
    /// Ideas to keep (10-500, default 150).
    pub max_ideas: Option<u32>,
    /// Most popular ideas that also get a SERP analysis (0-30, default 10).
    pub serp_top: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetKeywordResearch {
    pub project_id: String,
    /// Id returned by research_keywords.
    pub research_id: String,
    /// Substring filter on the keyword.
    pub search: Option<String>,
    /// informational | commercial | transactional | navigational
    pub intent: Option<String>,
    /// Only question-style ideas.
    pub questions: Option<bool>,
    pub min_popularity: Option<i32>,
    /// Only ideas with a known difficulty at or below this.
    pub max_kd: Option<i32>,
    /// popularity (default) | keyword | est_volume | kd | word_count
    pub sort: Option<String>,
    pub desc: Option<bool>,
    /// Page size (default 50, max 1000).
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveKeywords {
    pub project_id: String,
    /// Research the ideas belong to (required with idea_ids).
    pub research_id: Option<String>,
    /// Idea ids from get_keyword_research.
    pub idea_ids: Option<Vec<String>>,
    /// Keywords typed by hand (saved without metrics).
    pub keywords: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSavedKeywords {
    pub project_id: String,
    pub search: Option<String>,
    pub tag: Option<String>,
    pub intent: Option<String>,
    /// created_at (default) | keyword | popularity | est_volume | kd
    pub sort: Option<String>,
    pub desc: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct IdsInProject {
    pub project_id: String,
    /// 1 to 1000 ids.
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetSerp {
    pub project_id: String,
    pub keyword: String,
    pub country: Option<String>,
    pub language: Option<String>,
    /// Fetch a fresh page instead of the 7-day cache.
    pub refresh: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DomainRef {
    /// Domain or URL, e.g. example.com.
    pub domain: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunSiteAudit {
    pub project_id: String,
    /// Defaults to https://{project domain}/.
    pub start_url: Option<String>,
    /// Pages to crawl (1-10000, default 500).
    pub max_pages: Option<u32>,
    /// Obey robots.txt (default true). Turn off only for sites the user owns.
    pub respect_robots: Option<bool>,
    /// Render pages in a headless browser (needs the renderer service).
    pub render_js: Option<bool>,
    /// Add Lighthouse lab scores (needs the renderer service; much slower).
    pub lighthouse: Option<bool>,
    /// Path prefixes to skip, e.g. /tag/.
    pub exclude_paths: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AuditRef {
    pub project_id: String,
    /// Audit id from run_site_audit or list_site_audits.
    pub audit_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetAuditIssues {
    pub project_id: String,
    pub audit_id: String,
    /// Issue type id from the summary, e.g. missing-title. Omit for the summary.
    pub issue_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetAuditPages {
    pub project_id: String,
    pub audit_id: String,
    /// Substring of URL or title.
    pub search: Option<String>,
    /// 2xx | 3xx | 4xx | 5xx | error
    pub status: Option<String>,
    pub issue_type: Option<String>,
    pub indexable: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateRankTracker {
    pub project_id: String,
    pub name: Option<String>,
    /// Domain to track; defaults to the project's website.
    pub domain: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    /// manual (default) | daily | weekly | monthly
    pub schedule: Option<String>,
    /// Hour of day in UTC for scheduled runs (default 6).
    pub hour_utc: Option<i16>,
    /// ISO weekday for weekly runs, 1 = Monday.
    pub weekday: Option<i16>,
    /// Keywords to start with.
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrackerRef {
    pub project_id: String,
    /// Tracker id from list_rank_trackers.
    pub tracker_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddRankKeywords {
    pub project_id: String,
    pub tracker_id: String,
    pub keywords: Vec<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrackerKeywordIds {
    pub project_id: String,
    pub tracker_id: String,
    /// keyword_id values from get_rank_tracker.
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PinRankKeywords {
    pub project_id: String,
    pub tracker_id: String,
    pub ids: Vec<String>,
    /// Default true; false unpins.
    pub pinned: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunRankTracker {
    pub project_id: String,
    pub tracker_id: String,
    /// Check only these keywords; omit for all.
    pub keyword_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetRankHistory {
    pub project_id: String,
    pub tracker_id: String,
    /// keyword_id from get_rank_tracker; omit for the tracker trend only.
    pub keyword_id: Option<String>,
    /// Most recent checks to return (default 90).
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct JobRef {
    pub job_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Ga4PropertySelection {
    pub project_id: String,
    pub property_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Ga4PropertyPage {
    pub project_id: String,
    pub page_token: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Ga4ReportParams {
    pub project_id: String,
    pub start_date: String,
    pub end_date: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct IndexNowUrls {
    pub project_id: String,
    pub urls: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LinkObservationInput {
    pub project_id: String,
    pub source_url: String,
    pub target_url: String,
    pub state: String,
}
