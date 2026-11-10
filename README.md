# SeoSeo

Free, open-source, self-hostable SEO platform — an alternative to Semrush, Ahrefs and OpenSEO that works **without paid API keys**.

> Status: **M0 foundation** — accounts, workspaces, projects, API keys, job queue, OpenAPI, Docker. SEO tools land milestone by milestone ([roadmap](docs/07-roadmap.md)).

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

Checks: `scripts/cargo-docker.sh "clippy --workspace --all-targets -- -D warnings"`, `scripts/cargo-docker.sh "test --workspace"`, `pnpm lint && pnpm typecheck && pnpm build`, and `scripts/smoke.sh` against a fresh stack.

After changing API types: regenerate `frontend/openapi.json` (`seoseo openapi`) and run `pnpm gen:api`.

## Documentation

See [docs/00-README.md](docs/00-README.md): reverse-engineering analysis, PRD, feature catalog, architecture, free data strategy, API/MCP spec, roadmap.

## License

MIT
