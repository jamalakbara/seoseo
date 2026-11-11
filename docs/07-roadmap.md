# 07 — Roadmap & Milestones

Each milestone ends with: tests green, docker compose up works, docs + `03-feature-catalog.md` status updated, `aimem handoff` written.

| MS | Theme | Scope (feature IDs) | Exit criteria |
|---|---|---|---|
| **M0** | Foundation | Monorepo scaffold, Rust workspace (app/api/domain/db/jobs/common), Next.js app shell, Postgres+Valkey, migrations, auth (WS-01/02/06), projects (WS-08/09), OpenAPI (AP-01), compose (OP-01), health/metrics (OP-02), CI (fmt, clippy, test, build images) | Sign up → create project in UI via Docker |
| **M1** ✅ | Site audit core | Crawler (SA-01..05, SA-10, SA-11), job queue + SSE, orgs/invites (WS-04/05/12). WS-13 moved to M2 | Done 2026-11-10: `badseo` fixture finds all 26 expected issue types; crash-resume verified |
| **M2** ✅ | Keyword + SERP | Provider settings UI (WS-13), SERP providers (SearXNG, Chromium opt-in, BYOK), autocomplete, pageview demand estimates (Trends deferred), volume estimator, KD, intent (KW-01..09), SR authority from CC host graph (BL-08), extended audit rules + JS render + Lighthouse (SA-06..08) | Done 2026-11-11: seed → 150+ ideas with popularity, intent, labelled volume, KD via SearXNG + SR (`scripts/e2e-keywords.sh`). Extended rules, isolated JS rendering + Lighthouse, opt-in Chromium/Bing adapter and durable bulk refresh implemented. Provider-dependent PAA and Wikipedia-only volume coverage remain explicit limitations |
| **M3** | Rank tracking + GSC + MCP | RT-01..06, IN-01/02, MCP server w/ API keys (MC-01) + first ~40 tools, job dashboard (OP-03) | Claude Code connects via MCP, runs audit + keyword research + rank tracker |
| **M4** | Dashboard, memory, reports | DB-01..03/05, WS-10/11, IN-03 GA4, IN-05 IndexNow, RP-01..03, MCP OAuth (MC-02) | New project onboarding auto-runs audit; agent saves report; share link works |
| **M5** | Competitive + backlinks + AI visibility | DO-01..05/07, BL-01..06, DB-04, AI-01..04, skills (MC-05) | Domain overview of competitor; AI visibility run on 3 engines incl. Ollama |
| **M6** | Local SEO + advanced | LO-01..04/06, KW-10/11, DO-06, BL-07, SA-09/12/13, AI-05/06, IN-04 Bing | 5×5 grid heatmap; clustering UI; audit compare |
| **M7** | Assistant, alerts, content | AG-01..03, RT-08, SA-16, IN-06, RP-04..06, LO-05, OP-04, WS-07 | In-app assistant with local LLM; weekly digest email |
| **M8** | Polish & extras | Free public tools (FT), MCP widgets, log analyzer, volatility index, i18n, BYOK CPC, telemetry opt-in, docs site | v1.0 release, multi-arch images published |

## Next milestone
M3: rank tracking, GSC OAuth and measured search data, MCP server, job dashboard.

M2 setup and verification: [Browser audits and keyword refresh](09-m2-browser-and-refresh.md).
