# M6 acceptance — 2026-11-14

M6 implementation is delivered on `codex/m6-field-and-bing`. **Full milestone acceptance is still pending.** The original implementation session used offline fixtures because preview execution and credentials were unavailable. The operator has since configured live credentials and started the current app. Live/browser acceptance is now in progress, with the checkpoint below distinguishing verified results from remaining work. No merge or push has been performed.

## Live acceptance checkpoint

On 2026-11-14, the signed-in browser displayed the updated project navigation and the selected verified Bing site. All four Bing report types (queries, pages, crawl and inbound links) completed successfully against the configured live connection. This site currently returned empty samples; the UI correctly retained unknown/absence language instead of claiming measured zeroes.

Browser inspection found four form actions with an implicit Base UI `type="button"`: CrUX lookup, AI Overview capture/import and city demand import. Clicking CrUX produced no submission. All four actions now explicitly use `type="submit"`; frontend type checking, lint and the Docker production build passed. Verification of their corrected runtime behavior is pending recovery of Docker, which stopped answering engine requests while replacing the web container. Restarting Docker Desktop needs operator approval because it interrupts unrelated containers as well.

One live SerpApi Maps playground lookup identified an exact public Jakarta test-business ID and coordinates, as authorized by the operator. This was preparatory provider evidence, not a completed SeoSeo grid scan. No live SeoSeo CrUX, AI Overview or nine-cell Maps acceptance pass is claimed yet. The remaining checklist below still applies.

## Implemented scope

Prior commits delivered KW-11, DO-06, BL-07, RT-07, SA-09/12/13, AI-05 and IN-04. This continuation delivers local scan/heatmap/competitor/category workflows, source-labelled city demand context (KW-10), and Google AI Overview capture/import (AI-06). There are 115 MCP tools and 14 validated portable skills.

Local scans use explicitly enabled SerpApi BYOK; source-labelled evidence imports work without a paid key. City context imports source figures and normalized interest rather than inventing city volumes or bypassing Trends bot checks. AI Overviews are supplied/provider Google search observations, separate from generated model answers. These source and coverage limits remain visible in the product.

## Passed checks

- Rust workspace: **97 tests**, including unique OpenAPI operation IDs, local metric unknown/absence semantics, request budgets and provider parsers.
- Rust formatting and strict all-target workspace Clippy.
- Frontend TypeScript, ESLint and Next.js production build. Base tiles were disabled for offline QA with build-time `NEXT_PUBLIC_MAP_TILE_URL=none`.
- Generated OpenAPI/types; portable skill validation: **14 skills / 115 tools**.
- Compose configuration validation; additive migration 0018 applied on disposable Postgres 18. Current API and worker ran in isolated test containers; the normal stack was untouched.
- `scripts/e2e-m6-observations.py`: import validation, unknown versus measured absence, metric coverage, exact identity, reviewed budgets, active-run guards, nine-cell provider scan, stop after one 429, AIO continuation/absence/cache, safe citations/errors/tokens, city null versus zero, website/tenant isolation, REST/MCP writes and reads, and worker interruption recovery without a duplicate cell request.
- `scripts/e2e-crux.py`: URL/origin/device scope, missing metrics, numeric CLS, dates, cache/no-data cache, cooldowns/concurrency, isolation and sanitized errors.
- `scripts/e2e-bing.py`: encrypted project-bound connection, verified site selection, scoped reports, secret stripping, paging and disconnect.
- `scripts/e2e-local.py`: discovery cache, safe links, validation, atomic keyword saving, isolation, antimeridian wrapping, concurrent tracker limit and MCP geometry preview.

The combined provider fixture is `scripts/m6-observations-fixture.py`; it makes no outbound requests. Fixtures verify application contracts, not real provider account access.

## Reproduction

Use a disposable Postgres 18 database, the combined fixture and the current application binary on an isolated network. Apply migrations before starting the worker. API and worker need a disposable database, encryption key and signup enabled. Point `SERPAPI_URL` to `http://fixture:8080/search` on both services. API fixture settings are `NOMINATIM_URL=http://fixture:8080/nominatim`, `CRUX_API_URL=http://fixture:8080/crux` with a dummy CrUX key, and `BING_WEBMASTER_API_URL=http://fixture:8080/bing`. Publish only loopback ports and use a localhost web origin.

```sh
python scripts/e2e-m6-observations.py http://localhost:3461 seoseo-m6check-finish-db http://localhost:3462 seoseo-m6check-finish-worker
python scripts/e2e-crux.py http://localhost:3461 seoseo-m6check-finish-db seoseo-m6check-finish-fixture
python scripts/e2e-bing.py http://localhost:3461 seoseo-m6check-finish-db
python scripts/e2e-local.py http://localhost:3461
python scripts/validate-agent-skills.py
```

The optional worker-container argument deliberately interrupts and resumes that disposable worker. Never use these scripts against production. SQL-enabled regressions enforce an m6check database-container prefix.

## Open acceptance gates

**Browser QA:** the original host preview was rejected by automatic approval review with “blocked by policy,” and a safer container attempt also failed. The operator later supplied a running updated app, enabling the live checkpoint above. The submit-type fixes need runtime verification after Docker recovers. Remaining checks: discovery/category filtering, tracker creation, imported/live scan selection, point competitors, map display, city imports/history, AIO present/absent/incomplete and CrUX data. Include a narrow viewport and keyboard navigation. Check production tiles through normal interactive use with visible attribution and no prefetch. Do not claim responsive/WebGL/hydration acceptance from the production build alone.

**Real-provider QA:** Bing's selected-site read-only reports passed as described above. Using the now-configured credentials, still confirm a scoped CrUX lookup, a reviewed 3×3 single-keyword Maps scan, and Google AI Overview text/continuation where available. Check coordinate/language/country and exact place ID against returned evidence. Fixture IDs, imports and the preparatory Maps playground lookup do not prove the remaining SeoSeo live flows.

M3 retained follow-ups and M5's explicit live-three-engine gate remain separate and unchanged. M6 stays “implementation delivered; acceptance pending” until these gates are satisfied.
