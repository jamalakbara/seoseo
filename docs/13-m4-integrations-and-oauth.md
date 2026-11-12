# M4 integrations, model drafts and MCP OAuth

This delivery completes the remaining M4 feature catalog work: GA4 IN-03, IndexNow IN-05, MCP OAuth MC-02, optional model synthesis for WS-11 and recorded link activity for DB-02. Migrations `0009_m4_integrations.sql` and `0010_mcp_oauth.sql` are additive. Existing migrations and project memory/report contracts stay intact.

## Google Analytics 4

Open a project's **Integrations** tab. Configure `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` and the existing stable `APP_ENCRYPTION_KEY`. Enable the Google Analytics Admin and Data APIs in the operator's Google Cloud project. Register `{WEB_BASE_URL}/api/v1/ga4/oauth/callback` on the OAuth client, in addition to the existing Search Console callback.

Analytics uses its own read-only `analytics.readonly` grant, encrypted per project. It does not add scopes to Search Console, and disconnecting either integration does not disconnect the other. Browser-session admins connect and choose properties; current project members can read reports. Consent state is short-lived, single-use and bound to the initiating user/workspace/project. Refresh-token rotation updates only the grant originally read, preserving reconnects and existing tokens when Google omits a replacement.

Property discovery paginates `accountSummaries.list`. Selection verifies the exact numeric `properties/{id}` against fresh discovery (up to 20 pages). Reports cannot supply arbitrary properties, dimensions, metrics or provider URLs.

Nine fixed REST/MCP reports are available: organic overview, organic landing pages, page performance, traffic acquisition, key events, ecommerce performance, site search, audience breakdown and measurement health. Organic overview/landing pages/page performance/key events/ecommerce filter to the session Organic Search channel. Acquisition, audience and measurement health cover all channels; site search filters the `view_search_results` event. Every response identifies the channel.

Reports require explicit ordered dates, at most 366 days, ending at least two UTC days ago, and at most 1,000 rows per page (UI pages are 100 rows). This conservative lag excludes incomplete property days. Google metadata retains the property's reporting time zone/currency, privacy thresholding, row count and quota. Restricted metric values become null. Omitted/thresholded rows are not synthesized as zero. GA4 counts are not interchangeable with Search Console attribution. Measurement health summarizes received events; it does not prove a tag is correctly installed.

## IndexNow

An admin creates/reuses one public ownership key per current project domain and downloads `{key}.txt` from Integrations. Place that file at the root of each submitted host. A domain change rotates the local key; it cannot silently reuse a previously configured site's key.

Verification uses the crawler's guarded DNS resolver, a 1 KB body limit and no redirects. The response must be HTTP 200 with exactly the key (surrounding whitespace allowed). Missing, redirected or inconclusive ownership stops submission for that host. Production checks use HTTPS; the private-crawler test mode permits HTTP fixtures. Engine endpoints are operator configuration only (`INDEXNOW_URL`, default the protocol endpoint), never project input.

Submit 1–1,000 changed URLs across at most ten project-domain/subdomain hosts. Credentials and non-default ports are rejected. URLs are deduplicated, grouped by host and sent with the matching root keyLocation. A database claim enforces a one-minute project cooldown and rejects concurrent or stale-domain submissions. Host requests run with bounded concurrency/timeouts. HTTP 200 means received; 202 means engine ownership validation is pending. Neither guarantees indexing. No automatic resubmission of audit pages is enabled: automatic change detection belongs to later audit comparison work.

## Optional website synthesis

Set `OPENAI_COMPAT_BASE_URL` and `LLM_MODEL`; optionally set `OPENAI_COMPAT_API_KEY`. For local Ollama in Docker, an example base is `http://host.docker.internal:11434/v1` with an installed compatible model. Operator-controlled endpoints may be internal; projects cannot set them. With no endpoint, manual onboarding remains fully usable.

**Generate model draft** sends up to five bounded crawl-evidence records to a chat-completions-compatible model. No call happens until explicitly requested. Website content is labeled untrusted data, and the model receives no tools. JSON output is size/field validated. The draft shows an overview, suggested goal and caveat. Applying it edits the form only; memory changes require the existing explicit review/save action with revision and expected-domain checks. Model errors and invalid JSON never write memory.

## Recorded link activity

Users/agents can record source/target pairs with an observed live/lost state. Targets must belong to the project's domain. Only admins write; members read. The project caps observations at 10,000 and reads return the latest 100; the dashboard displays the latest five. Repeating an identical state preserves the original observation time. Updating live/lost changes the recorded state/time. Current-domain reads hide old website observations, and writes compare the expected domain under the project lock. This is user-provided evidence, clearly labeled in the UI; it does not claim an exhaustive backlink index or independently verified source status. Automatic backlink discovery and new/lost/live backlink analysis remain M5 scope.

## MCP OAuth

Existing `seo_` API keys continue to work. OAuth protects only the canonical `{WEB_BASE_URL}/mcp` resource; OAuth access tokens cannot authenticate REST endpoints. Browser OAuth/MCP clients can use CORS in password mode, but cross-origin MCP calls must present a bearer; cookie-only requests from another origin are refused. Authenticated REST routes keep the app-origin-only CORS policy. No-auth mode does not enable cross-origin MCP. Use HTTPS and secure cookies for a public deployment. OAuth consent requires password-authenticated browser sessions; trusted-network no-auth mode does not issue OAuth grants.

Discovery endpoints:

- `/.well-known/oauth-protected-resource` and `/.well-known/oauth-protected-resource/mcp`
- `/.well-known/oauth-authorization-server`

Public-client DCR is `POST /oauth/register` with exact HTTPS or loopback redirect URIs, `token_endpoint_auth_method: none`, and authorization-code/refresh-token grants. Registration is capped at 100/hour and 10,000 clients per instance, under a database lock. No remote client metadata or logo URL is fetched. Client-ID metadata documents and confidential clients are not supported in this initial DCR implementation.

`GET /oauth/authorize` validates the client, exact registered redirect, canonical resource, `scope=mcp`, `response_type=code` and S256 PKCE challenge before showing consent. Invalid redirects are never used for error redirects. The consent screen names the app, return address, resource and currently selected workspace. Approval permits MCP actions with the user's current role in that workspace. Denial returns `access_denied` and the original state. The authenticated consent POST requires CSRF protection and cannot use API-key/OAuth credentials.

`POST /oauth/token` exchanges a five-minute single-use code and valid verifier, or rotates a refresh token. Access tokens expire in one hour, refresh tokens in 30 days, and grants have a 90-day maximum lifetime. Bearer/code/refresh secrets are stored only as hashes. Tokens are resource- and client-bound. Code replay revokes the issued grant; refresh-token replay revokes the whole family. Clients must serialize refresh requests. Membership/role are resolved on every MCP call, so removal immediately stops access and downgrades do not retain old privileges.

Revoke through `POST /oauth/revoke` or **Connected apps** in Settings. A user sees/revokes only their own grants in the current workspace. Token and registration responses use no-store headers; traces use route templates, not OAuth query strings or bearer secrets. Storage contains a bounded number of pending consent codes per user. Operators should additionally apply their normal ingress rate limits to public endpoints.

## API and MCP

REST includes `/projects/{id}/ga4`, `/ga4/connect`, `/ga4/properties`, `/ga4/property`, `/ga4/reports/{kind}`, `/indexnow/{setup,verify,submit}`, `/links` and `/website-synthesis`. Google callback is `/api/v1/ga4/oauth/callback`. Workspace consent/grant endpoints are under `/api/v1/oauth/`; standard public OAuth endpoints are outside the API prefix. Generated OpenAPI covers authenticated feature routes; public OAuth metadata describes standard protocol routes.

17 added MCP tools (66 total): property list/selection, nine fixed Analytics reports, IndexNow setup/verification/submission, website synthesis, and link observation list/save. They call the same domain services as REST.

## Verification

Use a disposable Compose project with the base, test, GSC-test, dashboard-test and `compose.m4-final-test.yaml` overlays. Password auth/open signup, separate database/volumes and separately tagged images are required. The final overlay uses local fixtures for Analytics, IndexNow and a model; no real Google, indexing or paid model actions are performed. Set `WEB_BASE_URL=http://127.0.0.1:3300` and `WEB_PORT=3300`; the existing fixture origin is loopback port 3101.

```powershell
python scripts/e2e-m4-final.py http://127.0.0.1:3300
$env:PLAYWRIGHT_MODULE = "$env:TEMP/seoseo-m4-browser/node_modules/playwright-core"
node scripts/e2e-m4-final-browser.cjs http://127.0.0.1:3300
```

Keep existing dashboard, GSC and Memory/Reports regressions green. Live Google credentials/consent, real engine acceptance and a real local model remain operator verification. Fixture success does not prove access to a production account.

Protocol references: [GA4 report basics](https://developers.google.com/analytics/devguides/reporting/data/v1/basics), [GA4 schema](https://developers.google.com/analytics/devguides/reporting/data/v1/api-schema), [IndexNow protocol](https://www.indexnow.org/documentation), [MCP authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization).

Validation on 2026-11-12: 61 Rust workspace tests, strict Clippy/fmt, frontend lint/typecheck/production build, separate Docker images and fresh Compose migrations. API/MCP fixtures cover nine GA4 reports, model failure/no implicit saves, IndexNow ownership/cooldown, live member-role enforcement, OAuth PKCE/resource/CSRF/rotation/replay/revocation and CORS boundaries. Chrome covers model review/save, GA4 connect/nine reports, IndexNow, recorded links, consent/exchange/revoke and mobile layouts. Existing dashboard, GSC and Memory/Reports regressions remain green.
