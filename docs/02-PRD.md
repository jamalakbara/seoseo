# 02 — Product Requirements Document (PRD)

**Product codename:** SeoSeo (working name; rename freely)
**Version:** 1.0 draft · 2026-11-10
**Owner:** jamal
**Reference product:** OpenSEO v0.1.26 (see `01-reverse-engineering-analysis.md`)

---

## 1. Vision
A **fully free, open-source, self-hostable SEO platform** with feature parity (and beyond) with OpenSEO / Semrush / Ahrefs core workflows, that works **with zero paid API keys**. Rust backend for speed and cheap hosting, Next.js frontend, one `docker compose up`.

## 2. Problem
- Semrush/Ahrefs cost $100–$500+/mo. OpenSEO is "open" but every research feature needs paid DataForSEO credits.
- Small businesses, freelancers, students, agencies in emerging markets can't afford either.
- AI agents (Claude, ChatGPT, Codex, Cursor) need an SEO data layer via MCP — today only paid.

## 3. Goals / Non-goals
**Goals**
- G1: Parity with all OpenSEO user-facing features (feature catalog `03`).
- G2: Zero required spend: every feature has a free data path (`05`).
- G3: One-command self-hosting via Docker Compose; runs on a 2 vCPU / 4 GB VPS for a small team.
- G4: First-class MCP server + agent skills, compatible with OpenSEO tool names where sensible.
- G5: Extras beyond OpenSEO (see §7 "Beyond parity").

**Non-goals (v1)**
- Billing, credits, Stripe, Autumn (no monetization layer).
- Cloudflare Workers deployment target.
- Multi-region SaaS operations / guest MCP accounts (hosted-only concepts).
- A full Google-scale backlink index (we use Common Crawl + first-party sources).

## 4. Personas
| Persona | Needs |
|---|---|
| **Solo site owner** | Audit site, find keywords, track ranks, see GSC data — free. |
| **Freelancer / agency** | Many projects, client reports, white-label share links, teammates. |
| **Developer / AI power user** | MCP tools in Claude Code/Codex, skills, API, self-host. |
| **Local business marketer** | Local grid ranking, GBP info, reviews. |
| **Content marketer** | Keyword clustering, content briefs, AI visibility (GEO). |

## 5. Success metrics
- Install-to-first-audit < 5 min.
- Audit 10k pages < 15 min on 4 vCPU (polite throttle permitting).
- p95 API latency < 200 ms for cached reads.
- MCP: all tools pass schema validation; ≥ 90 tools.
- GitHub stars / active installs (anonymous opt-in telemetry only).

## 6. Functional requirements (summary — full list with IDs in `03-feature-catalog.md`)

### 6.1 Workspace
- FR-WS-1 Email/password auth, optional Google OAuth, optional "no auth" mode for private networks; first visitor = admin.
- FR-WS-2 Organizations with owner/admin/member roles; invitation links; API keys; MCP OAuth connections list/revoke.
- FR-WS-3 Projects (name, domain, brand, market country+language, optional city), archive/restore/delete.
- FR-WS-4 Project memory: typed sections, competitors, key pages (hub/spoke/money), research log — editable by user, agent, MCP.
- FR-WS-5 Onboarding wizard: website → auto discovery (sitemap, key pages, brand, competitors, seed keywords) → first audit auto-starts.

### 6.2 Dashboard
- FR-DB-1 Setup checklist; Overview (health score, new GSC queries, link activity, rank movers, audit issues delta); Keywords tab (GSC new/top/striking distance 5–20); Backlinks tab (new/lost/live).
- FR-DB-2 24 h visit-triggered refresh with lock; failures keep last data.

### 6.3 Keyword research
- FR-KW-1 Seed → ideas (autocomplete expansion, questions, related, PAA), filters (volume bucket, KD, intent, word count, include/exclude), sort, paging, CSV export.
- FR-KW-2 Metrics: est. volume (bucket + confidence), trend, KD (computed), intent, SERP features; CPC when BYOK.
- FR-KW-3 SERP analysis for a keyword (top 10–100, DA/SR per result, features).
- FR-KW-4 Saved keywords + tags + refresh + export.
- FR-KW-5 Keyword clustering (SERP-overlap clustering) — *beyond parity*.
- FR-KW-6 Local (city) keyword context.

### 6.4 Competitive research
- FR-DO-1 Domain overview: SR (authority), est. traffic, ranked keywords, top pages, competitors, position distribution.
- FR-DO-2 Keyword gap / content gap between project and up to 4 competitors.
- FR-DO-3 Domain keyword suggestions (competitor ranks, you don't).

### 6.5 Backlinks
- FR-BL-1 Overview: SR, backlinks, referring domains, new/lost, history.
- FR-BL-2 Tables: backlinks (anchor, dofollow, first/last seen), referring domains, top linked pages, anchors.
- FR-BL-3 Spam score; link prospecting (domains linking to ≥2 competitors but not you).
- FR-BL-4 Snapshots & diffs.

### 6.6 Rank tracking
- FR-RT-1 Trackers per (project, domain, country[, city]); devices; depth 10–100; schedule manual/daily/weekly/monthly at chosen time+tz.
- FR-RT-2 Add/remove/pin/tag keywords, bulk import.
- FR-RT-3 Run full/subset; record position, URL, SERP features per device.
- FR-RT-4 Views: latest table w/ deltas, keyword history chart, visibility trend, position matrix, distribution buckets, cannibalization flags.
- FR-RT-5 Alerts on movement thresholds (email/webhook) — *beyond parity*.

### 6.7 Site audit
- FR-SA-1 Start URL, max pages, JS render toggle, Lighthouse toggle, include/exclude patterns, user-agent choice, crawl speed.
- FR-SA-2 Discovery via robots.txt + sitemaps (nested) + link following; SSRF-safe; adaptive throttle; 429 cooldown.
- FR-SA-3 All OpenSEO issue rules (see `01` §6.7) **plus**: missing/multiple H1, hreflang errors, mixed content, HTTP→HTTPS, orphan pages (sitemap but no inlinks), broken internal/external links, redirected internal links, missing/invalid structured data (JSON-LD validation), missing viewport/lang, large page size, uncompressed/unminified assets, image size/format, sitemap errors (non-200/noindex/canonicalized in sitemap), robots.txt issues, pagination, duplicate H1, URL hygiene (uppercase, params, length), soft-404 detection.
- FR-SA-4 Lighthouse lab scores + CWV; CrUX field data.
- FR-SA-5 Results: health score, issues by severity/category, pages table with filters, page detail (all extracted fields, in/out links), internal link graph view, audit comparison (fixed/new issues), CSV/PDF export, scheduled re-audits.

### 6.8 Local SEO
- FR-LO-1 Business lookup (OSM / Places key), tracker with grid N×N (3,5,7,9), spacing km, keywords.
- FR-LO-2 Grid scan → heatmap (MapLibre + OSM), ARP/ATRP/SoLV metrics, competitors per point.
- FR-LO-3 Business profile, reviews (with Places key), NAP consistency check across own site schema.

### 6.9 AI visibility (GEO)
- FR-AI-1 Tracker: engines (OpenAI, Anthropic, Gemini, Perplexity, Ollama/local, Google AI Overview via SERP), market, schedule.
- FR-AI-2 Prompt generation from keywords/topics (LLM) + manual prompts; pause/archive.
- FR-AI-3 Runs: answer, cited sources, brand & competitor mentions/citations, first-mention position.
- FR-AI-4 Metrics: share of voice, mention rate, citation rate, trend; source domains leaderboard; answer viewer; export.
- FR-AI-5 Brand lookup & prompt explorer over instance-wide answer index.

### 6.10 Integrations
- FR-IN-1 GSC: OAuth, property per project, performance explorer (query/page/country/device/date, filters, compare periods), URL inspection, export.
- FR-IN-2 GA4: property per project, organic overview, landing pages, acquisition, key events, ecommerce, site search, audience, measurement health.
- FR-IN-3 Bing Webmaster Tools: inbound links, query stats.
- FR-IN-4 IndexNow key setup/verify/submit (+ auto-submit changed pages from audits, opt-in).
- FR-IN-5 Webhooks + outgoing Slack/Discord notifications.

### 6.11 Reports
- FR-RP-1 HTML reports saved by agents or generated from built-in templates (audit, rank, keyword, backlink, AI visibility); sandboxed viewer; print/PDF export.
- FR-RP-2 Templates (instruction briefs, branding: logo, accent color) — white-label.
- FR-RP-3 Public share links (token, revocable, noindex) — available in self-host too (no hosted-only restriction).
- FR-RP-4 Scheduled email digests (weekly) — *beyond parity*.

### 6.12 AI assistant ("Sam" equivalent)
- FR-AG-1 In-app chat per project, streaming, tool-calling over our own API, reads/writes project memory; works with Ollama or any OpenAI-compatible endpoint.
- FR-AG-2 Content brief generator & on-page optimizer (score page vs top-10 SERP terms) — *beyond parity*.

### 6.13 MCP & API
- FR-MCP-1 Streamable HTTP MCP endpoint, OAuth 2.1 (PKCE, dynamic client registration) + API key auth; credentials bound to one org.
- FR-MCP-2 Tool set ≥ OpenSEO's (names kept compatible), discriminated success/error outputs, errors phrased as instructions.
- FR-MCP-3 Ship agent skills (port the 12 + clustering/content-brief/local).
- FR-API-1 Public REST API with OpenAPI 3.1 spec (same handlers as UI).

### 6.14 Free tools (public, no login, rate-limited)
Keyword generator, SERP preview/simulator, backlink checker, domain authority checker, domain age checker, robots.txt tester, sitemap validator, meta tag checker, schema validator, redirect checker, hreflang checker.

### 6.15 Admin / ops
- Settings for provider adapters (enable/disable, keys, rate limits), job queue dashboard, storage usage, backup/restore, telemetry opt-in (off by default).

## 7. Beyond parity (differentiators)
Keyword clustering UI · content brief + on-page score · internal-link suggestions & graph · audit comparison & scheduled audits · CrUX field data · alerts & digests · white-label PDF · Bing WMT · tech-stack detection · log-file analysis (upload access logs → bot crawl stats) · SERP volatility index from own tracking data · change monitoring (title/meta/canonical/robots diffs on key pages) · multi-language UI.

## 8. Non-functional requirements
| Area | Requirement |
|---|---|
| Performance | Crawler ≥ 20 pages/s (throttle-limited per host); API p95 < 200 ms cached |
| Scale | 1 instance: 50 projects, 100k audited pages, 10k tracked keywords |
| Reliability | Durable jobs (Postgres-backed queue), resumable, watchdog fails stuck jobs with reason |
| Security | Argon2id passwords, CSRF, secure cookies, RBAC, tenant scoping on every query, SSRF guard, encrypted OAuth tokens (AES-256-GCM), sandboxed report HTML, rate limiting |
| Privacy | No telemetry by default; all data stays on the instance |
| Ethics | Crawlers obey robots.txt (configurable UA), polite rates; scraping adapters opt-in, never bypass CAPTCHAs |
| Portability | Docker Compose, amd64 + arm64 images |
| Observability | `tracing` JSON logs, `/health`, `/metrics` (Prometheus) |
| Accessibility | WCAG 2.1 AA UI |
| i18n | UI strings externalized |

## 9. Release plan
See `07-roadmap.md` (M0 foundation → M8 polish). MVP = M0–M3 (auth, projects, audit, keywords, SERP, rank tracking, GSC, MCP core).

## 10. Open questions
1. Product name & license (MIT like OpenSEO, or AGPL to keep forks open?).
2. Instance-wide SERP/answer index sharing between orgs on one instance — default on (better data) or off (isolation)?
3. Ship Ollama in default compose (big image) or as optional profile? (Proposed: optional profile `--profile ai`.)
4. Bundle SearXNG by default? (Proposed: yes, profile `serp`.)
