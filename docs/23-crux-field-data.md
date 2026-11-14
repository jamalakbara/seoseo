# CrUX field performance (SA-09)

Implementation checkpoint: REST, MCP and the audit-page card are present. Real-provider and browser acceptance remain pending; no real API key was used.

The server operator can set optional `CRUX_API_KEY` in `.env` and recreate the API service. Enable the Chrome UX Report API for that free Google Cloud key. The key stays server-side. `CRUX_API_URL` is an operator-only fixture/proxy override, outside project input.

Use **Audit → CrUX field performance**, POST `/api/v1/projects/{project_id}/crux`, or MCP `get_crux_field_data`. Input:

```json
{"url":"https://example.com/page","scope":"url","form_factor":"PHONE"}
```

Choose `origin` with a root URL to request site-wide coverage. `PHONE`, `DESKTOP`, `TABLET`, and `ALL` are separate device selections. Results retain requested and returned targets, the collection period and LCP/INP/CLS p75 values. Missing metrics and absent coverage stay unknown. Origin fallback is explicit; these measurements never change Lighthouse or audit health scores. See the [official CrUX API documentation](https://developer.chrome.com/docs/crux/api/).

Project authorization precedes every lookup. Only the current project host or its www equivalent is accepted; credentials, query strings, fragments and custom ports are rejected. The service calls the fixed upstream API, never the supplied website. Public positive/negative records are cached for 24 hours by operator endpoint, scope, target and device. Durable database claims limit uncached requests per project/target to one per minute across API processes. Errors omit upstream response bodies and credential-bearing transport URLs; redirects are disabled, requests time out and response bodies are bounded.

Regression fixture: `deploy/test/m6-field/fixture.py` or the combined `scripts/m6-observations-fixture.py`, API regression: `scripts/e2e-crux.py` (disposable m6check database only). It covers scoping, missing metrics, numeric/string percentiles, periods, cache reuse, no-data caching, concurrency and sanitized errors.

See [M6 acceptance](27-m6-acceptance.md) for current checks. Browser acceptance remains blocked by automatic approval review of the local preview startup; real-provider acceptance still needs an operator CrUX key. No production-preview browser or real-provider acceptance is claimed.
