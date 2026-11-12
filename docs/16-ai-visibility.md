# Manual AI visibility model comparisons

M5's first AI visibility delivery adds immutable manual trackers, durable queued runs, saved answers, literal brand-term matches, model-reported URL lists and JSON export through the web app, REST and MCP (80 tools total). AI-01/03/04 remain `wip`: scheduling, prompt generation, provider-specific web grounding, share-of-voice/trends and broader engine support remain unfinished. This delivery does not claim consumer chat visibility or independent source verification.

## Operator setup

The API and worker must use the same operator-controlled compatible gateway and approved model list:

```dotenv
OPENAI_COMPAT_BASE_URL=http://host.docker.internal:11434/v1
OPENAI_COMPAT_API_KEY=
AI_VISIBILITY_MODELS=installed-model-a,installed-model-b,installed-model-c
```

Use actual model identifiers installed/available at your gateway. [Ollama's compatible API](https://docs.ollama.com/api/openai-compatibility) supports the chat-completions interface used here. `AI_VISIBILITY_MODELS` allows up to 10 unique comma-separated model identifiers; if empty, it falls back to the existing `LLM_MODEL`. Each tracker chooses at most three. One configured gateway is supported; selecting three model identifiers does not establish three independently grounded consumer engines. A paid gateway may bill the operator's account; local Ollama requires no paid key. Actual installed models and live gateway access remain operator setup, separate from offline fixture validation.

Project inputs cannot set the endpoint, credentials or arbitrary model IDs. Only the operator environment controls the gateway. The endpoint must be an HTTP(S) base URL without embedded credentials, query or fragment. Redirects and environment proxies are disabled; private/local endpoints are allowed because the URL is operator configuration. The model-list endpoint returns readiness and IDs, never the key or URL. Changes to the gateway after queuing fail that run before any call; a removed model also stops collection.

## Trackers and explicit runs

Admins of active projects create trackers with an exact `expected_domain`, name, 1–10 brand terms (80 characters each), 1–10 prompts (500 characters each), 1–3 approved models and country/language (default project market). Terms/prompts are trimmed and case-insensitive duplicates rejected. Tracker creation makes no model request. Configurations are immutable; create another tracker to change prompts/models/terms. The UI accepts one term/prompt per line; REST supports multiline prompt strings.

**Run model comparison** explicitly queues up to 30 requests. It freezes all tracker inputs, the requested market and a gateway fingerprint in the same transaction as the job. The system instruction supplies the requested country/language but no tracked terms; terms are used only when scoring the answer. This avoids automatically inserting the expected brand into otherwise unbranded prompts. Prompts supplied by the user can still bias the sample. Country/language are requested model context, not geographic measurement. No search, browsing, function execution or external citation retrieval tools are supplied.

Requests are sequential, paced at least one second apart per worker process, with temperature 0, a 1,500-token output request, 45-second timeout, 128 KiB response cap and 20,000-character text cap. A normal nonempty text answer with `finish_reason=stop` is required; truncated/empty/filtered/tool-call responses are failures. Model aliases/weights can change at the gateway; frozen inputs do not guarantee repeatability.

The job checkpoints each model/prompt cell to Postgres. A resumed worker skips persisted successes. Provider/network/format failures save an error cell, fail the run and stop further calls; collected answers remain available. Internal database/queue failures may retry through the existing durable queue. Terminal queue failures are reconciled to failed runs when read or before another run. A crash between an external request and storing its result may repeat that request on resume; this is not an exactly-once external API guarantee.

There is one active run per tracker, at most two per project, and a one-minute cooldown between retained runs for a tracker. Limits are serialized under the project lock. There are at most 10 trackers per project and 20 retained runs per tracker. Admins can delete completed trackers/runs for quota cleanup; active runs prevent deletion for that tracker. Archived projects remain readable. A website change hides previous-website details and stops subsequent worker requests for those trackers; prior tracker metadata remains available for cleanup. A request already in flight may finish, but its answer is not intentionally committed after the worker detects a changed/archived project.

## Meaning of the results

- Every view includes expected, successful and failed answer counts. Cells without a saved answer may be pending, interrupted or unattempted; they are distinct from measured absence.
- Terms match case-insensitively with Unicode alphanumeric boundaries across the full answer text. This is literal occurrence, not sentiment, endorsement, rank or recommendation. Configure explicit aliases as separate tracked terms when useful.
- Each term's `mention_rate_pct` is its number of matching successful answers divided by all successful answers. With no successful answers it is null, not zero. Failed/uncollected cells never enter the denominator. Several terms can match one answer, so rates need not sum to 100%; this is not share of voice.
- Up to 20 unique public HTTP(S) URLs are extracted from answer text. They are model-reported, unverified strings, not validated citations or evidence that the model browsed a page. Credentials/private-address URLs are excluded, no URL is fetched, and the answer viewer renders plain escaped text.
- Source is `model_api`, confidence `relative`. Run JSON export includes frozen configuration, raw text answers, matches, URLs, errors and coverage. It contains user prompt/answer data and is tenant-scoped, with no public share route.

## API and MCP

- `GET /api/v1/ai-visibility/models` / `list_ai_visibility_models`
- `GET|POST /api/v1/projects/{project_id}/ai-trackers` / `list_ai_visibility_trackers`, `create_ai_visibility_tracker`
- `GET|DELETE /api/v1/projects/{project_id}/ai-trackers/{tracker_id}` / `get_ai_visibility_tracker`, `delete_ai_visibility_tracker`
- `POST /api/v1/projects/{project_id}/ai-trackers/{tracker_id}/runs` / `run_ai_visibility_tracker`, body `{ "expected_domain": "example.com" }`
- `GET|DELETE /api/v1/projects/{project_id}/ai-trackers/{tracker_id}/runs/{run_id}` / `get_ai_visibility_run`, `delete_ai_visibility_run`

Members read; admins create/run/delete. MCP invokes the same domain services as REST. Additive migration `0012_ai_visibility.sql` adds trackers, runs and answer cells. No applied migration is changed.

## Verification

`scripts/ai-model-fixture.py` is a local compatible gateway for offline tests, not a model implementation. Configure API/worker with its `/v1` URL and models `fixture-a,fixture-b,fixture-c` on a disposable password-auth stack with open signup. `python scripts/e2e-ai-visibility.py http://127.0.0.1:3401 seoseo-m5ai-db seoseo-m5ai-worker http://127.0.0.1:3402` verifies three-model collection, frozen prompts, mention rates/unknowns, partial failures/truncation, checkpoint resume, queue reconciliation, changed gateway identity, concurrency/retention limits, role/tenant/project/website isolation, deletion and MCP parity. It pauses the named test worker and seeds fixtures through `docker exec psql`; never run against production.

`node scripts/e2e-ai-browser.cjs http://127.0.0.1:3400 OUTPUT_DIRECTORY` uses the same disposable fixture stack and a temporary `puppeteer-core` installation via `PUPPETEER_MODULE`. It checks tracker creation, explicit queueing/polling, rates, expanded answers and desktop/mobile layouts; `CHROME_PATH` overrides the browser executable. Live Ollama or paid engines have not been validated by these fixture tests.
