# SeoSeo — Documentation Index

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
| 14 | [Competitive research](14-competitive-research.md) | First M5 delivery: project SERP evidence, domain overview, competitors and keyword gaps |
| 15 | [Backlink snapshots](15-backlink-snapshots.md) | Imported evidence, sample diffs, referring domains, pages/anchors, retention and tests |
| 16 | [AI visibility](16-ai-visibility.md) | Manual model comparisons, durable runs, literal mentions, coverage, setup and tests |

Reference source: `reference/open-seo/` (git-ignored clone; re-clone with `git clone https://github.com/every-app/open-seo reference/open-seo`).
Upstream design records worth re-reading before building a feature: `reference/open-seo/docs/maintainers/specs/00xx-*.md`.
