# SERP keyword clustering (KW-11)

The project's **Keywords** page offers **Group keywords into page topics**, an adjustable overlap threshold and CSV export. This is a read-only planning aid; grouping makes no external requests, stores no groups and changes no keywords or page assignments. Existing research and explicit SERP analyses populate the shared search cache.

## Evidence and grouping

The project scope is the union of saved keywords, research ideas, rank-tracker keywords and imported competitive-volume keywords in one country/language. It is deduplicated and bounded to the first 500 keywords alphabetically; the response reports the full scope size and sampled size. Another project's keyword list never enters this scope. Public SERP snapshots remain instance-wide shared cache data.

The newest snapshot per keyword/market is used. Only snapshots newer than seven days and distinct HTTP(S) URLs in positions 1–10 are eligible. Empty latest snapshots replace older evidence. URL parsing normalizes hosts/default ports and removes fragments; query strings, scheme, path case and trailing slash differences are preserved. Duplicate URLs do not increase overlap. Missing, stale/future and insufficient-result evidence is exposed separately, never interpreted as distinct search intent.

Every pair in a group must have the same provider and exactness flag and share at least min_shared URLs (2–10, default 3). Keywords are processed alphabetically and join the first compatible group. This deterministic complete-link rule prevents A/B and B/C overlap from implying an unsupported A/C match. It is conservative and order-defined, rather than an optimal partition or a claim of semantic identity. The representative is alphabetical, not chosen by volume or business importance. A group's minimum overlap is calculated over all member pairs. Ungrouped keywords had enough evidence but did not join a group at the selected threshold.

Each eligible keyword includes provider, exactness, snapshot time and the URL set used. Review groups and query differences before mapping to pages. Non-exact provider samples are clearly labelled aggregated results in the UI; they do not establish Google rankings.

## REST and MCP

`GET /api/v1/projects/{project_id}/keyword-clusters?country=US&language=en&min_shared=3`

Country and language default to the project. Normal project authorization applies: members can read, including archived projects; other tenants receive 404. Invalid market codes or an out-of-range threshold receive 422. The endpoint does not require configured provider credentials. No migration or worker is needed.

MCP `cluster_keywords` accepts project_id and the same optional market/threshold fields and calls the same service. The server now exposes 88 tools. The bundled clustering workflow uses this tool before recommending any additional SERP checks.

The response contains clusters, ungrouped, unavailable, scope/sample/eligible counts and warnings. CSV export uses the displayed response, includes grouped/ungrouped/unavailable keywords, provider, exactness, timestamp, market and minimum pair overlap. Formula-like cells are escaped. Opening an explicit keyword SERP analysis can make a provider request; closing it refreshes the clustering view.

## Verification

Backend tests cover complete-link chain prevention, deterministic ordering, provider/exactness separation, stale evidence, top-ten bounds, deduplication, conservative URL identity and empty scope. End-to-end fixtures additionally cover saved/research/rank/import scopes, latest empty snapshots, market/project/tenant isolation, member access, archived reads, threshold validation, the 500-keyword cap and REST/MCP parity.

Use a disposable password-auth stack with open signup and a DB container named with the `seoseo-m6check` prefix. The scripts create synthetic users/projects and directly insert fixtures with local psql; never target a normal deployment.

```powershell
python scripts/e2e-clustering.py http://127.0.0.1:3401 seoseo-m6check-db
$env:PUPPETEER_MODULE = 'PATH_TO_TEMP_INSTALL/node_modules/puppeteer-core'
node scripts/e2e-clustering-browser.cjs http://127.0.0.1:3400 seoseo-m6check-db OUTPUT_DIRECTORY
python scripts/validate-agent-skills.py
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend build
```

The browser regression exercises grouping, threshold changes, coverage/missing evidence, CSV download and formula protection, desktop/mobile overflow and absence of SERP requests while grouping. It saves desktop/mobile PNGs. Backend verification uses single-job Docker cargo fmt, workspace clippy with warnings denied, workspace tests and an app build.

Verified on 2026-11-13: all 80 workspace tests, fmt, workspace/all-targets clippy with warnings denied, app build, frontend typecheck/lint/production build, the API and browser regressions above, and all 14 portable skills against 88 MCP tools passed. Desktop and mobile screenshots were visually reviewed. An existing button/link semantics warning on the Keywords page was also corrected.

This delivers KW-11 only. The other M6 features and the credential-dependent M5 live-cloud acceptance remain open.
