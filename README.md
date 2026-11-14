# SeoSeo

Free, open-source, self-hostable SEO platform — an alternative to Semrush, Ahrefs and OpenSEO that works **without paid API keys**.

> Status: **M6 implementation delivered; acceptance in progress**. Local scans/maps/competitors, city demand imports, Google AI Overview observations, CrUX and Bing accompany clustering, audit comparison/scheduling and the earlier milestones, with 115 MCP tools. M5's three-live-engine gate still needs direct Anthropic/Gemini credentials. See [M6 acceptance](docs/27-m6-acceptance.md), [M5 acceptance](docs/17-m5-acceptance.md) and the [roadmap](docs/07-roadmap.md).

M5 guides: [competitive research](docs/14-competitive-research.md), [backlinks](docs/15-backlink-snapshots.md), [AI visibility setup](docs/16-ai-visibility.md), [agent skills](plugins/seoseo/README.md). Domain/backlink coverage is a bounded sample, and traffic estimates require eligible monthly search counts. Unknown metrics stay null.

Latest guides: [Local SEO and city context](docs/25-local-seo.md), [Google AI Overviews](docs/26-google-ai-overviews.md), [CrUX field performance](docs/23-crux-field-data.md) and [Bing Webmaster Tools](docs/24-bing-webmaster.md). Live collections are explicit and bounded; source-labelled imports work without paid keys.

## Quick start (Docker)

```bash
cp .env.example .env          # set POSTGRES_PASSWORD
docker compose -f deploy/compose.yaml --env-file .env up -d --build
```

Open <http://localhost:3000>. The first account you create becomes the instance admin.
API docs: <http://localhost:3000/api/docs>.

Forgot the admin password?

```bash
docker compose -f deploy/compose.yaml --env-file .env run --rm -e NEW_PASSWORD='new password here' api admin reset-password --email you@example.com
```

## Stack

| Part | Tech |
|---|---|
| Backend | Rust 2024 · axum 0.8 · sqlx 0.9 · PostgreSQL 18 · Valkey 9 · utoipa (OpenAPI 3.1) |
| Frontend | Next.js 16 · React 19 · Tailwind 4 · shadcn/ui (Base UI) · TanStack Query · openapi-fetch |
| Ops | Docker Compose · one Rust binary (`seoseo serve | worker | migrate | admin | openapi`) |

## Development

```bash
docker compose -f deploy/compose.dev.yaml up -d                 # Postgres + Valkey
scripts/cargo-docker.sh "run -p app -- serve --migrate"         # or cargo directly with a local toolchain
cd frontend && pnpm install && pnpm dev
```

Checks: `scripts/cargo-docker.sh "clippy --workspace --all-targets -- -D warnings"`, `scripts/cargo-docker.sh "test --workspace"`, `pnpm lint && pnpm typecheck && pnpm build`.

End-to-end tests run on a throwaway stack with the deliberately broken `badseo` site (see `CLAUDE.md` for the exact commands): `scripts/smoke.sh`, `scripts/e2e-audit.sh`, `scripts/e2e-team.sh`.

After changing API types: `scripts/gen-api.sh`.

## Documentation

See [docs/00-README.md](docs/00-README.md): reverse-engineering analysis, PRD, feature catalog, architecture, free data strategy, API/MCP spec, roadmap.

## License

MIT
