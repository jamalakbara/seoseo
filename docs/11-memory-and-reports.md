# 11 — Project memory and reports

M4's first delivery adds durable project context and self-contained HTML reports. Both are tenant-scoped in Postgres and available through REST, the web app, and MCP.

## Project memory

`GET/PUT /api/v1/projects/{project_id}/context` stores four typed collections:

- context sections (`key`, `title`, `content`, author source);
- competitors (`domain`, optional name and notes);
- key pages (`url`, role `hub|spoke|money|other`, optional topic);
- an immutable research log appended by the server.

The context is one bounded, revisioned document. A client must send the revision returned by its last read; a stale write returns `409 Conflict` instead of silently overwriting another person's or agent's changes. The initial revision is `0`. Limits are 20 sections, 50 competitors, 100 key pages, and the latest 100 research notes.

MCP tools: `get_project_context`, `update_project_context`.

## Reports

Reports contain a title, optional summary, complete HTML document, optional producing skill/template, and creator source. HTML is limited to 500 KiB. List responses omit HTML; the authenticated and public raw endpoints return it with a restrictive CSP (`sandbox`, no scripts, no forms, no external assets), `nosniff`, and no-referrer headers. The web viewer also uses a sandboxed iframe.

Templates are reusable instruction briefs, limited to 10 per project. REST supports create, list, update, and delete. MCP's `save_report_template` creates or updates when `template_id` is supplied.

Sharing generates a random 192-bit URL token. Only its SHA-256 hash is stored. The token is returned once and cannot be recovered; generating another token replaces it. Editing a report or explicitly revoking sharing immediately invalidates the old URL. Public metadata excludes the project id and report HTML; HTML is available only from the sandboxed raw endpoint.

MCP tools: `save_report`, `get_report`, `list_reports`, `delete_report`, `save_report_template`, `list_report_templates`, `delete_report_template`, `set_report_sharing`.

## Verification

- `bash scripts/cargo-docker.sh check --workspace`
- `bash scripts/cargo-docker.sh clippy --workspace --all-targets -- -D warnings`
- `cd frontend && pnpm lint && pnpm typecheck && pnpm build`
- Apply migrations to a disposable database, run the API with `AUTH_MODE=none`, then run `python3 scripts/e2e-m4.py`.
