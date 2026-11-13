# 07 — Roadmap & Milestones

Each milestone ends with: tests green, docker compose up works, docs + `03-feature-catalog.md` status updated, `aimem handoff` written.

| MS | Theme | Scope (feature IDs) | Exit criteria |
|---|---|---|---|
| **M0** | Foundation | Monorepo scaffold, Rust workspace (app/api/domain/db/jobs/common), Next.js app shell, Postgres+Valkey, migrations, auth (WS-01/02/06), projects (WS-08/09), OpenAPI (AP-01), compose (OP-01), health/metrics (OP-02), CI (fmt, clippy, test, build images) | Sign up → create project in UI via Docker |
| **M1** ✅ | Site audit core | Crawler (SA-01..05, SA-10, SA-11), job queue + SSE, orgs/invites (WS-04/05/12). WS-13 moved to M2 | Done 2026-11-10: `badseo` fixture finds all 26 expected issue types; crash-resume verified |
| **M2** ✅ | Keyword + SERP | Provider settings UI (WS-13), SERP providers (SearXNG, Chromium opt-in, BYOK), autocomplete, pageview demand estimates (Trends deferred), volume estimator, KD, intent (KW-01..09), SR authority from CC host graph (BL-08), extended audit rules + JS render + Lighthouse (SA-06..08) | Done 2026-11-11: seed → 150+ ideas with popularity, intent, labelled volume, KD via SearXNG + SR (`scripts/e2e-keywords.sh`). Extended rules, isolated JS rendering + Lighthouse, opt-in Chromium/Bing adapter and durable bulk refresh implemented. Provider-dependent PAA and Wikipedia-only volume coverage remain explicit limitations |
| **M3** (core verified; follow-ups) | Rank tracking + GSC + MCP | RT-01..06, IN-01/02, MCP server w/ API keys (MC-01), job dashboard (OP-03) | 2026-11-12: rank/MCP/Jobs E2E repaired and verified; GSC OAuth, property selection, performance/CSV, inspection and four MCP tools shipped (33 tools total). Browser checks + offline GSC fixture pass. Remaining catalog items explicitly retained as follow-ups below. |
| **M4** ✅ | Dashboard, memory, reports | DB-01..03/05, WS-10/11, IN-03 GA4, IN-05 IndexNow, RP-01..03, MCP OAuth (MC-02) | New project onboarding auto-runs audit; agent saves report; share link works |
| **M5** (implemented; live-cloud acceptance pending) | Competitive + backlinks + AI visibility | DO-01..05/07, BL-01..06, DB-04, AI-01..04, skills (MC-05) | Domain overview of competitor; AI visibility run on 3 engines incl. Ollama |
| **M6** (in progress) | Local SEO + advanced | LO-01..04/06, KW-10/11, DO-06, BL-07, RT-07, SA-09/12/13, AI-05/06, IN-04 Bing | KW-11, RT-07, SA-12/13 and AI-05 delivered; local grid heatmap and remaining data integrations/advanced scope remain |
| **M7** | Assistant, alerts, content | AG-01..03, RT-08, SA-16, IN-06, RP-04..06, LO-05, OP-04, WS-07 | In-app assistant with local LLM; weekly digest email |
| **M8** | Polish & extras | Free public tools (FT), MCP widgets, log analyzer, volatility index, i18n, BYOK CPC, telemetry opt-in, docs site | v1.0 release, multi-arch images published |

## M4 complete

M4 implementation is complete: project memory and reports, dashboard checklist/health/query/rank views, recorded link activity, reviewed website onboarding with optional configured model synthesis, nine fixed GA4 reports, IndexNow setup/ownership verification/submission and workspace-bound MCP OAuth. See [Dashboard and onboarding](12-dashboard-and-onboarding.md) and [Integrations and OAuth](13-m4-integrations-and-oauth.md) for configuration, limits and verification.

GA4/model access requires operator configuration; production consent and live provider access are separate from offline fixture checks. Link activity uses explicit user/agent observations; automatic backlink discovery and analysis remain M5. IndexNow does not automatically submit audit pages. Existing M3 follow-ups below remain unchanged.

M5 implementation is complete: [competitive research](14-competitive-research.md), [backlink discovery and evidence](15-backlink-snapshots.md), [AI visibility](16-ai-visibility.md) and [14 portable skills](../plugins/seoseo/README.md), with 87 MCP tools in total. Domain and backlink data are explicitly bounded samples; eligible imported search counts support sample-click estimates, never total domain traffic. Backlink rechecks preserve inconclusive evidence. AI supports compatible gateways, native Ollama, Anthropic (web search) and Gemini (standard text, no Google Search grounding), scheduling, prompt drafts, rates/trends and citations.

Acceptance evidence and reproducible commands: [M5 acceptance](17-m5-acceptance.md). The exact roadmap exit condition requiring a live run on three engines is **not yet met**: real local Ollama passed, while Anthropic/Gemini were checked against offline request/response fixtures. No cloud credentials were available. Do not infer real cloud access from fixtures or mark this gate passed. M6 development has started independently of this credential-dependent operator check.

## M6 in progress

KW-11 is implemented: [SERP keyword clustering](18-keyword-clustering.md). SA-12 adds [internal link graphs and suggestions](19-internal-links.md). SA-13 adds [audit comparison and scheduled re-audits](20-audit-comparison-and-schedules.md). RT-07 and AI-05 add [multi-URL candidates and the own AI prompt/brand index](21-cannibalization-and-ai-index.md), with bounded evidence, provenance and partial-collection coverage. The server now exposes 95 MCP tools. Remaining M6 scope includes local SEO/grid scans, local demand context, tech detection, link intersect, CrUX, AI Overview capture and Bing Webmaster Tools. M3 follow-ups remain below.

## M3 retained follow-ups
M3 core is shipped. Per user direction on 2026-11-12, retain keyword file import (RT-02), per-tracker timezones (RT-03), keyword/run matrix (RT-05), and dashboard retry (OP-03) as follow-ups. Device/city/depth controls (RT-01/04) depend on provider support. These rows remain `wip` in the feature catalog; M3 is not marked fully complete.

GSC operator setup and verification: [Search Console](10-search-console.md). Real Google consent/property access still needs a configured Google Cloud client.

M2 setup and verification: [Browser audits and keyword refresh](09-m2-browser-and-refresh.md).
