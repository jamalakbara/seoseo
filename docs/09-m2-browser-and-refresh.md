# M2 browser audits and keyword refresh

Enable browser audits by setting `RENDERER_URL=http://renderer:8080` in `.env`, then starting the stack with:

```sh
docker compose -f deploy/compose.yaml --env-file .env --profile renderer up -d --build
```

The audit form offers **Render JavaScript** and **Include Lighthouse lab scores**. Both default off. Rendered audits extract content and discover links from the browser DOM. A failed render produces a `render-failed` issue and retains the static HTML audit. Lighthouse scores, numeric lab metrics, measurement time and improvement suggestions are stored in `audit_pages.browser_result` (migration 0004) and displayed on the page detail screen. They are desktop lab measurements, not field Core Web Vitals. A Lighthouse failure is reported independently of successful DOM rendering.

The renderer runs one browser operation at a time, with a 90-second deadline, isolated browser state per request and a 1 MiB DOM limit. The Rust client waits for a busy renderer for up to roughly two minutes. Large audits with Lighthouse are substantially slower; start with a small page budget. Browser startup failure or an unavailable service is visible in page issues.

## Network boundary

The Chromium container is on an internal Docker network with no external route or published ports. Its HTTP/HTTPS traffic goes through `renderer-proxy`, which validates URL schemes, credentials, ports, every resolved IP, and pins the validated IP when opening the connection. Production only allows public unicast destinations on ports 80/443. Redirects and subresources pass through the same proxy. No browser TLS-verification bypass or stealth modifications are used. Chromium runs as the unprivileged `node` user with container capability drops, no-new-privileges, memory/process limits and fresh browser state; Chromium's own sandbox is disabled for container compatibility.

Do not expose the renderer or proxy ports. `deploy/compose.test.yaml` deliberately enables private destinations for the local fixture and must never be used for a production deployment.

## Extended audit coverage

- Hreflang: malformed codes/URLs, duplicate language entries, missing self-reference, non-indexable crawled targets and missing reciprocal links. In-scope alternates enter the crawl frontier. Out-of-scope or budget-excluded targets remain unknown.
- Soft 404: a short HTTP 200 page with a clear not-found title/H1 is flagged as a **heuristic**, not a confirmed missing page.
- URL hygiene: long URLs, uppercase/underscores in paths and excessive query parameters.
- Assets: up to 20 distinct same-origin image/script/stylesheet URLs per page, subject to robots, delay and SSRF policy. Reports HTTP 4xx/5xx and known sizes above 1 MiB. Blocks/rate limits stop that page's resource checks. Cross-origin and remaining resources are not checked; a missing size header is not an estimated size.

## Opt-in browser SERPs

Settings → Data providers → **Chromium (Bing, opt-in)** enables the adapter for the active workspace. It also requires the renderer service. This is direct Bing organic data, not Google data. It is the final fallback after Serper, Brave and SearXNG. It honors Bing robots.txt, uses an honest user agent, paces direct searches to one per ten seconds and stops on bot challenges; a challenge activates an hour cooldown in the service. Search markup changes, robots exclusions or provider blocks can make this adapter unavailable. During verification on 2026-11-11, Bing robots.txt disallowed search scraping, and the live adapter correctly refused the request; extraction was verified against a controlled browser fixture. No CAPTCHA or bot-check workaround is implemented. Provider-independent shared SERP caching remains seven days; explicit refresh requests fetch anew.

## Saved keyword refresh

**Refresh all** or **Refresh selected** creates a durable `saved_keyword_refresh` job. The selection is frozen at enqueue time, with one active refresh per project. The job checks project ownership, checkpoints every keyword and skips completed writes after a retry. Deleting a selected keyword while queued is safe. Tags and saved-keyword identity remain unchanged.

Refresh updates SERP difficulty, intent and features, plus Wikipedia-based volume/trend where the enabled demand provider has a matching article. The pageview cache retains its 30-day TTL. Missing estimates stay null; popularity remains the relative score of the originating research. Provider errors fail/retry the job and retain already completed progress. The saved-keywords screen shows the latest refresh state and error. Jobs use the existing workspace-scoped job API.

Endpoint: `POST /api/v1/projects/{project_id}/saved-keywords/refresh`, body `{ "ids": [] }` for all or a list of up to 1,000 IDs. Returns a job with HTTP 202. Poll `/api/v1/jobs/{job_id}` for status and `{done,total}` progress.

Google Trends remains deferred. PAA/related-search coverage remains provider-dependent. These limitations are not filled with invented metrics.

## Verification

`scripts/e2e-m2.py` uses the deterministic SERP response from the local badseo fixture. It checks bulk refresh selection, concurrency conflicts, project isolation, null metrics, preserved tags, rendered content and persisted Lighthouse scores. `scripts/e2e-audit.sh` covers the new rule types alongside M1 regressions. `cd renderer && npm test` covers public/private address checks, mixed DNS answers and both HTTP/CONNECT proxy denials.

The Integration checks GitHub workflow runs the isolated fixture stack on pull requests. Its optional `live_keywords` dispatch input also runs `scripts/e2e-keywords.sh` against live free suggestion/demand providers. To use live SearXNG with the test overlay, set `TEST_SEARXNG_URL=http://searxng:8080` and add `--profile serp`; the overlay otherwise uses the deterministic fixture.
