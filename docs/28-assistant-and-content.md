# M7 assistant and content foundation

The project **Assistant** tab adds private conversations, tool evidence, content briefs and draft term coverage. This records the initial M7 delivery. Its streaming and reviewed-memory follow-ups are implemented in [Assistant streaming and reviewed memory](29-assistant-streaming-and-memory.md). The remaining milestone features are completed in [M7 operations and acceptance](30-m7-operations-and-acceptance.md).

## Configuration and privacy

Configure the same existing model settings on API and worker: `AI_OLLAMA_URL` + `AI_OLLAMA_MODELS` for free local inference, or `OPENAI_COMPAT_BASE_URL` + `LLM_MODEL` (optionally `AI_VISIBILITY_MODELS` and `OPENAI_COMPAT_API_KEY`) for a compatible gateway. Pull an instruction-following model into Ollama first. Use a context window of at least 16k tokens. JSON-object response mode must be supported. Native Anthropic/Gemini visibility engines are excluded from the assistant.

The selected model receives project identity/market, bounded canonical memory/tool evidence and up to eight prior completed turns. Each history message/answer caps at 1,000 characters; evidence objects cap at 6,000 characters with labelled truncated previews. External gateways receive the same data when selected. Chats are private to the authenticated user, including API keys/OAuth acting as that user. Admins do not automatically see other users' chats. Draft coverage runs locally without sending drafts to a model.

Endpoints/keys come only from operator configuration. Model requests follow no redirects/proxies, time out after 45 seconds and cap responses at 128 KiB. Malformed, truncated, blocked and rate-limited responses fail visibly.

## Read-only tools and lifecycle

Messages enqueue durable single-attempt `assistant_turn` jobs and return 202. The UI polls queued/running turns and resumes after refresh. Progress also appears in Jobs. Each model response selects one JSON action:

| Tool | Evidence |
|---|---|
| `project_memory` | Canonical context/key pages/research notes; injected before the first call |
| `audit_summary` | Latest completed audit matching the project website |
| `rank_movers` | Comparable numeric position changes from the current website/market dashboard sample |
| `cached_serp` | Up to ten titles/snippets within seven days in the project market; provider/exactness/time included |

Models cannot select tenant identities or URLs to fetch. Unknown tools produce evidence errors; unexpected action fields fail closed. There is no shell, fetch, mutation or arbitrary MCP dispatch. Stored data can influence answer quality but cannot grant extra capabilities. Answers render as plain text.

Each turn permits four additional tool actions/five model calls. Intent persists before model access; ambiguous requests are never replayed. Lease reaping fails crashed single-attempt jobs, and reads/new sends reconcile failed turns. Saved evidence survives errors; retry requires a new message.

Limits: 20 chats/user/project; 40 turns/chat; one active turn/chat; four active turns/workspace; 4,000 characters/message. Project/workspace locks serialize quotas and queue insertion. Chats freeze website/country/language. Changes/archival prevent continuation and invalidate active work before another model request/final answer. History retains its original scope. Workers recheck membership before requests. Deletion removes prompts/answers/evidence; job payloads contain IDs only. Already submitted model requests cannot be recalled.

## Content

Briefs require a nonempty SERP cached within seven days in the project market. They open a private assistant conversation with an editorial request for audience/intent assumptions, title/outline/questions, links from saved key pages, sources and evidence gaps. They do not publish, save reports, refresh SERPs or write memory. Recommendations need editorial review; snippets do not establish full competitor content.

Coverage uses tokens appearing in at least two distinct top-ten titles/snippets. It removes common English/Indonesian stop words and numeric tokens, selects up to 30 by reference frequency/lexical order, and returns the percentage present in the draft using case-insensitive whole-token matching. Missing evidence/no repeated terms yields `null`; measured zero overlap yields `0`. Matching works best in languages with spaces; word count uses whitespace. This is a lexical editorial hint, not quality or ranking prediction. Sources, exactness, market, time and references remain visible.

## REST and MCP

Authenticated project paths start at `/api/v1/projects/{project_id}`:

| Method/path | Purpose |
|---|---|
| GET `/assistant/models` | Models and disclosure |
| GET/POST `/assistant/chats` | List/create caller's chats |
| GET/DELETE `/assistant/chats/{chat_id}` | Read/delete caller's chat |
| POST `/assistant/chats/{chat_id}/messages` | Queue `{model,message}`; 202 |
| POST `/content/brief` | Queue `{keyword,model}`; 202/private chat |
| POST `/content/score` | Local `{keyword,content}` coverage |

MCP adds `create_content_brief`, `score_page_content`, `get_assistant_chat` (118 tools). Poll the private chat until its final turn is completed/failed.

## Verification

Migration `0019_assistant.sql` is additive. Run workspace fmt/clippy/tests, frontend lint/typecheck/build and API generation. Build the debug app with backend mounted at `/src` and `seoseo-target` at `/src/target`, then run `docker compose -p seoseo-m7check -f deploy/compose.m7check.yaml up -d --build` and `python scripts/e2e-assistant.py http://127.0.0.1:3501 seoseo-m7check-db-1`.

Test Compose does not read production `.env`; loopback ports are 3500 web, 3501 API, 3502 offline model. Optional `ollama` reuses the external `seoseo-m5-ollama-models` volume; pass `M7_OLLAMA_MODELS` explicitly. Fixtures verify contracts/isolation, not model intelligence or live search.

Verified 2026-11-14 (Asia/Jakarta):

- Workspace fmt and clippy with warnings denied passed. All 101 workspace unit tests and doc tests passed, including action parsing, instruction/data roles, evidence truncation, missing metrics and whole-token coverage.
- Frontend lint, typecheck and production build passed; Docker web image built and standalone Compose started successfully. Generated OpenAPI matched the built binary byte-for-byte. The 14 portable skills validate against 118 MCP tools. Generated third-party MapLibre assets are excluded from lint.
- `e2e-assistant.py` passed against a disposable password-auth database: ownership/tenant/auth, strict input validation, tool evidence/history, unknown tools, forbidden identity arguments, provider failures without retry, tool budget, interrupted work, cached briefs, null/zero/full coverage, market isolation, concurrent chat quotas, deleted-chat outstanding job budget, API-key/MCP parity and deletion.
- `e2e-assistant-browser.cjs` passed desktop and 390px mobile flows with no page errors or horizontal overflow: chat creation/send, reload/history selection, expanded evidence, missing/100% coverage, queued brief and malformed-model failure. It caught and fixed a new-chat text-clearing race. Screenshots were visually reviewed.
- Real `ollama/qwen2.5:1.5b` completed an in-app turn using canonical memory and correctly answered that the fixture business provides technical SEO audits for small businesses. This confirms local inference through the actual worker/UI path, not just a fixture. Early small-model tool loops prompted separating server policy into the system role and an explicit final-response instruction; loop failure remains bounded and visible.

Browser command: `PUPPETEER_MODULE=PATH_TO_TEMP_INSTALL/node_modules/puppeteer-core node scripts/e2e-assistant-browser.cjs http://127.0.0.1:3500 seoseo-m7check-db-1 OUTPUT_DIRECTORY ollama/qwen2.5:1.5b`. `CHROME_PATH` optionally selects Chrome. QA artifacts for this run remain outside the repository at `C:/Users/jamal/AppData/Local/Temp/seoseo-m7-browser/qa-verified`.

This records the initial delivery on `codex/m7-assistant-content`. Subsequent streaming/reviewed memory and the completed weekly digest exit criterion are documented in the linked follow-up guides.
