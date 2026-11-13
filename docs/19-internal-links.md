# Internal link graph and suggestions (SA-12)

Completed site audits now have an **Internal links** tab. It visualizes a selected page's incoming/outgoing neighborhood, exposes full stored-audit link counts, supports URL/title filtering and weak-link filtering, and exports the displayed evidence as JSON. Clicking a graph node or table URL selects the page; a page-detail link opens the existing audit inspection view. The diagram is keyboard accessible and scrolls within its container on smaller screens.

## Evidence and limits

`GET /api/v1/projects/{project_id}/audits/{audit_id}/internal-links?limit=200`

MCP `get_internal_link_graph` accepts project_id, audit_id and optional limit with the same semantics. Members can read, including archived projects. Other projects/tenants receive 404; unauthenticated callers receive 401. Only completed audits qualify (other statuses return 409). Limit is 10–500 (default 200), with invalid ranges returning 422. No migration, worker, provider credentials or fresh crawl is needed. Graph reads do not write or queue jobs.

The repository reads authorized audit metadata, nodes, counts and edges in one repeatable-read, read-only transaction. Pages are sampled alphabetically by URL, with ID as the tie-breaker. total_pages reports the full stored audit; sampled_pages reports the selected subset. Node title/H1 evidence is capped at 300 characters each.

For each selected page, incoming_sources counts distinct other source pages across the full audit. followed_incoming_sources requires at least one observed link without nofollow. outgoing_targets counts distinct non-self destinations; unfetched_targets counts destinations with no matching stored audit page. An unfetched destination is unknown, not necessarily broken. Counts are recomputed from link records rather than copied from potentially incomplete analysis metrics.

Edges connect only selected stored pages and exclude self-links. Duplicate source/target pairs become one edge with occurrences, an anchor sample (up to 200 characters) and followed=true if any observed link for the pair lacks nofollow. Full-audit counts can therefore exceed counts implied by the displayed edges. The response returns at most 5000 edges and exposes total_sample_edges and edges_truncated. Its page sample and edge limits are independent of the UI neighborhood display cap of six neighbors in each direction.

The audit itself is a bounded crawl. Zero observed incoming sources never proves website-wide orphan status. Early stop reasons, failed fetches, omitted sample pages and truncated graph edges have explicit warnings. Dashed edges mean nofollow-only observations, not a claim about how search engines value the link.

## Editorial suggestions

Sources and targets must be indexable HTTP 200 HTML pages without redirects, within the audit start host or its www twin. Both HTML and header canonical declarations, when present, must normalize to the page's own URL. The audited entry URL is not suggested as a target.

Targets have at most one observed followed incoming source across the full stored audit. Candidate pairs must share at least two distinct title/H1 terms (Unicode alphanumeric, at least three characters, excluding a small English stopword list). Existing edges—including nofollow-only pairs—and self-pairs are excluded. Terms are lexical evidence, not semantic intent, and the stopword list is not a multilingual language model.

Targets sort by fewest followed sources then URL; candidates sort by number of shared terms then source URL. Results are capped at three sources per target and fifty suggestions overall. Only selected sample pages participate. If the edge list is truncated, all suggestions are disabled because omitted edges might already connect a proposed pair. No suggested links are inserted into the site or database.

## Verification

Unit tests exercise eligibility, redirects/canonicals/status/indexability/scope, lexical overlap, existing nofollow/self-pair exclusion, entry-URL exclusion and edge-cap behavior. The API regression adds stored fixtures for full-audit counts from outside the sample, duplicates, self-links, unfetched targets, early-stop warnings, empty/noncompleted audits, a 5112-edge dense graph, tenant/project/member/archive isolation, REST/MCP parity and no job creation.

Use a disposable password-auth stack with open signup and a database container named with the seoseo-m6check prefix. These scripts create synthetic users/projects and directly insert fixtures using local psql; never point them at a normal deployment.

```powershell
python scripts/e2e-internal-links.py http://127.0.0.1:3401 seoseo-m6check-db
$env:PUPPETEER_MODULE = 'PATH_TO_TEMP_INSTALL/node_modules/puppeteer-core'
node scripts/e2e-internal-links-browser.cjs http://127.0.0.1:3400 seoseo-m6check-db OUTPUT_DIRECTORY
python scripts/validate-agent-skills.py
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend build
```

The browser regression verifies the audit tab, interactive SVG/table selection, dashed nofollow style, page filter, sample-limit changes, JSON download, suggestions, no new crawl requests, desktop/mobile overflow and absence of browser errors. It saves screenshots for visual review.

Verified on 2026-11-13: all 84 workspace tests, workspace/all-targets clippy with warnings denied, Rust formatting, app build, frontend typecheck/lint/production build, API regression, browser regression and validation of 14 portable skills against 89 MCP tools passed. Desktop and mobile screenshots were visually reviewed. The existing CSV link-button semantics on the touched audit page were corrected.

Audit comparison and scheduled re-audits (SA-13) are documented in [audit comparison and schedules](20-audit-comparison-and-schedules.md). Other M6 features remain open. M5's live-cloud acceptance is unchanged.
