# Assistant streaming and reviewed memory

This completes AG-01's streaming and reviewed memory follow-ups from the [initial assistant delivery](28-assistant-and-content.md). Subsequent alerts, weekly digests and the remaining milestone features are complete; see [M7 operations and acceptance](30-m7-operations-and-acceptance.md).

## Streamed answers

The worker requests real streamed chat completions from the same approved Ollama/compatible model. The bounded SSE decoder accepts arbitrary transport/UTF-8 boundaries, CRLF, comments and multiline data. It requires both a normal `stop` finish and `[DONE]`. Truncation, malformed packets, unsupported native tool calls/refusals, timeouts and interrupted streams fail the turn without replaying inference. Limits are 45 seconds/request, 1 MiB of wire data, 128 KiB/packet and 20,000 characters/action. Existing five-call/four-tool budgets remain.

The assistant still validates complete JSON actions. While a canonical final action streams (`type` first, `answer` second), it decodes complete string prefixes and persists the draft at most four times/second. Incomplete escapes and Unicode surrogate pairs stay buffered. Other field orders still validate at completion but do not expose partial text. Tool actions and proposed memory notes are not rendered as answer drafts. Drafts are explicitly labelled incomplete; final validation replaces them with the completed answer. Failure clears the draft, and crash reconciliation clears interrupted drafts. No synthetic character animation or extra inference call is used.

`GET /api/v1/projects/{project_id}/assistant/chats/{chat_id}/turns/{turn_id}/events` emits `turn` events containing private turn snapshots. Ownership and workspace membership are checked at entry and throughout the connection. Responses use `Cache-Control: no-store, no-transform`, `X-Accel-Buffering: no`, and keepalives. `no-transform` is required to prevent Next.js compression from buffering partial events until completion; direct API tests alone do not catch that problem. Connections close after terminal status or about 45 seconds, before the existing HTTP timeout. Browser EventSource reconnects to stored state; it never starts a model request. If access ends, an `unavailable` event closes the connection. The UI retains polling as a disconnected-stream fallback, closes subscriptions on chat changes/unmount, and restores persisted drafts after reload/chat selection.

## Reviewed memory notes

A final action may propose an optional plain-text `memory_note` of at most 2,000 characters. Instructions ask the model to propose one only when the user requests remembering information. This is an unsaved suggestion; even an unsolicited model proposal cannot modify memory.

The caller sees **Review memory note**, can edit the note and inspect current memory/recent notes, then explicitly selects **Save reviewed note**. Workspace admins/owners can save their own chat's proposal. Members can chat and view their own proposals, but cannot save them, matching the existing canonical-memory permission. No assistant tool performs writes, and no report/content publication is implied.

`POST /api/v1/projects/{project_id}/assistant/chats/{chat_id}/turns/{turn_id}/memory` accepts `{revision,note}` using normal authentication and cookie CSRF rules. The server appends a research-log entry through the existing canonical memory service; sections, competitors, key pages and unchanged section authors are preserved. Proposal scope must still match the active project website/country/language. Archived projects, another user's chat, non-completed/failed turns, missing proposals and already saved proposals are rejected.

Memory revision checking, the append, and marking the proposal applied occur in one transaction under the project/turn locks. Concurrent/repeated approvals cannot append twice. A stale revision returns 409 without changes; the UI offers **Reload memory for review** and requires another save action. The original model proposal and actual edited saved note remain distinct; the turn records `memory_applied_revision` and `memory_saved_note`. No model inference is invoked by review or approval.

## Migration and validation

Migration `0020_assistant_stream_memory.sql` adds only draft/proposal/approval fields. API/worker must run matching code and migrations. Regenerate OpenAPI/TypeScript as usual. MCP remains at 118 tools; `get_assistant_chat` includes the added draft/proposal state, and the existing canonical memory tools remain available to authorized external clients. Approval is an explicit UI/REST action rather than an automatically callable model tool.

The disposable stack and regression commands are documented in [the initial delivery](28-assistant-and-content.md). The model fixture now emits real SSE packets, delayed partials and interrupted streams. API coverage includes observable draft-before-completion, private SSE access, failure cleanup, no automatic memory write, editable approvals, role/ownership enforcement, stale revisions, repeated/concurrent approvals and changed-market rejection. Browser coverage includes the streaming draft and explicit review/edit/save flow.

Verified 2026-11-15 (Asia/Jakarta):

- All 103 workspace unit tests and doc tests passed. Workspace fmt/clippy with warnings denied and frontend lint/typecheck passed. The Docker frontend production build and standalone Compose startup passed.
- The expanded API regression passed streaming drafts before completion, private SSE access, interruption cleanup, reviewed/edited notes, stale/repeated/concurrent approvals, ownership/role enforcement and changed-market rejection, alongside the existing assistant/content checks.
- Desktop and 390px mobile browser checks passed streaming through the real Next proxy, proposal review/edit/explicit save, no automatic memory write, unchanged original sections, history reload, content coverage/briefs, visible model errors and no page errors/overflow. Screenshots were visually reviewed.
- Real `ollama/qwen2.5:1.5b` completed a streamed in-app answer using canonical memory that included the newly reviewed note. Fixture tests establish failure contracts; the Ollama check separately establishes actual local model access.

QA artifacts for this delivery: `C:/Users/jamal/AppData/Local/Temp/seoseo-m7-browser/qa-stream-memory-verified`. The normal local stack was not rebuilt during that acceptance. Subsequent milestone completion and deployment-specific limits are recorded in [M7 operations and acceptance](30-m7-operations-and-acceptance.md).
