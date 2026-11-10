# 04 — Technical Architecture (Rust + Next.js + Docker)

Versions verified 2026-11-10 (npm registry, crates.io, Docker Hub). **Re-verify at scaffold time** (use context7 / `cargo search` / `npm view`); always take latest stable.

## 1. Stack

### Backend (Rust)
| Concern | Choice | Version |
|---|---|---|
| Toolchain | Rust stable, edition 2024 | 1.99.x |
| Async runtime | tokio | 1.54 |
| HTTP framework | axum (+ tower, tower-http: cors, compression, trace, timeout, limit) | 0.8.9 / tower-http 0.7 |
| DB | PostgreSQL | 18.x |
| DB access | sqlx (compile-time checked queries, migrations) | 0.9 |
| Cache / rate-limit / pubsub | Valkey (Redis-compatible) via `redis` crate | Valkey 9.x / redis 1.8 |
| Job queue | Postgres-backed queue (`apalis` 1.0 with postgres backend, or hand-rolled `SELECT … FOR UPDATE SKIP LOCKED`) | 1.0 |
| Scheduler | cron-like table polled by worker (tz-aware via `chrono-tz`) | — |
| HTTP client | reqwest (rustls) | 0.13 |
| HTML parsing | `lol_html` (streaming, low memory) for crawl extraction; `scraper` for SERP parsing | 3.0 / 0.29 |
| robots.txt | `texting_robots` | 0.2 |
| Sitemaps | `quick-xml` | latest |
| URL/domain | `url`, `publicsuffix`/`psl`, `idna` | latest |
| Headless browser | `chromiumoxide` → `renderer` container (Chromium) | 0.9 |
| MCP | `rmcp` (official Rust MCP SDK, streamable HTTP server) | 3.9 |
| OpenAPI | `utoipa` + `utoipa-swagger-ui`/scalar | 6.0 |
| Auth | `argon2` (passwords), `jsonwebtoken` (OAuth/MCP tokens), `oauth2` crate (Google), cookie sessions in Valkey | 0.6 / 11 |
| Crypto | `aes-gcm` (encrypt OAuth refresh tokens), `rand` | latest |
| Serialization | serde, serde_json | 1.0 |
| Validation | `validator` / `garde` | latest |
| Errors | `thiserror` (lib), `anyhow` (bins) | latest |
| Observability | tracing, tracing-subscriber (JSON), `metrics` + prometheus exporter | 0.1 |
| Config | `figment` or `config` from env | latest |
| Testing | `cargo nextest`, `sqlx::test`, `wiremock`, `insta` snapshots | latest |
| LLM | `async-openai`-compatible client (works with OpenAI, OpenRouter, Ollama, vLLM), Anthropic + Gemini adapters | latest |

### Frontend (Next.js)
| Concern | Choice | Version |
|---|---|---|
| Framework | Next.js App Router, React Server Components, Turbopack | 16.4 |
| React | React | 19.3 |
| Language | TypeScript | 7.0 |
| Styling | Tailwind CSS v4 + shadcn/ui | 4.3 / shadcn 4.23 |
| Server state | TanStack Query | 5.107 |
| Tables | TanStack Table (virtualized) | 8.x |
| Forms | react-hook-form or TanStack Form + Zod 4 | zod 4.6 |
| Charts | Recharts (or ECharts for heavy) | latest |
| Maps | MapLibre GL + OSM / PMTiles | latest |
| API client | generated from OpenAPI (`openapi-typescript` + `openapi-fetch`) | latest |
| Auth | httpOnly session cookie set by Rust API; Next middleware checks session via API | — |
| Testing | Vitest, Playwright | latest |
| Package manager | pnpm | latest |
| Runtime image | Node LTS (26.x) `output: "standalone"` | — |

## 2. System diagram

```
             ┌──────────────┐        ┌───────────────┐
 Browser ───▶│  web (Next)  │──REST─▶│   api (axum)  │◀── MCP clients (Claude, ChatGPT, Codex, Cursor)
             └──────────────┘        │  /api/v1/*    │      /mcp  (streamable HTTP, OAuth2.1 / API key)
                                     │  /mcp         │
                                     └──────┬────────┘
                         enqueue jobs       │  read/write
                ┌───────────────────────────┼─────────────────────────┐
                ▼                           ▼                         ▼
        ┌──────────────┐           ┌──────────────┐           ┌──────────────┐
        │ worker (Rust)│──────────▶│ Postgres 18  │◀──────────│ Valkey       │
        │ crawler, SERP│           │ (data, queue)│           │ cache, locks,│
        │ rank, AI, CC │           └──────────────┘           │ rate limits, │
        └──┬───┬───┬───┘                                      │ SSE pubsub   │
           │   │   │                                          └──────────────┘
           │   │   └──▶ renderer (Chromium + Lighthouse, CDP)
           │   └──────▶ searxng (optional SERP meta-search)
           └──────────▶ external free APIs (GSC, GA4, PSI, CrUX, Trends, autocomplete,
                         Common Crawl, RDAP, IndexNow, Bing WMT, LLM providers / ollama)
```

`api` and `worker` are the **same Rust binary** with different subcommands (`seoseo serve`, `seoseo worker`, `seoseo migrate`, `seoseo admin …`) — one image, simpler ops.

## 3. Repository layout (planned)

```
seoseo/
├─ CLAUDE.md / AGENTS.md          # agent context (aimem-managed block)
├─ .aimem/                        # project notebook (decisions, memories, handoffs)
├─ docs/                          # this documentation set
├─ reference/open-seo/            # cloned reference (git-ignored)
├─ backend/
│  ├─ Cargo.toml                  # workspace
│  ├─ crates/
│  │  ├─ app/          (bin)      # main: serve | worker | migrate | admin
│  │  ├─ api/                     # axum routers, extractors, auth middleware, OpenAPI
│  │  ├─ mcp/                     # rmcp tool definitions → call domain services
│  │  ├─ domain/                  # services: projects, keywords, rank, audit, links, local, ai, reports, gsc, ga4
│  │  ├─ db/                      # sqlx repositories, migrations/
│  │  ├─ jobs/                    # queue, scheduler, job handlers (audit, rank, local, ai, cc-ingest, refresh)
│  │  ├─ crawler/                 # fetcher, url-policy (SSRF), robots, sitemaps, throttle, analyzer, issue rules
│  │  ├─ providers/               # trait-based adapters: serp/, keywords/, trends/, links/, authority/, llm/, places/, lighthouse/, gsc/, ga4/, bing/, indexnow/, rdap/
│  │  ├─ metrics/                 # KD, SR, traffic est., spam score, intent, clustering
│  │  └─ common/                  # config, errors, ids, telemetry, crypto
│  └─ Dockerfile
├─ frontend/
│  ├─ app/(auth)/ (app)/p/[projectId]/{dashboard,keywords,saved,domain,backlinks,rank-tracking,audit,local,ai-visibility,search-performance,analytics,reports,assistant,context,settings}
│  ├─ components/ui (shadcn) · features/* · lib/api (generated)
│  └─ Dockerfile
├─ skills/                        # agent skills (SKILL.md) ported from OpenSEO
├─ deploy/
│  ├─ compose.yaml                # production
│  ├─ compose.dev.yaml            # dev overrides (mailpit, hot reload)
│  ├─ searxng/settings.yml
│  └─ renderer/Dockerfile
└─ .env.example
```

Layering rule (from OpenSEO, keep it): **router/MCP tool → service (domain) → repository (db)**. Providers are only called from services. Every repository function takes `OrgId`/`ProjectId` explicitly.

## 4. Data model (Postgres)
Port OpenSEO tables (see `01` §4) with these changes:
- Single dialect (Postgres): use `uuid` (v7) PKs, `timestamptz`, `jsonb`, enums as `text` + CHECK, `citext` for emails/domains.
- Drop: billing tables, guest accounts, telemetry tables (replace with `instance_settings`).
- Add:
  - `provider_settings(org_id, provider, enabled, config jsonb, secret_enc bytea)`
  - `jobs(id, kind, payload jsonb, status, attempts, run_at, locked_by, locked_until, last_error, progress jsonb)` + `schedules(id, kind, target_id, interval, hour, minute, weekday, tz, next_run_at)`
  - `serp_results(id, engine, keyword, location, language, device, fetched_at, raw_hash)` + `serp_items(result_id, position, type, url, domain, title, snippet, features)` — instance-wide SERP index (powers domain overview, KD, competitors).
  - `domains(domain citext pk, sr_score, cc_harmonic, cc_pagerank, tranco_rank, first_seen, rdap_created_at, tech jsonb, spam_score)`
  - `links(src_url, src_domain, dst_url, dst_domain, anchor, rel, first_seen, last_seen, source enum{cc,crawl,bing,import})` partitioned by `dst_domain` hash.
  - `keyword_cache(keyword, location, language, est_volume, volume_bucket, confidence, trend jsonb, kd, intent, features, updated_at)`
  - `audit_links(audit_id, from_page, to_url, anchor, rel, status)` for link graph & broken links.
  - `alerts`, `webhooks`, `notification_channels`.
- All project-scoped tables: FK `project_id → projects ON DELETE CASCADE`, index `(project_id, …)`.

## 5. Job system
- Kinds: `site_audit`, `audit_lighthouse`, `rank_check`, `local_scan`, `ai_visibility_run`, `keyword_refresh`, `backlink_snapshot`, `cc_domain_ingest`, `cc_graph_import`, `gsc_refresh`, `report_digest`, `indexnow_submit`.
- Postgres queue with `FOR UPDATE SKIP LOCKED`, lease + heartbeat, exponential retry, max attempts, dead-letter status `failed` with reason. Watchdog marks expired leases.
- Long jobs are **phased & checkpointed** (audit: probe → discover → crawl → render → analyze → lighthouse → finalize), state in DB so restarts resume (OpenSEO spec 0009 lesson).
- Progress events → Valkey pubsub → SSE endpoint `/api/v1/jobs/{id}/events` for live UI.
- Per-host politeness: token bucket in Valkey keyed by host; global concurrency limits per provider.

## 6. Crawler design
- Fetcher: reqwest + custom DNS resolver that rejects private/loopback/link-local/metadata IPs (SSRF), re-validated on each redirect; max body 8 MB, HTML 1 MB parsed; timeouts; gzip/br.
- Frontier: per-audit table `audit_frontier(url_hash, url, depth, source, state)` — dedupe by normalized URL.
- Extract with `lol_html` streaming handlers → title, meta, canonical, robots, hreflang, headings, links (≤1000), images (≤1000), JSON-LD blocks, OG, word count (text nodes), content hash (simhash for near-duplicates).
- Rules engine: each rule = `fn(&PageFacts) -> Vec<Issue>` or multi-page `fn(&AuditIndex) -> Vec<Issue>`; severity & thresholds configurable; rule registry documented & versioned.
- Rendering: if `render_js`, send URL to renderer (CDP) → post-render HTML → same pipeline; compare raw vs rendered to flag JS-dependent content.

## 7. Provider abstraction
```rust
#[async_trait]
pub trait SerpProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> SerpCaps;               // engines, devices, local, depth
    async fn search(&self, q: &SerpQuery) -> Result<SerpPage, ProviderError>;
}
// Similar traits: KeywordIdeaProvider, VolumeEstimator, TrendsProvider, BacklinkProvider,
// AuthorityProvider, LlmProvider, PlacesProvider, LighthouseProvider, RdapProvider.
```
- Registry built per-org from `provider_settings`; fallback chain (e.g. `serp = [searxng, chromium_google, brave]`).
- Every response records `provider`, `fetched_at`, `confidence` → surfaced in UI/MCP.
- Global request budgets & rate limits per provider to stay inside free tiers.

## 8. Auth & security
- Sessions: opaque random id in httpOnly, Secure, SameSite=Lax cookie; session store in Valkey + Postgres.
- CSRF: double-submit token for cookie-auth mutating requests; API keys/Bearer exempt.
- RBAC: `owner` > `admin` > `member`; checked in service layer with `Actor { user_id, org_id, role }`.
- MCP: OAuth 2.1 authorization server in Rust (authorization code + PKCE, dynamic client registration, consent page in Next) + API keys; tokens bound to one org (OpenSEO spec 0017).
- Secrets at rest: AES-256-GCM with `APP_ENCRYPTION_KEY`.
- Reports: served from `/r/{id}/raw` with `Content-Security-Policy: sandbox; default-src 'none'; img-src data: https:; style-src 'unsafe-inline'` and `X-Robots-Tag: noindex`.
- Rate limiting: tower-governor / Valkey sliding window on auth & public tools.

## 9. API conventions
- REST under `/api/v1`, JSON, cursor pagination (`?cursor=&limit=`), `ProblemDetails` (RFC 9457) errors, OpenAPI 3.1 at `/api/v1/openapi.json`, docs at `/api/docs`.
- Project-scoped routes: `/api/v1/projects/{projectId}/…` (OpenSEO spec 0001 lesson: projectId explicit in every call).
- Long ops return `202 { job_id }`; poll `GET /jobs/{id}` or SSE.
- Exports: `GET …/export.csv` streaming.

## 10. Docker
Services in `deploy/compose.yaml`:

| Service | Image | Notes |
|---|---|---|
| `db` | `postgres:18-alpine` | volume `pgdata`, healthcheck |
| `cache` | `valkey/valkey:9-alpine` | AOF on |
| `api` | `ghcr.io/<you>/seoseo-backend` | `seoseo serve`, port 8080 |
| `worker` | same image | `seoseo worker`, scale replicas |
| `migrate` | same image | `seoseo migrate` one-shot, `api` depends_on completed |
| `web` | `ghcr.io/<you>/seoseo-frontend` | Next standalone, port 3000 |
| `renderer` | `chromedp/headless-shell` or custom Chromium + Lighthouse | internal only |
| `searxng` | `searxng/searxng` | profile `serp` |
| `ollama` | `ollama/ollama` | profile `ai` |
| `proxy` | `caddy:2` | optional profile `tls`, auto HTTPS |
| `mailpit` | `axllent/mailpit` | dev only |

Backend Dockerfile: multi-stage `rust:1.99-slim-bookworm` + `cargo-chef` cache → `gcr.io/distroless/cc-debian12` (or `debian:bookworm-slim` + ca-certs) runtime, non-root. `SQLX_OFFLINE=true` with `.sqlx/` committed.
Frontend Dockerfile: `node:26-alpine` + pnpm, `next build` with `output: "standalone"` → slim runtime, non-root.
Multi-arch build (amd64/arm64) via `docker buildx` in GitHub Actions.

## 11. Environment variables (initial)
```
DATABASE_URL, VALKEY_URL, APP_BASE_URL, WEB_BASE_URL, APP_ENCRYPTION_KEY (32B base64), SESSION_SECRET,
AUTH_MODE=password|none|oidc, SMTP_URL, SMTP_FROM,
GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET            # GSC/GA4 (+ optional login)
PAGESPEED_API_KEY, CRUX_API_KEY, BING_WMT_API_KEY, GOOGLE_PLACES_API_KEY   # optional, free tiers
SEARXNG_URL, RENDERER_URL, OLLAMA_URL, OPENAI_COMPAT_BASE_URL, OPENAI_COMPAT_API_KEY, LLM_MODEL
DATAFORSEO_LOGIN, DATAFORSEO_PASSWORD, SERPAPI_KEY, SERPER_KEY, BRAVE_API_KEY   # optional BYOK
CRAWLER_USER_AGENT, CRAWLER_MAX_CONCURRENCY, SERP_SCRAPE_ENABLED=false
RUST_LOG, TELEMETRY_ENABLED=false
```
