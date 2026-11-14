# M8 â€” Public tools, evidence extras and release packaging

M8 implementation adds eleven public tools at `/tools`, a project **Extras** page, a public documentation site at `/docs`, English/Indonesian navigation, optional diagnostics, and multi-architecture release automation. The MCP server exposes **124 tools** including `show_serp_widget`, `show_gsc_widget` and `get_serp_volatility`.

## Public tools

No account or paid key is needed. `POST /api/v1/public/tools` accepts `{tool, content, url}`; maximum content is 256 KiB. Results carry source and limitations. Submitted content is parsed and discarded, never executed or saved.

| Tool | Behavior and evidence boundary |
|---|---|
| `keywords` | Twelve local seed templates. Ideas are brainstorming prompts; no invented search demand. |
| `serp-preview` | Extract title/description from supplied HTML and display a snippet preview. Engines may rewrite snippets. |
| `backlinks` | Extract absolute links to the target host from one supplied source page. No exhaustive backlink index or live freshness claim. |
| `authority` | Read imported Common Crawl SeoSeo Rank; null outside the loaded sample. SR is not Moz DA or Ahrefs DR. |
| `domain-age` | Existing IANA-discovered guarded RDAP lookup, minimal public registry metadata, daily cache. No contacts retained. |
| `robots` | Test supplied rules for SeoSeoBot against the entered URL. Invalid directives may be ignored by the parser. |
| `sitemap` | Check complete XML syntax, root and absolute public locations. Does not crawl locations or validate the full XSD. |
| `meta` | Extract title, description, canonical and robots from supplied HTML. Counts are editorial hints. |
| `schema` | JSON-LD syntax/basic shape and typed-node checks. Not a full Schema.org vocabulary or rich-result eligibility validator. |
| `redirects` | Explicit bounded live HTTP header check. Guarded DNS, robots rules/delay, five redirects, same-origin only; stop at blocks/challenges. No HTML or JavaScript execution. |
| `hreflang` | Extract alternates, duplicate-language hints and resolvable targets from supplied HTML. Reciprocal links and target indexability remain unverified. |

Two fixed Postgres budget rows are shared across replicas: 120 analysis requests and 10 network requests per minute, across the whole instance. No IP fingerprints are stored. Failed validation consumes its budget. Operators exposing these tools publicly should also apply their normal ingress limits.

## Log analysis

`/projects/{project_id}/log-analyses` supports list/create and `/{analysis_id}` deletion. Members read; admins create/delete. Accept Apache/Nginx common or combined format, at most 1 MiB and 20,000 lines. Unsupported/malformed lines are counted as rejected, never guessed. Response bytes of `-` remain unknown.

Only aggregates persist: status counts, recognized self-declared bot-family counts, accepted/rejected lines, known response bytes and unknown-byte rows. Raw logs, IPs, accounts, request paths, query strings, referrers and raw user agents are discarded. Bot names are not verified bot identities. A project lock enforces a rolling maximum of 20 summaries. Exports contain the same aggregates.

## Volatility

`GET /projects/{project_id}/volatility` and `get_serp_volatility` report two distinct measures:

- `index`: mean percentage of replaced top-ten URLs for up to 500 saved keywords in the current project market. Use the latest exact-engine snapshot and its latest same-provider/exactness baseline; current snapshot age and pair gap are limited to 30 days. Both lists must contain ten distinct URLs. Twenty percent means two of ten URLs changed, irrespective of order. This is URL churn, not a Google algorithm-update detector.
- `rank_movement_index`: mean normalized absolute tracked position movement in the latest compatible completed run pair per current-market tracker. Provider, exactness and depth must agree. Measured absence is depth + 1; unchecked rows are excluded.

No evidence means null. Snapshot IDs, coverage and per-keyword churn are exposed for review. Changing the project's market/website excludes unrelated trackers. These are samples of the instance's observations, never a web-wide volatility claim.

## Signed merchant crawler access

Project admins can save/remove the three Web Bot Auth headers on Extras, or use `/projects/{project_id}/crawler-credential`. Use the values generated in **Shopify Admin â†’ Online Store â†’ Preferences â†’ Crawler access**, including the quoted `Signature-Agent` value. Provide the store signature's actual expiry.

The exact credential host must match the project website. Values are single-line and size-bounded; the signature input must cover authority and signature agent and identify Web Bot Auth. Values are AES-GCM encrypted using `APP_ENCRYPTION_KEY`; GET returns only configured/host/expiry. The worker loads credentials per audit and attaches them only to that exact HTTPS host on port 443, before expiry. Other hosts, subdomains, HTTP downgrades and custom ports never receive them. Changing the project website disables old-host use. Removing a credential prevents new audits from loading it; an already-running audit retains its loaded credential until expiry or completion. Stop that audit for immediate removal from active use.

Robots rules, honest user agent and existing block/rate-limit behavior remain in force. The renderer never receives signed headers; this delivery covers HTTP crawling, not authenticated browser rendering. Shopify verifies the actual signature; saving headers locally does not establish provider acceptance.

Primary provider instructions: [Shopify crawling your store](https://help.shopify.com/en/manual/promoting-marketing/seo/crawling-your-store).

## Google Ads CPC (explicit BYOK)

Extras calls `POST /projects/{project_id}/keyword-cpc` only on an administrator's explicit action. Pass 1â€“50 keywords, Google geo target/language constant IDs and expected account currency. The service refreshes an operator-supplied token, reads the actual account currency, then requests historical metrics from fixed Google endpoints. Currency mismatches stop collection. Int64 micros/search counts are parsed without manufacturing zeros; close variants can share a result. Advertising competition is separate from organic keyword difficulty.

Set operator configuration on the API service:

```text
GOOGLE_CLIENT_ID=...
GOOGLE_CLIENT_SECRET=...
GOOGLE_ADS_DEVELOPER_TOKEN=...
GOOGLE_ADS_CUSTOMER_ID=1234567890
GOOGLE_ADS_LOGIN_CUSTOMER_ID=... # optional manager account, digits only
GOOGLE_ADS_REFRESH_TOKEN=...     # OAuth grant with adwords scope
GOOGLE_ADS_API_VERSION=v25
```

These are environment secrets, not UI-visible provider keys. The Ads refresh grant is distinct from saved per-project Search Console grants. No Ads credentials means `available:false`, with null CPC/competition/search counts. Errors contain sanitized HTTP status, not provider response bodies or secrets. Results are returned for review/download and not persisted as general SEO metrics.

Primary API contract: [Google historical metrics](https://developers.google.com/google-ads/api/docs/keyword-planning/generate-historical-metrics).

## MCP Apps

Both widget tools declare `_meta.ui.resourceUri = ui://seoseo/evidence.html`. Authenticated `resources/list` and `resources/read` return a static `text/html;profile=mcp-app` resource. The server advertises the UI extension. SERP widgets use the existing cache-first service and project market; GSC widgets read at most 100 query rows from the selected project property. Existing domain authorization and provider requirements apply.

Every successful widget call returns both structured content and meaningful JSON text. Hosts without Apps support can consume the JSON. The resource speaks the `ui/initialize` / initialized / tool-result protocol through the parent bridge, checks message source, pins the host origin after handshake, filters/reverses the displayed sample locally, and renders values with `textContent`. It requests no network domains or extra permissions. No browser credentials or project data are baked into the HTML.

Protocol: [MCP Apps specification](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx).

## Language and documentation

The English/Indonesian picker persists a device-local preference and updates document language. Navigation, project tabs, tool names and core tool/action labels use the dictionary. Number/date formatting uses the selected locale. Legacy feature prose, provider errors and documentation remain English; this is localization infrastructure and a scoped first dictionary, not a claim that every screen is translated. No new translation library or URL routing scheme is required to extend the dictionary.

`python scripts/build-docs.py` packages canonical `docs/*.md` into the checked-in web bundle. `/docs` lists all guides; `/docs/{filename-without-md}` renders text, headings, tables, code and safe links without executing Markdown HTML. CI runs `--check` to prevent stale documentation. The bundle is self-contained for the existing frontend Docker context.

## Optional diagnostics

`/settings/diagnostics` is restricted to instance administrators using a browser session. Diagnostics are off by default and never scheduled. An administrator can opt in, review the exact payload and send once to an operator-configured public HTTPS `TELEMETRY_URL`. Payload fields are only `schema`, `product` and `version`. The receiver also sees the server IP. No built-in receiver, instance identifier, usage counts, project/account data or automatic retries are included. Turning off prevents subsequent sends. Normal Prometheus operational metrics are independent.

## Release packaging and gates

`.github/workflows/release.yml` reuses the complete CI workflow before publishing backend/frontend images to `ghcr.io/<owner>/<repository>-{backend,frontend}:<tag>`. It runs on version tags or manual dispatch, validates semantic version-shaped tags, builds `linux/amd64` and `linux/arm64`, and includes provenance and SBOM attestations. Renderer is an optional separately built profile and is not included in the two published core image tags. The registry token is GitHub Actions' package-scoped token, never a local checked-in credential.

After publication, use the existing `SEOSEO_BACKEND_IMAGE` and `SEOSEO_WEB_IMAGE` variables and start Compose without `--build`. Preserve the current images and database backup before upgrading. Migration 0022 is additive; do not modify previously applied migrations.

**Release publication is still open:** no v1.0 tag, GitHub release, GHCR publish or ARM64 image run was performed in this local delivery. A workflow file is not proof of published images. M5's three-live-engine gate and M6's final export gate remain separately documented in the roadmap; M8 does not silently close them. Live Google Ads and merchant Shopify acceptance require their respective credentials and are not established by local checks.

## Acceptance (2026-11-15)

- Rust formatting, Clippy with warnings denied, and all 113 workspace unit tests plus doc tests pass. New tests cover exact-host/HTTPS/port/expiry credential isolation, parser bounds, private targets, PII-free log aggregation, measured absence, complete-list SERP churn and CPC null/close-variant decoding.
- `scripts/e2e-m8.py` passed on `deploy/compose.m8check.yaml`: anonymous tools, private-network denial, input limits, tenant/role/CSRF enforcement, 20-summary retention, deletion, encrypted/redacted/revocable credentials, known 20-percent SERP churn, incomplete-list unknowns, unconfigured CPC, diagnostics defaults/admin policy, and 124-tool MCP resource/structured/JSON behavior.
- `scripts/e2e-m8-browser.cjs` passed on desktop and 390px mobile: anonymous tools and snippet preview, public docs/tables, log submission, CPC unknowns, persistent language selection, no page errors/overflow, and sandbox-host widget handshake/filter/XSS isolation. Screenshots were visually inspected.
- Live public checks returned measured registration metadata from Verisign RDAP for example.com and a robots-aware HTTP 200 redirect-tool result for https://example.com/.
- Frontend lint, typecheck, optimized build and Docker web build pass. The disposable Compose API/worker/web/database/cache stack starts successfully, including migration 0022. The normal instance and its data were not upgraded.

Reproduce after building the debug app in `seoseo-target`:

```bash
docker compose -f deploy/compose.m8check.yaml up -d --build
python scripts/e2e-m8.py
npm ci --prefix renderer
node scripts/e2e-m8-browser.cjs http://localhost:3600 /absolute/path/to/screenshots
python scripts/build-docs.py --check
```

The browser script uses installed Chrome on Windows by default; set `CHROME_PATH` for other environments. The stack is deliberately isolated and contains only disposable test accounts. No tests send paid requests or actual telemetry.
