# M7 operations and acceptance

M7 is complete as of 2026-11-15 (Asia/Jakarta). [Assistant/content](28-assistant-and-content.md) and [streaming/reviewed memory](29-assistant-streaming-and-memory.md) cover AG-01..03. This delivery completes movement alerts, key-page monitoring, destinations, built-in reports/PDF, weekly email, business records, backup/restore and account deletion. The server exposes 121 MCP tools.

## Alerts and destinations

Open a project's **Alerts** tab. An admin chooses a position threshold (1–100), entering/leaving measured depth, and up to 100 exact-host key-page URLs. Settings use revision checks; reload after a concurrent-edit conflict. Changing project domain/market requires saving settings for the new scope. Enabling alerts considers subsequently completed observations, without backfilling old runs or calling providers. Schedule rank tracking and re-audits separately to collect observations.

The worker evaluates stored observations every minute. A manual evaluation uses the same domain service. Rank comparisons require adjacent completed runs from the same tracker, provider, exactness and measured depth. A null position means outside that measured depth, not position zero; absent result rows are unknown. Key pages compare completed audits with the same start URL and render mode, considering successful HTML pages without redirects. Missing/failed pages are incomparable. Title, description, canonical, robots and indexability changes are recorded. Each evaluation retains comparable/unavailable counts; event details contain at most 100 changed items. The feed supports acknowledgement and pagination.

An evaluation, immutable event and delivery jobs commit together. Unique source keys prevent concurrent evaluations from duplicating alerts. The scheduler rotates through up to 1,000 eligible projects per tick, processing up to 20 sources per project; large backlogs drain over subsequent ticks.

Admins can add up to ten generic webhook, Slack or Discord incoming-webhook destinations. Secret URLs are encrypted with `APP_ENCRYPTION_KEY`; reads return only name/type/host/state. Public HTTPS is required, with DNS filtering, no proxy and no redirects. Slack and Discord require their official webhook paths. Slack formatting escapes mention syntax; Discord disables mention parsing. Tests are limited to three per minute per destination. `NOTIFY_ALLOW_PRIVATE=true` exists solely for trusted disposable transport tests and is not forwarded by normal Compose.

Delivery is **single attempt**: an atomic transition to `sending` precedes network I/O. Timeouts, crashes, HTTP errors and ambiguous disconnects are visible in delivery history and never automatically replayed. This avoids duplicate external messages at the cost of possible missed delivery. Disabling/removing a destination, changing its revision, archiving/changing project scope, or disabling a subscription cancels stale queued work. Members see only their own email deliveries; admins can inspect project history.

## Weekly email

Set identical operator configuration on API and worker:

```dotenv
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_FROM=reports@example.com
SMTP_USERNAME=operator-supplied
SMTP_PASSWORD=operator-supplied
SMTP_SECURITY=starttls
```

`starttls` requires STARTTLS; `tls` uses implicit TLS. `none` is intended for a trusted local relay such as the acceptance receiver. No SMTP configuration means the UI shows delivery unavailable. Each member can opt their own account email into one weekly project digest, choosing weekday and hour **in UTC**; the next date is displayed in the browser's timezone. Arbitrary recipients are rejected. Digests contain stored audit evidence and recent scoped alerts, with no fresh provider/model requests. Claims are durable and deduplicated by scheduled period. Delivery history exposes errors; there is no automatic resend after an ambiguous attempt.

## Reports and PDF

The Reports page generates five saved HTML report kinds: `audit`, `rank`, `keywords`, `links`, `ai`. Optional title/byline/accent provide simple branding. Generation preserves source, timestamp, null and sample-limit metadata, escapes evidence HTML, and does not share automatically or fetch fresh data. Rank/keyword samples are capped at 200 rows; link snapshots and AI runs at 100. Audit reports use the latest matching completed audit. Oversized evidence is rejected explicitly.

Enable the renderer profile and set `RENDERER_URL=http://renderer:8080` for **Download PDF**. Chromium runs in a separate restricted renderer service; PDF export disables JavaScript and external network requests, allows embedded data images/inline styles, limits input to 500 KiB and output to 16 MiB, and closes the browser before returning. It does not load external report assets. Existing HTML sharing and revocation rules remain in force.

MCP adds `list_alerts`, `get_alert_settings` and `generate_report`, using the same authorization/domain services as REST. Destination secrets, account deletion and email enrollment are explicit UI/REST operations.

## Business workspace

The Business tab stores a source-labelled profile and review/question/update records, including response drafts. Admin writes use revision checks; members can read. These are **local records and drafts**, not Google Business Profile publishing or sent replies. Up to 1,000 records per project are supported with pagination.

Optional `GOOGLE_PLACES_API_KEY` enables an explicit read-only Google Place Details preview by exact Place ID. Enable Places API (New) in the operator's Google project and review its billing/field pricing. Responses are transient, not copied into saved records; optional ratings/reviews are a provider-selected sample of up to five. See Google's [Place Details documentation](https://developers.google.com/maps/documentation/places/web-service/place-details). Live credentials and external publishing are not required for the manual workspace.

## Account deletion

Open **Account**, enter the current password and `DELETE MY ACCOUNT` to request deletion after seven days. Only an authenticated password session can request/cancel; API keys and no-auth mode cannot. The account remains usable and cancellation is available during the grace period. The worker performs due deletion. A sole owner of a shared workspace must first transfer ownership; the worker checks again before purging and reports a blocked reason if ownership changed.

Deletion removes the user, sessions/private records and solo-owned workspaces. Shared workspaces and their project evidence survive; the deleted user's GSC, GA4 and Bing connections are removed. This is application data deletion, not erasure from historical operator backups. Restore procedures must account for deletion requests made after a backup.

## Backup and restore

Python 3.11+ and Docker CLI are required. Choose a new private backup folder outside the repository, with an existing parent:

```powershell
python scripts/maintenance.py backup --container seoseo-db-1 --directory F:/private-backups/seoseo-20261115 --env-file .env
```

The custom-format Postgres dump, operator environment and SHA-256 manifest include encrypted tokens **and the secrets needed to decrypt them**. The script restricts folder/file permissions (including Windows ACL inheritance). Keep the folder private and copy it to secure operator storage. PostgreSQL is durable state; Valkey caches are disposable. Keep matching application images/source and restore the original `APP_ENCRYPTION_KEY`.

Provision an empty target database and choose a new environment output path. Stop application writers on the target before restoration:

```powershell
python scripts/maintenance.py restore --container seoseo-db-1 --db-name seoseo_restored --directory F:/private-backups/seoseo-20261115 --env-output F:/private-backups/restored.env --confirm "RESTORE EMPTY DATABASE"
```

Checksums are checked before restoration; nonempty databases and existing environment output files are refused. `pg_restore` uses one transaction. Review restored database/container addresses, then start matching images/migrations. Retain and periodically test private backups; the script does not configure storage retention or an unattended schedule.

## Acceptance evidence

All checks used disposable data/receivers, without production email, Slack/Discord messages or live Places requests:

- 106 Rust unit tests and doc tests passed; domain tests and workspace clippy with warnings denied were rechecked after report presentation changes.
- Frontend lint/typecheck and production Docker web build passed. Desktop and 390px browser flows passed without page errors or horizontal page overflow: settings, destination test/status, weekly subscription, rank evidence, business profile/review edit with rating preservation, Places preview, report generation/PDF, deletion/cancellation.
- `scripts/e2e-m7.py` passed tenant/member/CAS checks, comparable/incomparable evidence, concurrent evaluation deduplication, actual local webhook delivery, 429/disconnect/redirect refusal without retries, queued destination cancellation, ambiguous-attempt failure without replay, test rate limits, all five reports and escaped HTML. The three added MCP tools passed parity and project isolation checks.
- Real Chromium export passed, including script/external-image isolation. Exported pages were rendered with Poppler and visually inspected.
- A due weekly schedule delivered actual SMTP mail to Mailpit once across worker restarts. Unsubscription was verified. Earlier real local Ollama acceptance satisfies the assistant half of the roadmap exit criterion.
- Seven-day request/cancel and forced-due synthetic purge passed, preserving a shared workspace while deleting a solo workspace/user and its shared-project OAuth grant.
- Real `pg_dump`/empty-target `pg_restore` passed, comparing restored table counts and environment bytes; restoration to a nonempty database was refused.
- Fourteen portable skills validate against all 121 MCP tools.
- Production backend, web and renderer Docker images built successfully. The complete backend acceptance suite passed again with the distroless production API/worker image in the disposable stack; normal Compose configuration also validates.

Reproduce against `deploy/compose.m7check.yaml` (never a production database): build `app` in the documented Rust helper volume, start the disposable stack, run `python scripts/e2e-m7.py`, then `scripts/e2e-m7-browser.cjs` with `PUPPETEER_MODULE` pointing to an installed Puppeteer Core and arguments for web origin, disposable DB container and screenshot folder. Existing assistant fixture/real Ollama commands are in the prior guides. The backend script uses only containers named `seoseo-m7check*`.

Production-image backend QA artifacts are in `F:/seoseo-m7-qa/seoseo-m7-acceptance-o60ch7ah`; browser screenshots and the verified downloaded PDF are in `F:/seoseo-m7-qa/browser/qa-m7-packaged`. The final browser pass also verifies the visible Places rating and source links. Earlier artifacts remain in Windows temp and `F:/seoseo-m7-qa/browser/qa-m7-complete`. These were moved off the system drive after it filled during the first production backend image build attempt. Docker and normal local services recovered without deleting durable volumes; the subsequent production backend image build passed. Backups contain disposable fixture secrets and are not committed. Production deployment is separate from the disposable acceptance stack. Live SMTP/Slack/Discord/Places operator setup remains deployment-specific. M5's live-cloud acceptance and M6's final export closeout remain separate open gates; M7 completion does not close them.
