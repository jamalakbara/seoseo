# 01 — Reverse-Engineering Analysis: OpenSEO (every-app/open-seo)

> Source analyzed: `https://github.com/every-app/open-seo` @ `46b815b` (v0.1.26, Nov 2026), cloned to `reference/open-seo/` (git-ignored, read-only reference).
> Method: static source analysis (directory inventory, Drizzle schemas, server functions, MCP tool registry, workflows, DataForSEO client, maintainer specs `docs/maintainers/specs/0001–0020`).
> Note: the REA MCP `analyze_javascript_application` tool was attempted on `src/`, `src/serverFunctions`, `src/server/workflows` and crashed each time (`unreadable_output` / `Connection closed`). All findings below come from direct source reading; file paths are cited so every claim can be re-verified.

---

## 1. What OpenSEO is

"Open source alternative to Semrush and Ahrefs." A pay-as-you-go SEO suite whose **data comes almost entirely from DataForSEO** (a paid API). The hosted version (openseo.so) charges $10/mo + resells DataForSEO calls at +28% markup via "credits". Self-hosters bring their own DataForSEO key.

Differentiators it markets:
- First-class **MCP server** (~90 tools) + **Agent Skills** (Claude Code, Codex, Cursor, ChatGPT, Hermes).
- Focused workflows, modern UI.
- In-app AI agent "**SAM**" (OpenRouter LLM).
- Self-hostable (Docker or Cloudflare).

**Key insight for us:** OpenSEO is not "free". Every core research feature (keywords, SERP, backlinks, domain overview, rank tracking, local grid, AI visibility, Lighthouse) is a metered DataForSEO call. Only the site-audit crawler, GSC, GA4, IndexNow, reports and project memory are truly free. Our product's core job is replacing the paid data layer with free/self-computed sources. See `05-free-data-sources.md`.

---

## 2. Tech stack (theirs)

| Layer | Tech |
|---|---|
| Framework | TanStack Start (React 19, SSR) + TanStack Router/Query/Form/Table/DB |
| Runtime | Cloudflare Workers (+ Workflows, Durable Objects, KV, R2, D1, Browser Rendering); Docker edition emulates workers via `workerd`/wrangler |
| DB | Drizzle ORM, dual dialect: SQLite/D1 and Postgres (schemas mirrored in `src/db/*.schema.ts` and `src/db/pg/*.schema.ts`) |
| Auth | Better Auth (email/password, Google OAuth, organizations, API keys), Cloudflare Access JWT, `local_noauth`, Every App gateway |
| MCP | `@modelcontextprotocol/sdk` + `@cloudflare/workers-oauth-provider` (OAuth 2.1 for MCP clients) + MCP Apps widgets |
| AI | Vercel AI SDK v6, OpenRouter provider, `@cloudflare/think` / `agents` for SAM |
| Billing | Autumn (Stripe) credits, holds, top-ups, auto-reload |
| UI | Tailwind 4, shadcn on Base UI, lucide, recharts, leaflet (maps), sonner, react-markdown |
| Parsing | htmlparser2, cheerio (old), robots-parser, fast-xml-parser (sitemaps), tldts |
| Crawl auth | `web-bot-auth` / `http-message-sig` (signed crawler for Shopify) |
| Analytics | PostHog; self-host anonymous telemetry heartbeat |
| Tests | Vitest, Playwright (e2e incl. `tests/badseo` – a deliberately broken SEO site fixture) |

Auth modes (`AUTH_MODE`): `cloudflare_access`, `local_noauth`, `hosted`, `password` (Docker standalone), `everyapp`.

---

## 3. Code map

```
src/
  routes/                 TanStack file routes (UI + API)
    _app/p/$projectId/    per-project pages: index(dashboard), keywords, saved, domain, backlinks,
                          brand-lookup, prompt-explorer, ai-visibility(+prompts), audit(+issues),
                          rank-tracking, local-seo, search-performance, reports, sam, context, settings
    api/                  auth, autumn, ga4/oauth, gsc/oauth, local-seo/maps tiles, health
    s/$token              public share page for reports;  r/  report routes; report-map tiles
  serverFunctions/        RPC layer (createServerFn) – ~170 functions, see §6
  server/
    features/<feature>/{services,repositories}   business logic (service → repository)
    lib/dataforseo/       typed DataForSEO client (core, labs, serp, backlinks, business, ai, lighthouse, google-ads, pricing, request-slots)
    lib/audit/            crawler: discovery (robots/sitemap), url-policy (SSRF), throttle, page-analyzer, issues
    workflows/            durable jobs: SiteAuditWorkflow, RankCheckWorkflow, LocalRankWorkflow, AiVisibilityWorkflow
    mcp/tools/            MCP tool definitions (~90)
    features/sam/         in-app agent (system prompt, tools, skills)
  db/                     Drizzle schemas (sqlite + pg)
  client/features/        React feature modules mirroring server features
web/                      marketing site + free public tools (keyword generator, backlink checker, SERP simulator, …)
plugins/openseo/skills/   12 shipped agent skills (SKILL.md)
deploy/                   docker, standalone (nginx + runtime), alchemy (Cloudflare IaC)
```

Pattern enforced by AGENTS.md: **server function → service → repository**; every query scoped by `organizationId`/`projectId`; schema must work on SQLite and Postgres.

---

## 4. Domain model (extracted from `src/db/pg/*.schema.ts`)

### Identity / tenancy
- `user`, `session`, `account`, `verification`, `organization`, `member`, `invitation`, `apikey` (Better Auth).
- `user_onboarding_answers` (interested features, work-for, client site count, found via, MCP intent), `user_signup_campaigns` (UTM), `guest_accounts` (MCP guest w/ IP hash + claim token), `account_deletion_requests`.
- Roles: owner (billing), admin (projects, integrations). All invitees are admins.

### Projects
- `projects`: id, organization_id, name, brand_name, domain, location_code (default 2840 = US), language_code (en), ai_research_keywords, indexnow_key, archived.
- Project memory (spec 0010): `project_context_sections` (key, title, content, updated_by ∈ user|sam|mcp), `project_competitors` (domain, name, notes), `project_key_pages` (url, role ∈ hub|spoke|money|other, topic), `project_research_log` (date, summary).
- `project_activation_state`, `organization_activation_state`, `dashboard_step_dismissals`, `data_refresh_claims` (24 h refresh lock).

### Keywords
- `saved_keywords` (keyword, location, language), `saved_keyword_tags` (name, color), `saved_keyword_tag_assignments`.
- `keyword_metrics` cache: search_volume, cpc, competition, keyword_difficulty, intent, monthly_searches (JSON).

### Rank tracking
- `rank_tracking_configs`: domain, location_code, language, devices (desktop|mobile|both), serp_depth (10–100 step 10, default 40), schedule_interval (manual|daily|weekly|monthly), schedule time, location_name (city-level), is_active, last_skip_reason.
- `rank_tracking_keywords`: keyword, match_case, pinned, cached volume/KD/CPC.
- `rank_check_runs`: status, keywords_total/checked, is_subset_run, error.
- `rank_snapshots`: run_id, keyword, device, position (null = not found), url, serp_features (JSON).

### Backlinks
- `backlink_snapshots`: rank, backlinks, referring_domains, broken, new/lost backlinks, new/lost referring domains.

### Site audit
- `audits`: start_url, status, workflow id, config JSON, pages crawled/total, lighthouse total/completed/failed, current_phase, error code/detail, failed_phase.
- `audit_pages`: url, status_code, redirect_url, title, meta_description, canonical, robots_meta, og_*, word_count, images_total, images_missing_alt, internal/external link counts, has_structured_data, is_indexable, x_robots_tag, header canonical, crawl_depth, in_sitemap, content_hash, fetch_class, response_time_ms.
- `audit_issues`: page_id, page_url, issue_type, severity (critical|warning|info), details_json.
- `audit_lighthouse_results`: strategy (mobile|desktop), perf/a11y/best-practices/SEO scores, LCP, CLS, INP, TTFB.
- `crawler_credentials`: per-host signed-crawler credentials (Shopify).

### Local SEO (geo-grid rank tracking)
- `local_rank_trackers`: business title, address, cid, place_id, grid_size, language.
- `local_rank_keywords`, `local_rank_checks`, `local_rank_scans` (per keyword), `local_rank_points` (row, col, rank, results_count), `local_rank_businesses` (competitors in pack: title, cid, place_id, reviews), `local_rank_results` (point × business → rank). Grid spacing default 2 km, N×N points.

### AI visibility (LLM brand tracking)
- `ai_trackers`: engines enabled (chatgpt, gemini, claude, perplexity, google_ai_overview), market, schedule (daily|weekly|monthly), next_check_at.
- `ai_prompts`: topic, text, paused, archived.
- `ai_runs`: trigger (baseline|scheduled|manual), status (queued|running|completed|partial|failed).
- `ai_observations`: run × prompt × engine, branded, answer_markdown.
- `ai_sources`: cited url/domain/title/position.
- `ai_matches`: brand/competitor name+domain, own, mentioned, cited, first_mention.

### Integrations
- `gsc_connections` (project → property, connecting user's Google grant), `ga4_connections` (property id, tz, currency).

### Reports
- `reports`: title, summary (md ≤2500), html (≤500 KB), skill, template_id, created_by (client label), share_token (192-bit), shared_at.
- `report_templates`: name, description, instructions (md ≤3000), ≤10 per project.

### Agent / billing / telemetry
- `sam_sessions` (chat sessions per project/user).
- `billing_customer_status` (Autumn mirror). — **we drop billing entirely.**
- `telemetry_state` (install id, heartbeat, MCP call count).

---

## 5. External dependencies → what each feature costs them

DataForSEO endpoints in use (from `src/server/lib/dataforseo/*`):

| Area | Endpoints |
|---|---|
| Keyword research | `dataforseo_labs/google/{keyword_suggestions,keyword_ideas,related_keywords,keyword_overview}/live`, `keywords_data/google_ads/{search_volume,keywords_for_keywords}/live` |
| Domain overview | `dataforseo_labs/google/{domain_rank_overview,ranked_keywords,relevant_pages,serp_competitors}/live` |
| SERP / rank tracking | `serp/google/organic/live/advanced`, `serp/google/organic/task_post` + `task_get/advanced`, `serp/google/locations/{iso}` |
| Backlinks | `backlinks/{summary,backlinks,referring_domains,domain_pages_summary,history}/live` |
| Local SEO | `serp/google/maps/{live,task_post,task_get}`, `serp/google/local_finder/live/advanced`, `business_data/business_listings/{search,categories}`, `business_data/google/{my_business_info,reviews,extended_reviews,questions_and_answers,my_business_updates}` |
| AI visibility | `ai_optimization/{chat_gpt,gemini}/llm_scraper`, `ai_optimization/perplexity/llm_responses/live`, `ai_optimization/llm_mentions/{search,top_pages,aggregated_metrics,cross_aggregated_metrics}/live` |
| Lighthouse | `on_page/lighthouse/live/json` |
| Misc | `appendix/user_data`, `appendix/goto_url_converter` |

Other: Google Search Console API (free), GA4 Data API (free), Google Map Tiles API (local SEO basemap), OpenRouter (SAM LLM), Context.dev / Cloudflare Browser Rendering (JS rendering for audits), Ahrefs DR lookup (`serverFunctions/ahrefs.ts`), IndexNow (Bing/Yandex/Yep, free), Loops (email), PostHog, Autumn/Stripe.

---

## 6. Feature inventory with flows

### 6.1 Projects & onboarding
- Create project (name, domain, country/language market). Archive / restore / delete archived.
- Onboarding: profile questions → website + country → "onboarding agent" (spec 0005): sitemap-first discovery → scrape 3–5 key pages to markdown → keyword data → one LLM call writes strategy, streamed live.
- `researchProjectWebsite` / `saveProjectWebsiteSetup`: reads site, proposes brand, competitors, keywords, key pages.
- Saving a website auto-starts first 50-page audit with JS rendering.

### 6.2 Dashboard (spec 0015)
Tabs: **Overview** (setup checklist, new search queries from GSC, recent link activity), **Keywords** (GSC new keywords, top keywords by clicks, striking-distance = pos 5–20 sorted by impressions), **Backlinks** (new / lost / live links, ≤25 ref domains, filters: text, link type, DR, spam score). Visit-triggered refresh every 24 h with refresh claims; failures keep old data.

### 6.3 Keyword research
- Input seed keyword(s) + market → modes: auto (blend suggestions+ideas), related, suggestions, ideas. Default 150 rows.
- Columns: keyword, volume, monthly trend (12 mo), CPC, competition, KD (0–100), intent badge, SERP features.
- SERP analysis card for a keyword (top results).
- Save keywords → **Saved keywords** list with tags (color), export CSV, refresh metrics, remove.
- City-level local volume (Google Ads `search_volume` with location_name).

### 6.4 Domain overview / competitor research
- Domain → rank overview (organic traffic estimate, keyword count, positions distribution), ranked keywords table (filters, paging), top pages table, SERP competitors, domain keyword suggestions (keywords competitor ranks for that you don't).

### 6.5 Backlinks
- Overview (summary: rank, backlinks, ref domains, broken, new/lost), backlinks rows, referring domains, top pages, history chart. Snapshots stored. Free tools: backlink checker, spam score checker, domain age checker.

### 6.6 Rank tracking
- Config per (project, domain, market[, city]): devices, depth, schedule (manual/daily/weekly/monthly, UTC time, weekday).
- Add/remove/pin keywords, cost estimate, trigger run (full or subset), RankCheckWorkflow posts SERP tasks, polls, records position + URL + SERP features per device.
- Views: latest results, keyword history, config trend (avg position, visibility), position matrix (keyword × date), skip reasons.

### 6.7 Site audit (free in OpenSEO — fully replicable)
Flow (`SiteAuditWorkflow`, spec 0009): start URL probe (≤5 redirects) → discovery (robots.txt ≤500 KB, sitemaps ≤3 levels/300 docs/10 MB, concurrency 5) → crawl (frontier in Durable Object scratchpad, adaptive throttle 1–10 s interval, cooldown up to 2 h on 429, SSRF url-policy, max HTML 1 MB, links/images ≤1000 per page) → optional JS rendering → per-page analysis → multi-page checks → Lighthouse sample (mobile/desktop) → persist → summary.
Config: max pages, render JS, Lighthouse on/off.

Issue rules (`src/server/lib/audit/issues/*`):

| Issue | Rule |
|---|---|
| missing-title / title-too-short / title-too-long | <10 or >60 chars |
| missing-meta-description / too-short / too-long | <70 or >160 chars |
| duplicate-title, duplicate-meta-description, duplicate-content | grouped across pages (content hash), sample 3 |
| broken-page | 4xx |
| server-error | 5xx |
| redirect-chain / redirect-loop | multi-hop / cycle |
| canonical-conflict / canonicalized-page | HTML vs header canonical mismatch / canonical points elsewhere |
| noindex-page / blocked-page | meta/X-Robots noindex / robots.txt disallow |
| thin-content | <150 words |
| images-missing-alt | count |
| heading-order-skip | e.g. H2 → H4 |
| deep-page | crawl depth ≥5 |
| no-outgoing-links | page has no internal links out |
| slow-response | >1.5 s (warn) / >10 s |
| rate-limited-page | 429 |
| javascript-rendering-suspected | low-content HTML suggesting CSR |
| Lighthouse issues | per-audit category failures, exportable |

Results UI: summary, pages table (filters), issues view grouped by type/severity, Lighthouse issues, rendering warnings, Shopify warnings, CSV export, audit history.

### 6.8 Local SEO
- Look up business (Google Business listing search → cid/place_id), create tracker with N×N grid (spacing ~2 km) around business, keywords.
- LocalRankWorkflow: per keyword × grid point Maps SERP → rank of business, competitors at each point. Heatmap on Leaflet with Google tiles. Business profile, reviews, Q&A, updates tools.

### 6.9 AI visibility (GEO / LLM SEO)
- Setup: generate prompts from keywords/topics (`generateAiVisibilityPrompts`, `researchAiVisibilityPrompts`), pick engines, schedule.
- Run (`AiVisibilityWorkflow`): for each prompt × engine fetch answer → parse cited sources → match own brand + competitors (mentioned, cited, first mention position).
- Results: share of voice, mention rate, citation rate, trend, sources (top cited domains/pages), per-answer view, CSV export.
- **Brand lookup** (LLM mentions aggregated metrics) and **Prompt explorer** (search what prompts mention a domain).

### 6.10 Search performance (GSC)
- OAuth incremental grant, pick property per project. Report: clicks, impressions, CTR, position by query/page/country/device/date; table + export; URL inspection (1–10 URLs).

### 6.11 Google Analytics 4
- Property per project. Reports: organic overview, landing pages, page performance, traffic acquisition, key events, ecommerce, site search, audience breakdown (device/country/new-vs-returning), measurement health.

### 6.12 IndexNow (spec 0016)
- Per-project 32-hex key, verify `https://host/{key}.txt` on bare + www, submit ≤10k URLs grouped by host → Bing, fallback Yandex → Yep.

### 6.13 Reports & templates (specs 0012–0014)
- Agents save self-contained HTML reports (≤500 KB, shape-checked), served sandboxed (CSP `sandbox`), print/export, full screen. Templates (≤10/project) are instruction briefs. Public share link `/s/<token>` (192-bit token, revocable, noindex, 60 s edge cache).

### 6.14 SAM (in-app AI agent)
- Chat sessions per project; tools include project data reads, `map_links`, `read_pages`, `get_product_info`; skills; system prompt with fact sheet; reads/writes project memory.

### 6.15 MCP server + Agent skills
- OAuth 2.1 MCP (consent page, org-bound credentials, spec 0017), API keys, guest accounts (spec 0019), MCP Apps widgets (interactive SERP & GSC views).
- Tools (90, grouped): projects/context (`list_projects`, `create_project`, `get/update_project_context`, `research_project_website`, `save_project_website_setup`, `whoami`); keywords (`research_keywords`, `get_keyword_metrics`, `get_suggested_keywords`, `get_domain_keyword_suggestions`, `save/list/remove_saved_keywords`); SERP (`get_serp_results`, `show_serp_results`, `find_serp_competitors`, `search_serp_locations`); domain (`get_domain_overview`, `get_ranked_keywords`, `get_search_opportunities`); backlinks (`get_backlinks_overview`, `get_backlinks_profile`); rank tracking (`create_rank_tracker`, `get_rank_tracker`, `add/remove/pin_rank_tracking_keywords`, `estimate_rank_tracker_cost`, `run_rank_tracker`, `get_rank_history`); audit (`run_site_audit`, `get_audit_status/issues/pages`, `list_site_audits`, `delete_site_audit`); local (`search_local_businesses`, `list_business_categories`, `get_local_serp_results`, `get_local_rank_grid`, `get_business_profile/reviews/updates`, `get_google_business_questions`); AI visibility (`generate/research_ai_visibility_prompts`, `save/get_ai_visibility_tracker`, `estimate_ai_visibility_cost`, `run_ai_visibility_check`, `get_ai_visibility_run/results/sources/trend/answer`, `export_ai_visibility_data`, `set_ai_visibility_schedule`, `explore_prompt`, `complete_ai_research_setup`); GSC (`list_search_console_sites`, `set_search_console_site`, `get_search_console_performance`, `show_search_console_performance`, `inspect_urls`); GA4 (`list/set_google_analytics_property`, 9 `get_google_analytics_*`); reports (`save/get/list/delete_report`, `save/list/delete_report_template`, `set_report_sharing`); `submit_indexnow_urls`.
- Skills (12): ai-prompt-research, ai-visibility-audit, competitive-landscape, competitor-analysis, keyword-clustering, keyword-research, link-prospecting, local-seo, seo-audit, seo-coach, seo-project-setup, seo-report.

### 6.16 Team, settings, account
- Invite teammates (admins), transfer ownership, switch org, MCP connections list/revoke, API keys, crawler credentials (Shopify), Google accounts removal impact, account deletion (grace period), product-update opt-in, terms acceptance.

### 6.17 Marketing site free tools (`web/`)
Keyword generator, competitor keyword finder, backlink checker, spam score checker, domain age checker, website traffic checker, SERP simulator — protected by Turnstile + budget caps.

### 6.18 Billing (hosted only — out of scope for us)
Autumn credits, holds, top-ups, auto-reload, plan upgrades, invoices. Each DataForSEO call = credit hold → actual cost → deduct.

---

## 7. Cross-cutting invariants worth copying
1. Every query tenant-scoped (org → project). Lookups by id return owner project, then authorize.
2. SSRF-safe fetcher for any user-supplied URL (block private IPs, redirects re-checked).
3. Long jobs are durable, resumable, idempotent; jobs never stay `running` forever (watchdog sets `failed` with reason).
4. Provider data cached with explicit freshness; failed refresh keeps last good data.
5. Agent-facing errors are written as instructions.
6. Unknown ≠ zero: missing metrics are `null`, never synthesized.
7. Reports sandboxed; share tokens are capabilities, revocation = null.

## 8. Weaknesses / gaps we can beat
- Paid data for every research feature → we go free-first (self-computed + free APIs + optional BYOK).
- Cloudflare lock-in & dual-dialect Drizzle complexity → single Postgres, single Docker Compose.
- No content optimization / on-page editor, no internal-link suggestions graph UI, no log-file analysis, no schema validator, no hreflang/i18n audit depth, no Core Web Vitals field data (CrUX), no alerting/digests, no white-label PDF, no keyword clustering in-app (skill only), no SERP volatility index, no content-gap tool UI, no change monitoring.
