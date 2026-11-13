# Audit comparison and scheduled re-audits (SA-13)

Completed audit results now have a **Compare** tab. Choose another completed audit of the same normalized start URL to inspect observed page and issue differences. The audit history page also has **Scheduled re-audits**, where an admin can review a completed audit's crawl settings and explicitly enable or disable recurring crawls.

## Comparison evidence

`GET /api/v1/projects/{project_id}/audits/{audit_id}/comparison?baseline_id=UUID`

MCP `compare_site_audits` accepts the same IDs plus kind, limit and offset. Both audits must belong to the current workspace/project and be completed. Different audits/start URLs are required. Members can read, including archived projects. Other-project/tenant IDs return 404; incomplete audits return 409; invalid comparison filters/ranges return 422.

One read-only repeatable-read transaction reads both audit records and at most 10,000 stored pages each. URLs are exact stored normalized identities; query strings and schemes remain distinct. Metadata field fingerprints compare full title/description/H1 values rather than truncated previews. Differences include status, fetch class, redirect state, title, meta description, H1, canonicals, indexability, content hash, word count, depth and inlinks where comparable.

The response distinguishes changed, unchanged, only_in_baseline and only_in_current observations. An absent URL is never automatically called deleted. Newly observed issues are distinguished from issues no longer observed on comparable successful HTML pages. If a page was not visited, failed, redirected or used different rendering/Lighthouse settings, absent prior issues are unconfirmed. A failed fetch does not turn its missing title/content into a content change or resolved issue. Unchanged means no comparable observed differences, not proof that the live page is identical.

Whole-audit health/issue totals remain visible in the baseline/current records, with a score delta only if both scores exist. Differences can reflect changed crawl coverage, configuration or analysis rules. Early stops, failed fetches and settings changes have explicit warnings. Comparison does not crawl, mutate data or queue jobs.

Kind defaults to changes (all except unchanged); other values are all, changed, unchanged, only_in_baseline and only_in_current. Limit is 1–500 (default 50), offset defaults to zero. Counts cover the full URL union; total_matching describes the selected filter before pagination. Pages sort by URL. The UI links to each observation's baseline/current page detail.

## Recurring crawls

`GET /api/v1/projects/{project_id}/audit-schedule` returns the current schedule or null.

`PUT /api/v1/projects/{project_id}/audit-schedule` and MCP `set_audit_schedule` require admin access. Supply expected_revision (0 for first configuration), expected_domain, baseline_audit_id, schedule, hour_utc and weekday. MCP `get_audit_schedule` reads the same record. Frequency is manual (disabled), daily, weekly, or monthly on day 1. Hours are UTC 0–23; weekdays are Monday=1 through Sunday=7. Next time is strictly in the future. The UI additionally displays the next occurrence in the viewer's local time.

Enabling requires an active project and a completed baseline auditing the current website host or its www twin. The service validates the URL policy, crawler limits and renderer availability and freezes that baseline's configuration. The form shows start URL, page budget, robots policy, rendering/Lighthouse and exclusions before saving. It does not start an immediate audit. Disabling an existing schedule needs no baseline and remains possible on archived projects. Revision checks prevent silent overwrites; reload after a 409 conflict.

Migration 0015 adds one schedule per project. The existing worker scheduler checks every sixty seconds. Under the same project row lock used by manual audit starts, it validates the due slot/revision and atomically commits the audit, job, next time and latest-audit pointer. Multiple workers cannot enqueue the same occurrence. Missed occurrences are not replayed as a backlog. An already queued/running audit skips the occurrence and advances to the next scheduled time, exposing the reason.

Archives, website identity changes, unavailable required rendering, invalid operator URL policy/settings and 100 retained project audits pause the schedule in manual mode with an explicit error. The cap never deletes existing audits; clean up unneeded history and explicitly re-enable. The last queued audit remains inspectable. The worker checks scheduled website authorization before starting and before each further crawl batch; queued runs for a changed/archived website stop with a recorded error. Requests already in flight can finish. A probe moving outside the reviewed host/www scope does not proceed with a full recurring crawl.

Frozen crawl budgets, robots policy and the existing guarded fetcher apply. Scheduled jobs reuse the normal durable crawler, checkpoints, retries and cancellation. Deleting the baseline audit clears its reference while retaining frozen settings; deleting the project cascades the schedule. Saving a new configuration preserves the historical last-run pointer as the last queued scheduled audit.

## Verification

Unit tests cover comparable versus missing/failed issue observations and changed collection methods. The disposable API regression additionally verifies full-value metadata changes, URL/status/configuration gates, filters/paging, tenant/project/member isolation, schedule revision races, admin controls, two-worker atomic claims, active-run skips, archive/domain/quota/renderer pauses, a queued website guard and a real three-page crawl of the local badseo fixture. All three MCP tools are checked against REST.

`scripts/e2e-audit-comparison.py API_ORIGIN M6CHECK_DB WORKER1 WORKER2` requires two created but stopped disposable worker containers; it starts both. Use a password-auth API with open signup, private crawler access and a dummy renderer URL; workers need private crawler access and no renderer. The local badseo nginx fixture must have the network alias audit-fixture.example. The renderer mismatch exercises safe schedule pausing without any browser rendering request. Container names must start with seoseo-m6check. Never use a normal deployment.

`scripts/e2e-audit-comparison-browser.cjs WEB_ORIGIN M6CHECK_DB OUTPUT_DIRECTORY` uses a temporary puppeteer-core installation through PUPPETEER_MODULE and local Chrome. It verifies baseline/filter controls, unknown absences, settings review, save/disable and desktop/mobile layouts. The test disables its recurring schedule before exiting.

Verified on 2026-11-13: all 86 workspace tests, Rust formatting, workspace/all-targets clippy with warnings denied, app build, frontend typecheck/lint/production build, the disposable API/two-worker/real-crawl regression and desktop/mobile browser checks passed. All 14 portable skills validate against 92 MCP tools. Screenshots were visually reviewed; comparison controls were corrected to keep narrow layouts within their container.

No normal-stack migration, deployment or live-cloud acceptance is implied by disposable tests.
