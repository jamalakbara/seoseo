# Google Search Console

M3 adds a project Search Console tab, a read-only Google OAuth connection, verified property selection, performance explorer, CSV export, URL inspection, and four MCP tools. Google credentials are optional; rank tracking and other features still work without them.

## Operator setup

1. In Google Cloud, enable **Google Search Console API**, configure the OAuth consent screen, and create a **Web application** OAuth client. When using a testing consent screen, add the Google accounts that will connect as test users.
2. Register this exact authorized redirect URI: `<WEB_BASE_URL>/api/v1/gsc/oauth/callback`. For the default local instance, use `http://localhost:3000/api/v1/gsc/oauth/callback`.
3. Set `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, and a stable `APP_ENCRYPTION_KEY` in `.env`. The encryption key is base64 of 32 random bytes; keep a secure backup. Do not replace an existing key: it also encrypts provider credentials.
4. Recreate the application services with the usual Compose command. Open a project → **Search Console** → **Connect Google**, authorize read-only access, then choose **Use property**.

The user must complete real Google consent. The integration tests use a local fixture and do not prove that a particular Google Cloud client or live property is configured correctly.

## Behavior and limits

- Connections belong to a project, with one Google grant and selected property per project. Workspace admins can connect, select a verified property, or disconnect; members can read the selected property's data. An API key can use the GSC tools but cannot start or complete browser OAuth.
- OAuth requests only `webmasters.readonly`, asks for offline access, uses S256 PKCE, and stores a hashed, user/workspace/project-bound state with a 10-minute lifetime. State is consumed once, including declined or failed callbacks. Refresh tokens and PKCE verifiers are AES-GCM encrypted at rest. Access tokens are short-lived and kept only in memory for each operation. HTTP tracing excludes query strings.
- Each operation refreshes access from the stored grant. Revoked grants produce an instruction to reconnect; quota errors do not trigger automatic retries. Each request has a 15-second timeout and bounded response size.
- Disconnect deletes this project's local grant and pending OAuth state. It does not revoke the Google account's entire grant, which could disrupt other connected projects. Reconnecting resets the selected property so it must be selected again.
- Performance supports query/page/country/device/date/searchAppearance dimensions, AND filters, search types, and offset pagination. Dates must be within the last 16 months and exclude the latest three days. Requests return 1–1000 rows, default 100. Dates are interpreted by Google in Pacific Time. Google returns top rows and may omit anonymized queries; grouped rows must not be presented as whole-site totals.
- The UI exports the displayed page, with spreadsheet formula prefixes escaped. The API accepts the same query body at `POST /projects/{project_id}/gsc/performance.csv`; callers can paginate to export more pages.
- Inspection accepts 1–10 HTTP(S) URLs and returns Google's **indexed** version, not a live test or an indexing request. Invalid individual URLs return partial errors; connection/quota failures require correction before retrying. Google enforces whether a URL belongs to the selected domain or URL-prefix property. SeoSeo sends URLs to Google's fixed endpoint and does not fetch them directly.

## REST and MCP

All project routes are under `/api/v1/projects/{project_id}/gsc`:

| Method | Suffix | Purpose |
|---|---|---|
| GET | (none) | Configuration/connection status without secrets |
| POST | `/connect` | Start browser OAuth |
| DELETE | (none) | Disconnect this project |
| GET | `/sites` | List verified properties |
| PUT | `/site` | Select an exact returned `site_url` |
| POST | `/performance` | Query measured traffic |
| POST | `/performance.csv` | Export one query page |
| POST | `/inspect` | Inspect 1–10 URLs |

Callback: `GET /api/v1/gsc/oauth/callback`, using the signed-in user who began the flow.

MCP: `list_search_console_sites`, `set_search_console_site`, `get_search_console_performance`, `inspect_urls`. All take an explicit `project_id` and use the same domain services as REST. Total server inventory: 33 tools.

## Offline verification

Use the existing throwaway `seoseo-ci` project and add `-f deploy/compose.gsc-test.yaml` to the test stack's Compose command. It supplies fake Google credentials, encryption key, endpoint overrides, and a local fixture on `127.0.0.1:3101`. Never use this overlay in production.

```sh
WEB_PORT=3100 WEB_BASE_URL=http://localhost:3100 docker compose -p seoseo-ci \
  -f deploy/compose.yaml -f deploy/compose.test.yaml -f deploy/compose.gsc-test.yaml \
  --env-file .env --profile renderer up -d --build
python scripts/e2e-gsc.py admin@example.com 'correct horse battery' http://127.0.0.1:3100
```

Run `scripts/smoke.sh` first if the test instance has no accounts. Run integration scripts sequentially because provider settings are shared within the test workspace. On Windows, `127.0.0.1` avoids the per-request delay observed with Python's `localhost` connections.

The endpoint overrides (`GOOGLE_AUTH_URL`, `GOOGLE_TOKEN_URL`, `GSC_API_URL`, `GSC_INSPECTION_URL`) are operator environment settings only, never client-supplied URLs. Production Compose keeps Google's defaults; test Compose overrides them explicitly.

References: [Google OAuth web-server flow](https://developers.google.com/identity/protocols/oauth2/web-server), [Search Analytics query](https://developers.google.com/webmaster-tools/v1/searchanalytics/query), [property list](https://developers.google.com/webmaster-tools/v1/sites/list), [URL inspection](https://developers.google.com/webmaster-tools/v1/urlInspection.index/inspect).
