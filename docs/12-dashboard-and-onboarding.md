# 12 — Dashboard and website onboarding

M4 continuation adds DB-01/03/05 and the available portions of DB-02/WS-11. Migration `0008_dashboard.sql` is additive; never replace the applied `0007_memory_reports.sql` migration.

## Website setup

Creating a project with a website in the UI starts its first audit (50-page cap, robots.txt respected, rendering when configured), then opens `/p/{id}/onboarding`. API/MCP callers explicitly invoke website research. Concurrent requests lock the project and reuse the latest audit for the current domain, including completed/failed audits. Retry a failed audit from Site audit. A project remains saved if the initial request fails.

Setup shows up to five crawled pages and a draft formed from title, description and H1. This is source evidence, not LLM synthesis. The user reviews it, supplies their goal and explicitly saves. Manual entry works when a crawl fails. Saving replaces only `business_overview` and `current_goal`, adds selected evidence URLs, preserves other context/competitors/pages and appends a research note. Revision checks prevent lost updates; a project lock checks the expected domain and archive status at the write. Website text is untrusted input, never agent instructions.

## Dashboard

The overview shows six evidence-derived checklist steps, latest completed site health, running audit progress, saved-keyword/tracker/report counts and up to ten rank movers. Skip choices belong to the current user. Audit health matches the current website; changing domain hides the old site's audit. Rank movements compare adjacent completed measurements only when provider and depth match, excluding unknown positions and trackers for another domain/market.

Search keywords shows top queries, new queries and striking-distance query/page pairs with search, sorting, minimum impressions and 25-row pagination. All data is from Search Console web search; positions are averages, not live ranks. A visit requests a background refresh when data is missing/stale. Dates deliberately exclude four UTC days to avoid incomplete Pacific-time days.

- Top: 28 completed days, sorted by clicks.
- New: at least ten impressions in seven completed days with no returned observations in the preceding 28 days. If the baseline hits the 1,000-row cap, candidates are checked individually before being labeled new (maximum 25 candidates).
- Striking distance: query/page pairs with average position 5–20 inclusive, sorted by impressions.

Each period/view is capped at 1,000 rows. The UI discloses truncation and Google query omissions; these are sampled opportunities, not an exhaustive inventory.

## Refresh lifecycle and isolation

Refresh claims and jobs are created in one transaction under a project lock. Parallel visits reuse the same active job. Success is fresh for 24 hours; errors preserve the previous snapshot and back off 15 minutes. Queued workers resolve membership again. Snapshot identity includes project domain/market, selected property and connection timestamp. Reads and final writes compare identity, preventing old jobs from publishing data after project/property/grant changes. Disconnect immediately hides prior snapshots. No Google grant is required to use the rest of the dashboard.

REST and six MCP tools call the same domain services. Memory/report schemas and tools remain those shipped on the pulled branch; no duplicate document store is introduced. This delivery also fixes existing memory updates after revision 0 and rejects archived-project memory writes.

## Verification

Use a **disposable** Compose project with `compose.yaml`, `compose.test.yaml`, `compose.gsc-test.yaml` and `compose.dashboard-test.yaml`. The last overlay disables rendering to keep the offline onboarding test deterministic. Use password auth, open sign-up, a separate database/volumes, `WEB_BASE_URL=http://127.0.0.1:3200` and `WEB_PORT=3200`; the GSC fixture listens on loopback port 3101. Use separately tagged backend/web images so the normal stack is unaffected.

```powershell
python scripts/e2e-dashboard.py http://127.0.0.1:3200
$env:PLAYWRIGHT_MODULE = "$env:TEMP/seoseo-m4-browser/node_modules/playwright-core"
node scripts/e2e-m4-browser.cjs http://127.0.0.1:3200
```

The API regression covers concurrent audit/refresh claims, evidence review, repeated memory saves, preserved context, stale revisions/domain, cross-tenant access, GSC OAuth fixture, truncated baseline verification, source invalidation, provider quota backoff, MCP reads and archived-memory protection. Browser regression covers project creation through review/save, dashboard/navigation 390-pixel layouts and anonymous report sharing. Standard checks: Rust fmt, workspace tests, strict Clippy, frontend lint/typecheck/build, and Docker image/Compose startup.

## Remaining scope

DB-02 link activity awaits the M5 link store. WS-11 LLM synthesis is deferred; the current flow requires human review. GA4 IN-03, IndexNow IN-05 and MCP OAuth MC-02 remain M4 work. Real Google consent requires operator credentials; offline fixtures do not verify live account access.

Verified 2026-11-12: 58 Rust workspace tests, strict Clippy/fmt, frontend lint/typecheck and production builds, separate backend/web Docker images, fresh Compose migration/startup, dashboard API/MCP regression, existing Memory/Reports and GSC regressions, and Chrome desktop/mobile/anonymous-share checks.
