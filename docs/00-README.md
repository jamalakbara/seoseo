# SeoSeo — Documentation Index

M8 implementation and release gates: [Polish and release acceptance](31-m8-polish-and-release.md). Public tools and documentation are served at `/tools` and `/docs`.

M7 complete: [Operations and acceptance](30-m7-operations-and-acceptance.md) documents alerts, destinations, weekly email, reports/PDF, business records, backup/restore and account deletion. [Assistant and content](28-assistant-and-content.md) and [streaming/reviewed memory](29-assistant-streaming-and-memory.md) cover AG-01..03.

Free, open-source, self-hostable SEO platform. Rust (axum) backend · Next.js 16 frontend · Docker Compose. Reverse-engineered from OpenSEO (every-app/open-seo v0.1.26) and extended.

| # | Doc | What it answers |
|---|---|---|
| 01 | [Reverse-engineering analysis](01-reverse-engineering-analysis.md) | What OpenSEO is, stack, code map, full data model, every feature & flow, external APIs, invariants, gaps |
| 02 | [PRD](02-PRD.md) | Vision, goals, personas, functional & non-functional requirements, beyond-parity features, open questions |
| 03 | [Feature catalog](03-feature-catalog.md) | Every feature with ID, priority, data source, milestone, status (living backlog) |
| 04 | [Architecture](04-architecture.md) | Rust + Next.js stack with versions, system diagram, repo layout, DB, jobs, crawler, providers, security, Docker |
| 05 | [Free data sources](05-free-data-sources.md) | How each paid DataForSEO capability is replaced by free/computed data; confidence labels; risks |
| 06 | [API & MCP spec](06-api-and-mcp-spec.md) | REST endpoints, MCP tool list (OpenSEO-compatible names), skills |
| 07 | [Roadmap](07-roadmap.md) | Milestones M0–M8 with scope and exit criteria |
| 08 | [Site audit](08-site-audit.md) | How the shipped crawler, rules and audit job work; invariants and tests |
| 09 | [Browser audits and refresh](09-m2-browser-and-refresh.md) | Renderer, Lighthouse, browser SERP and bulk keyword refresh |
| 10 | [Search Console](10-search-console.md) | Google OAuth setup, project access, performance/export, inspection and fixture tests |
| 11 | [Memory and reports](11-memory-and-reports.md) | Project context, reports/templates, public sharing, isolation and regression tests |
| 12 | [Dashboard and onboarding](12-dashboard-and-onboarding.md) | Checklist, evidence review, cached GSC keywords, refresh lifecycle and regression tests |
| 13 | [M4 integrations and OAuth](13-m4-integrations-and-oauth.md) | GA4, IndexNow, optional model drafts, link observations, MCP OAuth and fixture verification |
| 14 | [Competitive research](14-competitive-research.md) | Project SERP samples, volume-based sample-click estimates, competitors, gaps and RDAP |
| 15 | [Backlink snapshots](15-backlink-snapshots.md) | Imported evidence, sample diffs, referring domains, pages/anchors, retention and tests |
| 16 | [AI visibility](16-ai-visibility.md) | Manual model comparisons, durable runs, literal mentions, coverage, setup and tests |

| 31 | [M8 polish and release](31-m8-polish-and-release.md) | Public tools, log aggregates, volatility, crawler credentials, MCP Apps, CPC, localization, telemetry and release gates |

Reference source: `reference/open-seo/` (git-ignored clone; re-clone with `git clone https://github.com/every-app/open-seo reference/open-seo`).
Upstream design records worth re-reading before building a feature: `reference/open-seo/docs/maintainers/specs/00xx-*.md`.

M5 implementation and verification: [Acceptance record](17-m5-acceptance.md). M6 includes [SERP keyword clustering](18-keyword-clustering.md), [internal links](19-internal-links.md), [audit comparison/schedules](20-audit-comparison-and-schedules.md), [cannibalization/own AI index](21-cannibalization-and-ai-index.md) and [technology/link prospects](22-technology-and-link-prospects.md). Portable [agent workflows](../plugins/seoseo/README.md) contain 14 skills for the self-hosted MCP server.
