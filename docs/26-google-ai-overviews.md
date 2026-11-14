# Google AI Overview observations (AI-06)

The **AI visibility → Google AI Overviews** card captures an observed Google search summary and its reported citations. It is separate from generated model answer comparisons: no model creates an overview on Google's behalf.

For live collection, explicitly enable **SerpApi (Maps and AI Overviews)** and save your own key in Settings → Data providers. Keys use the existing encrypted workspace provider store and are never included in responses, jobs or logs. The key is optional: saved search evidence can be imported without a paid key.

Collection uses the project's current country/language, a keyword and optional search location. Review the maximum of two requests: one Google search, followed by at most one Google AI Overview token continuation. Tokens expire quickly and are used immediately, never stored or exposed. Provider continuation URLs are never followed; the fixed operator endpoint is used for both requests. A 24-hour project/website/market/keyword/location cache reuses captured data, and durable per-query cooldowns limit repeated requests. API calls time out, disable redirects, bound streamed responses to 2 MiB and sanitize upstream failures. There are no automatic scheduled requests.

Capture distinguishes:

- **present**: usable returned overview text, with reported citations.
- **absent**: a successful search response did not contain an overview. This is a single observation, not a claim about all users or future searches.
- **incomplete**: an overview envelope exists without usable text, such as an unresolved continuation. Presence/absence remains unknown.

A network/provider error is an error, never observed absence. Source links are limited to HTTP(S) URLs without credentials. They are reported citations, not verified support. Rendering uses plain React text and normal links; no provider HTML is inserted. Only normalized text (up to 20,000 characters), up to 50 citations, source/date and search context are retained; metadata and tokens are discarded. The most recent 100 observations per project/research kind are retained across website identities. History is visible only for the current project website/country/language.

REST under `/api/v1/projects/{project_id}`:

- `GET /ai-overviews`: retained observations in the current scope.
- `POST /ai-overviews`: keyword, optional location, expected_max_requests=2.
- `POST /ai-overviews/import`: keyword, optional location, observed_at, source_note and a SerpApi-format search response.

MCP tools: `capture_google_ai_overview`, `import_google_ai_overview`, `list_google_ai_overviews`. Writes require member role and project access. The UI shows history, provenance, status, citations, cache/request counts and JSON export.

Example evidence import:

```json
{
  "keyword": "what is coffee",
  "observed_at": "2026-11-01T00:00:00Z",
  "source_note": "Saved Google search response from the stated market",
  "response": {"ai_overview": {
    "text_blocks": [{"snippet": "Observed summary text"}],
    "references": [{"title": "Source", "link": "https://example.com/source"}]
  }}
}
```

Official provider contracts: [Google search AI Overview results](https://serpapi.com/ai-overview) and [token continuation API](https://serpapi.com/google-ai-overview-api). Reproducible fixture and browser validation are recorded in [M6 acceptance](27-m6-acceptance.md). Live collection still requires the operator's real provider credentials.
