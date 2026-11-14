# M6 acceptance — 2026-11-14

M6 implementation is delivered on `codex/m6-field-and-bing`. **Full milestone acceptance is still pending.** The original implementation session used offline fixtures because preview execution and credentials were unavailable. The operator has since configured live credentials and started the current app. Live/browser acceptance is now in progress, with the checkpoint below distinguishing verified results from remaining work. No merge or push has been performed.

## Live acceptance checkpoint

On 2026-11-14, the signed-in browser displayed the updated project navigation and the selected verified Bing site. All four Bing report types (queries, pages, crawl and inbound links) completed successfully against the configured live connection. This site currently returned empty samples; the UI correctly retained unknown/absence language instead of claiming measured zeroes.

Browser inspection found four form actions with an implicit Base UI `type="button"`: CrUX lookup, AI Overview capture/import and city demand import. All now explicitly use `type="submit"`, and all were exercised successfully after the operator-approved Docker Desktop restart. The normal application services and the unrelated OpenWA container recovered. Subsequent production web rebuilds completed normally.

Live browser results on 2026-11-14:

- CrUX: explicit origin / ALL-device lookup completed with a valid no-data response. LCP, INP and CLS stayed unknown, with no scope fallback. This verifies provider access and the no-coverage UI, not actual p75 values.
- Google AI Overview: the Indonesian query `apa itu seo` returned overview text and reported citations in one provider request. Repeating the capture reused the cache with zero requests. Live continuation was not returned; continuation remains fixture-tested.
- Maps: a clearly labelled public Jakarta cafe tracker completed a reviewed 3×3, one-keyword scan with nine requests. Coverage was 9/9; the exact target ID was absent from the first 20 returned results at every point. ARP stayed unknown, ATRP was 21 and SoLV was 0%. Competitor samples and keyboard point selection worked.
- OSM discovery returned no matches for the tested public business. A new manual business form allowed verified name/address/coordinates to be saved without an external lookup; manual identity remains separate from the exact Google provider ID.
- Synthetic, explicitly labelled imports verified partial Maps coverage (2/9, ARP 2, ATRP 11.5, SoLV 50%), AI Overview absent/incomplete states, malformed JSON feedback, and city null versus zero with dated source provenance. These are UI fixtures, not live market observations.
- The optional attributed OSM map rendered at a 390-pixel viewport. A MapLibre worker path error found in production was fixed by serving the installed worker/shared modules from the same origin. After rebuilding, tiles/markers rendered with no new worker errors. The numeric grid remained usable by keyboard.

The browser connection failed during the final JSON-download check and could not reconnect. Download completion, the final return to the live scan, and reset of the temporary viewport override are unverified. This is the remaining browser closeout gate; no further paid provider requests are required. No full milestone acceptance is claimed until it is closed.

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

**Browser closeout:** reconnect browser automation, confirm a saved scan/observation JSON download, restore the real completed scan, and clear the temporary viewport override. The preceding live/UI checks passed as recorded above. The browser connection failed while waiting for the download; the downloaded file was not inspected.

**Provider coverage limits:** selected-site Bing reports, scoped CrUX access/no-data, the reviewed nine-cell Maps scan, and inline Google AI Overview capture/cache passed. No real CrUX p75 values or live AIO continuation were available in these samples; their data contracts remain covered by fixtures. Public test-business results and synthetic imports do not describe the project's business rankings or market demand.

M3 retained follow-ups and M5's explicit live-three-engine gate remain separate and unchanged. M6 stays “implementation delivered; acceptance pending” until these gates are satisfied.
