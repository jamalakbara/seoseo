# M5 implementation and acceptance

M5 implementation covers DO-01..05/07, BL-01..06, DB-04, AI-01..04 and MC-05. The exact roadmap acceptance gate requiring **three live AI engines including Ollama remains pending**. Local Ollama was run successfully; Anthropic and Gemini have offline contract checks, not verified live accounts. No cloud credentials were available in this workspace.

## Delivered behavior

| Area | Implementation | Boundary |
|---|---|---|
| Competitive | Overview/SR, observed keywords and distribution, pages, competitors, gaps, RDAP age, volume import and sample-click estimates | Up to 500 project keywords; exact ranks and eligible monthly search counts required for estimates; never total domain traffic |
| Backlinks | Search/explicit-source discovery, robots-aware HTML verification, rechecks, imported samples, history, groups, filters, risk reasons and dashboard API | Up to 40 source pages/1,000 current pairs; failures retain prior evidence as unconfirmed; no exhaustive inbound index |
| AI | Compatible gateway plus native Ollama, Anthropic and Gemini; schedules, question drafts, durable answers, literal rates, share of tracked mentions, trends, citations and JSON export | Anthropic requests web search; Gemini/local/compatible have no web tools. Country/language are requested context, not measured location |
| Agents | Fourteen portable SeoSeo skills, display metadata, install/connection guide, upstream attribution and validator | Self-hosted MCP server only; no global installation or invented M6 capabilities |

There are 87 MCP tools. New schema is additive: 0013 schedules/citation provenance, 0014 search volumes/discovery/verification. Existing migrations remain unchanged. Normal deployments must rebuild the API/worker/web images and run migrations. Existing local deployment data was not migrated during acceptance work.

## Verified

- Rust formatting, strict workspace Clippy and all 76 workspace unit tests.
- Generated OpenAPI/TypeScript, frontend typecheck/lint and production Docker builds.
- Isolated Compose startup with Postgres, Valkey, migrations, API, web and workers. Two workers exercise the same due schedule without duplicate runs; retention pauses at 20 runs and preserves evidence.
- Existing competitive, imported-backlink and AI regressions: roles, tenant/project/current-website isolation, validation, concurrent quotas, durable partial failures/resume and REST/MCP parity.
- Final M5 regression: real local Ollama `smollm2:135m`; native Anthropic request/auth/search/citation fixture; native Gemini text-only request/auth fixture; drafts, rates/trends, schedules, eligible/partial click estimates, robots, blocked/redirect unknowns, verified missing links, over-limit snapshot preservation and dashboard/MCP parity.
- Headless Chrome desktop (1440px) and mobile (390px): draft review/apply, tracker collection, citation links/trends, schedule save/disable, discovery polling/risk and volume import/estimates. No page errors or document overflow. Screenshots were visually inspected.
- All fourteen skills pass the skill-creator validator and the repository tool-reference validator.

Fixture output is synthetic and explicitly labelled. The tiny local model proves API integration, not answer quality. Fixture checks do not establish real cloud credentials, billing, model availability or search permissions. Live free SERP coverage depends on the configured provider; the crawl regression uses local HTML fixtures.

## Reproduce on a disposable stack

Run from the repository root. The test overlay disables private-address crawler protection and uses open signup and synthetic API keys. Never apply it to a normal deployment. It needs ports 3400/3402/3403, Docker, Python, pnpm and enough memory for the builds/local model. Override `M5_MODEL_PORT`/`M5_RESEARCH_PORT` if occupied and use those ports in the test commands.

PowerShell example:

```powershell
$m5Env = Join-Path $env:TEMP 'seoseo-m5check.env'
Copy-Item .env.example $m5Env
$env:POSTGRES_PASSWORD = 'disposable-m5check-only'
$env:WEB_PORT = '3400'
$env:WEB_BASE_URL = 'http://127.0.0.1:3400'
$env:SEOSEO_BACKEND_IMAGE = 'seoseo-backend:m5check'
$env:SEOSEO_WEB_IMAGE = 'seoseo-web:m5check'
docker build -t seoseo-backend:m5check backend
docker build -t seoseo-web:m5check frontend
$m5Args = @('-p','seoseo-m5check','-f','deploy/compose.yaml','-f','deploy/compose.m5-test.yaml','--env-file',$m5Env,'--profile','ollama')
docker compose @m5Args up -d --no-build --scale worker=1
docker compose @m5Args exec -T ollama ollama pull smollm2:135m
python scripts/e2e-competitive.py http://127.0.0.1:3400 seoseo-m5check-db-1
python scripts/e2e-backlinks.py http://127.0.0.1:3400 seoseo-m5check-db-1
python scripts/e2e-ai-visibility.py http://127.0.0.1:3400 seoseo-m5check-db-1 seoseo-m5check-worker-1 http://127.0.0.1:3402
docker compose @m5Args up -d --no-build --scale worker=2
python scripts/e2e-m5-final.py http://127.0.0.1:3400 seoseo-m5check-db-1 http://127.0.0.1:3402 http://127.0.0.1:3403
python scripts/validate-agent-skills.py
```

The legacy AI regression pauses its named worker, so run it with one worker. The final regression waits for 60-second scheduler ticks, forces test-only due times/cooldowns through the named database and may take several minutes. Run scripts sequentially: fixtures have shared request counters/modes.

For UI checks, install `puppeteer-core` outside the repository and set `PUPPETEER_MODULE` to its module path (`CHROME_PATH` optionally selects Chrome):

```powershell
node scripts/e2e-m5-final-browser.cjs http://127.0.0.1:3400 seoseo-m5check-db-1 http://127.0.0.1:3403 (Join-Path $env:TEMP 'seoseo-m5check-screenshots')
docker compose @m5Args down -v
```

Cleanup removes only that explicitly named disposable project and its test volumes. Restore/remove task-specific environment variables afterwards. The three-live-engine gate requires operator-provided Ollama, Anthropic and Gemini configuration from [AI visibility](16-ai-visibility.md), then a tracker with one real model from each engine and all three successful saved answer cells. No fixtures can satisfy that gate.
