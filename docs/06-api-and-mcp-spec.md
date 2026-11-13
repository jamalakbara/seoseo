# 06 — REST API & MCP Tool Specification

Base: `/api/v1`. Auth: session cookie (web) or `Authorization: Bearer <api_key|oauth_token>`. Errors: RFC 9457 ProblemDetails. Long-running → `202 {job_id}`.

## 1. REST endpoints (planned)

### Auth & workspace
```
POST   /auth/sign-up | /auth/sign-in | /auth/sign-out | /auth/forgot | /auth/reset | /auth/verify
GET    /me
GET    /orgs · POST /orgs/switch · GET /orgs/current/members · POST /orgs/current/invitations
POST   /invitations/{id}/accept · POST /orgs/current/transfer-ownership
GET/POST/DELETE /api-keys · GET/DELETE /mcp-connections/{id}
GET/PUT /providers (adapter settings) · GET /providers/health
```

### Projects & memory
```
GET/POST /projects · GET/PATCH/DELETE /projects/{pid} · POST /projects/{pid}/archive|restore
GET/PUT  /projects/{pid}/context         (sections, competitors, key pages, research log)
POST     /projects/{pid}/website-research  → job      · POST /projects/{pid}/website-setup
GET      /projects/{pid}/dashboard/{overview|keywords|backlinks}
GET      /locations/search?country=&q=    (GeoNames)
```

### Keywords
```
POST /projects/{pid}/keywords/research        {seeds[], mode, market, limit, filters}
GET  /keywords/metrics?kw=&loc=&lang=
GET  /projects/{pid}/keywords/serp?kw=&device=&depth=
GET/POST/DELETE /projects/{pid}/saved-keywords · POST …/refresh · GET …/export.csv
GET/POST/PATCH/DELETE /projects/{pid}/saved-keyword-tags
POST /projects/{pid}/keywords/cluster          → job
```

### Domains & backlinks
```
GET /domains/{domain}/overview?market=
GET /domains/{domain}/ranked-keywords?cursor= · /top-pages · /competitors · /tech · /age
GET /projects/{pid}/keyword-gap?competitors=a.com,b.com
GET /domains/{domain}/backlinks/{summary|links|referring-domains|top-pages|anchors|history}
POST /domains/{domain}/backlinks/refresh       → job (CC ingest)
GET /projects/{pid}/link-prospects
```

### Rank tracking
```
GET/POST /projects/{pid}/rank-trackers · GET/PATCH/DELETE …/{tid}
POST   …/{tid}/keywords (bulk) · DELETE …/{tid}/keywords · PATCH …/{tid}/keywords/pin
POST   …/{tid}/runs {keyword_ids?}  → job · GET …/{tid}/runs/latest
GET    …/{tid}/results · /history?keyword= · /trend · /matrix · /estimate
```

### Site audit
```
POST /projects/{pid}/audits {start_url, max_pages, render_js, lighthouse, include[], exclude[], ua}  → job
GET  /projects/{pid}/audits (history) · GET …/{aid} (status+summary) · DELETE …/{aid}
GET  …/{aid}/pages?filters · …/pages/{page_id} · …/issues?type=&severity= · …/lighthouse
GET  …/{aid}/links/graph · …/{aid}/compare/{other_aid} · …/{aid}/export.csv
GET  /jobs/{id} · GET /jobs/{id}/events (SSE)
```

### Local SEO
```
GET  /local/businesses/search?q=&near= · GET /local/categories
GET/POST /projects/{pid}/local-trackers · GET/PATCH …/{lid} · PUT …/{lid}/keywords
POST …/{lid}/checks → job · GET …/{lid}/scans/{sid} (grid points, competitors)
GET  /local/businesses/{id}/{profile|reviews|questions}
```

### AI visibility
```
GET/PUT /projects/{pid}/ai-tracker · POST …/ai-tracker/schedule
POST /projects/{pid}/ai-prompts/generate · /research · CRUD /projects/{pid}/ai-prompts
POST /projects/{pid}/ai-runs → job · GET …/ai-runs/{rid}
GET  /projects/{pid}/ai-visibility/{results|sources|trend|export.csv} · GET …/answers/{oid}
GET  /ai/brand-lookup?brand=&domain= · GET /ai/prompt-explorer?q=
```

### Integrations
```
GET /integrations/google/oauth/start?scope=gsc|ga4&pid= · GET /integrations/google/oauth/callback
GET /projects/{pid}/gsc/{sites|connection} · PUT …/gsc/site · DELETE …/gsc
POST /projects/{pid}/gsc/performance {dimensions, filters, range, compare} · POST …/gsc/inspect {urls[]}
GET /projects/{pid}/ga4/{properties|connection} · PUT …/ga4/property
GET /projects/{pid}/ga4/{organic-overview|landing-pages|page-performance|acquisition|key-events|ecommerce|site-search|audience|health}
GET/POST /projects/{pid}/indexnow (setup, check, submit)
GET/PUT  /projects/{pid}/bing-wmt
CRUD /webhooks · /alerts
```

### Reports
```
GET/POST /projects/{pid}/reports · GET/PUT/DELETE …/{rid} · GET /r/{rid}/raw (sandboxed)
POST …/{rid}/share · DELETE …/{rid}/share · GET /s/{token} · GET /s/{token}/raw
CRUD /projects/{pid}/report-templates · POST …/{rid}/pdf → job
```

### Assistant
```
GET/POST /projects/{pid}/assistant/sessions · POST …/{sid}/messages (SSE stream) · DELETE …/{sid}
```

### Public free tools (no auth, rate-limited)
```
GET /public/tools/{keyword-generator|serp-preview|backlinks|authority|domain-age|robots|sitemap|meta|schema|redirects|hreflang}
```

## 2. MCP tools (target parity with OpenSEO names)

Transport: Streamable HTTP at `/mcp`. Each tool: zod-like JSON Schema input, discriminated `{ok:true,data,meta:{source,confidence,fetched_at}} | {ok:false,error:{code,message(instructional)}}`.

| Group | Tools |
|---|---|
| Identity / projects | `whoami`, `list_projects`, `create_project`, `get_project_context`, `update_project_context`, `research_project_website`, `save_project_website_setup` |
| Keywords | `research_keywords`, `get_keyword_metrics`, `get_suggested_keywords`, `get_domain_keyword_suggestions`, `save_keywords`, `list_saved_keywords`, `remove_saved_keywords`, `cluster_keywords`* |
| SERP | `get_serp_results`, `find_serp_competitors`, `search_serp_locations` |
| Domain | `get_domain_overview`, `get_ranked_keywords`, `get_search_opportunities`, `get_keyword_gap`*, `detect_tech_stack`* |
| Backlinks | `get_backlinks_overview`, `get_backlinks_profile`, `find_link_prospects`* |
| Rank tracking | `create_rank_tracker`, `get_rank_tracker`, `add_rank_tracking_keywords`, `remove_rank_tracking_keywords`, `pin_rank_tracking_keywords`, `estimate_rank_tracker_cost` (→ request budget), `run_rank_tracker`, `get_rank_history` |
| Site audit | `run_site_audit`, `get_audit_status`, `get_audit_issues`, `get_audit_pages`, `list_site_audits`, `delete_site_audit`, `compare_audits`* |
| Local | `search_local_businesses`, `list_business_categories`, `get_local_serp_results`, `get_local_rank_grid`, `get_business_profile`, `get_business_reviews`, `get_business_updates`, `get_google_business_questions` |
| AI visibility | `generate_ai_visibility_prompts`, `research_ai_visibility_prompts`, `complete_ai_research_setup`, `save_ai_visibility_tracker`, `get_ai_visibility_tracker`, `estimate_ai_visibility_cost`, `run_ai_visibility_check`, `get_ai_visibility_run`, `get_ai_visibility_results`, `get_ai_visibility_sources`, `get_ai_visibility_trend`, `get_ai_visibility_answer`, `export_ai_visibility_data`, `set_ai_visibility_schedule`, `explore_prompt` |
| GSC | `list_search_console_sites`, `set_search_console_site`, `get_search_console_performance`, `inspect_urls` |
| GA4 | `list_google_analytics_properties`, `set_google_analytics_property`, `get_google_analytics_{organic_overview,organic_landing_pages,page_performance,traffic_acquisition,key_events,ecommerce_performance,site_search,audience_breakdown,measurement_health}` |
| Reports | `save_report`, `get_report`, `list_reports`, `delete_report`, `save_report_template`, `list_report_templates`, `delete_report_template`, `set_report_sharing` |
| Indexing | `submit_indexnow_urls` |
| Content* | `create_content_brief`*, `score_page_content`* |

`*` = new beyond OpenSEO. Cost-estimate tools return request counts & provider quota impact instead of credits.

### Implemented (M3 + first M4 delivery)
GSC REST routes, OAuth setup, access rules and limits: [Search Console](10-search-console.md).

The M3 set plus `get_project_context`, `update_project_context`, `save_report`, `get_report`, `list_reports`, `delete_report`, `save_report_template`, `list_report_templates`, `delete_report_template`, and `set_report_sharing` (43 tools total before the dashboard/onboarding delivery).

- Endpoint: `POST /mcp` (also reachable through the web app origin). Stateless: no session ids, JSON responses, any API replica can answer.
- Auth: `Authorization: Bearer seo_…` API key (or the built-in admin in `AUTH_MODE=none`). 401 carries `WWW-Authenticate: Bearer`.
- Result: one text block holding `{"ok":true,"data":…}` or `{"ok":false,"error":{"code","message"}}` with `isError` set. Provenance (`provider`, `exact`, `*_source`, `*_confidence`) lives inside `data`, next to the value it describes.
- `Host` must be the `WEB_BASE_URL` host, localhost or the compose service name (DNS-rebinding guard).
- Connect: `claude mcp add --transport http seoseo https://your-host/mcp --header "Authorization: Bearer seo_…"`.

## 3. Agent skills to ship (`skills/`)
Port & adapt (MIT-licensed upstream — keep attribution): seo-project-setup, keyword-research, keyword-clustering, competitor-analysis, competitive-landscape, link-prospecting, local-seo, seo-audit, ai-prompt-research, ai-visibility-audit, seo-report, seo-coach. New: content-brief, technical-fix-plan.

### Dashboard and website onboarding (M4 continuation)

Six additional MCP tools (49 total): `get_dashboard_overview`, `get_dashboard_keywords`, `refresh_dashboard`, `research_project_website`, `get_project_website_setup`, `save_project_website_setup`.

REST: `GET /projects/{pid}/dashboard/overview`, `GET /projects/{pid}/dashboard/keywords`, `POST /projects/{pid}/dashboard/refresh`, `PUT /projects/{pid}/dashboard/checklist`, `GET/POST /projects/{pid}/website-setup`, `POST /projects/{pid}/website-research`. All use existing tenant authentication; research/setup writes need admin. Saved setup uses the existing revisioned `/context` document, preserving other sections, competitors and pages. See [Dashboard and onboarding](12-dashboard-and-onboarding.md).

### M4 completion

The subsequent first M5 delivery adds `get_domain_overview` (67 tools total) and `GET /projects/{project_id}/domain-overview`; see [Competitive research](14-competitive-research.md) for evidence scope and limits.

M5 continuation adds `get_domain_age` (68 tools total) and `POST /projects/{project_id}/domain-age` for on-demand, cached registry registration dates. This is separate from SERP inspection and never substitutes other RDAP event dates for registration.

Imported backlink snapshots add `list_backlink_snapshots`, `import_backlink_snapshot`, `get_backlink_snapshot` and `delete_backlink_snapshot` (72 tools total), with corresponding `/projects/{project_id}/backlink-snapshots` REST routes. See [Backlink snapshots](15-backlink-snapshots.md) for dataset/domain identity, comparison semantics and limits.

Manual AI visibility adds eight model/tracker/run tools (80 tools total), model availability at `/ai-visibility/models`, and `/projects/{project_id}/ai-trackers` tracker/run routes. See [AI visibility](16-ai-visibility.md) for operator setup, answer-sample semantics, durable jobs and limits.

MCP now exposes 66 tools: the previous 49 plus property discovery/selection, nine Analytics report tools, IndexNow setup/verification/submission, website synthesis and link-observation list/save. OAuth discovery/DCR/PKCE/consent and rotating resource-bound tokens complement existing API keys. Feature endpoints and protocol details are documented in [M4 integrations and OAuth](13-m4-integrations-and-oauth.md).

### M6 stored evidence research (95 tools)

`GET /projects/{project_id}/cannibalization` / `get_cannibalization_flags` return same-snapshot multi-URL candidates with source/coverage. `GET /projects/{project_id}/ai-index` / `lookup_ai_brand`, `explore_ai_prompts` search the current website's retained model evidence with frozen market and successful/failed/unchecked denominators. Both are read-only and make no provider calls. See [stored evidence research](21-cannibalization-and-ai-index.md).

### M6 audit comparison and recurring crawls (92 tools)

`GET /projects/{project_id}/audits/{audit_id}/comparison` / `compare_site_audits` compare completed same-start audit observations, preserving unknown absences and pagination. `GET/PUT /projects/{project_id}/audit-schedule` / `get_audit_schedule`, `set_audit_schedule` provide revisioned admin-controlled UTC recurring crawls from a reviewed completed baseline. See [audit comparison and schedules](20-audit-comparison-and-schedules.md).

### M6 internal links (89 tools)

`GET /projects/{project_id}/audits/{audit_id}/internal-links` and MCP `get_internal_link_graph` expose completed-audit graph evidence and conservative editorial suggestions. Page limit is 10–500 (default 200); edges cap at 5000. Full-audit counts remain distinct from sampled graph edges, and incomplete evidence never proves website-wide orphan status. Read-only with no crawl, job or provider requests. See [internal links](19-internal-links.md).

### M6 keyword clustering (88 tools)

`GET /projects/{project_id}/keyword-clusters` and MCP `cluster_keywords` return conservative page-topic suggestions from the project's recent cached SERP evidence. Country/language default to the project; min_shared is 2–10 (default 3). No provider requests or database writes occur. See [keyword clustering](18-keyword-clustering.md) for coverage, grouping and export semantics.

### M5 completion (87 tools)

The runtime MCP schemas and generated OpenAPI are authoritative. Added `import_competitive_search_volumes`; `discover_backlinks`, `list_backlink_discoveries`, `get_dashboard_backlinks`; `set_ai_visibility_schedule`, `research_ai_visibility_prompts`, `get_ai_visibility_summary`. Domain overview accepts `limit`/`offset`. REST paths and payloads are documented in guides 14–16. Fourteen portable skills ship in `plugins/seoseo/skills`; see its install guide. Cloud adapters are fixture-verified; the three-live-engine acceptance gate remains pending credentials.
