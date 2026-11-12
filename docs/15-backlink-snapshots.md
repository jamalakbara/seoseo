# Backlink discovery and snapshot evidence

The **Backlinks** project tab supports guarded source-page discovery, rechecks, imported samples, comparable snapshot history, groups, explainable HTML risk scores and new/lost pairs through REST/UI/MCP. Existing M4 individual link observations stay separate. Coverage is a bounded sample, never a comprehensive backlink index.

## Import and comparison rules

An admin of an active project imports a complete JSON array with at most 1,000 distinct source/target URL pairs, within the API's 2 MiB request limit. The UI accepts a file or pasted JSON. Example row:

```json
{
  "source_url": "https://publisher.example/article",
  "target_url": "https://your-project.example/page",
  "anchor": "Optional anchor text",
  "nofollow": null
}
```

Each import supplies a stable lowercase `dataset` key, the exact current project website in `expected_domain`, and `complete_sample: true`. The confirmation means these rows replace this dataset's entire sample, including an empty array. Do not use the same dataset key for partial/differently scoped exports.

Only public HTTP(S) URLs with default ports and no credentials are accepted; URL fragments are removed. Target hosts must equal the project website or one of its subdomains with a label-boundary match. Sources must have a different registrable domain. URLs are capped at 2,048 bytes each, anchors at 500 characters, and normalized duplicate source/target pairs are rejected. Links are counted as URL pairs, not individual hyperlink occurrences. Anchor and nofollow values remain null when unspecified; an empty anchor is distinct from unknown.

The first import for a website/dataset is a baseline: all pairs have state `live`, with no new/lost claims. Later imports compare against the most recent remaining snapshot for that exact website/dataset/source:

- `live`: pair remains in the sample; current metadata replaces previous metadata.
- `new`: pair is present now but was absent from the previous sample.
- `lost`: pair was in the previous sample but is absent now. Its metadata comes from that previous sample.

`live_count` includes both `live` and `new`. These states describe **sample membership**; a lost pair does not establish that a link was removed from the web. A recurring pair becomes new if it was absent from the immediately previous sample. Old lost rows do not accumulate across snapshots.

Imports, quota checks, baseline selection and deletions serialize under the project-row lock. Snapshot timestamps use the actual insertion clock after acquiring that lock, so concurrent imports form an ordered chain. A snapshot and all its rows commit atomically. Reads use a repeatable-read transaction for consistent metadata/rows during deletion or website changes. Migration `0011_backlink_snapshots.sql` is additive.

## Reading and retention

Members can read. Details are available only for the current project website, including while archived. History metadata lists up to 100 snapshots across all website identities so admins can delete old samples and recover quota. Changing the project website starts an independent comparison history and hides old evidence details. Stale imports are rejected under the write lock.

The table supports `all`, `live` (includes new), `new`, and `lost` filters with up to 200 rows per page (default 100). A snapshot has at most 1,000 current pairs plus 1,000 lost pairs. Referring domains, top target pages and anchor groups aggregate only current membership, independently of pagination/filtering, with the top 100 groups returned for each. Imported spam scores remain null. Views label the source `user_import` or `serp_crawl`, with confidence `relative`.

At most 100 snapshots per project are retained, including previous websites. An admin can delete a snapshot in an active project. Other snapshots retain their recorded counts and comparison ID/timestamp even if the comparison snapshot is deleted. Future imports compare against the latest remaining snapshot in that dataset/website. Deleting the latest snapshot therefore changes the next import's baseline. Project deletion cascades to snapshots and evidence.

## REST and MCP

- `GET /api/v1/projects/{project_id}/backlink-snapshots` / `list_backlink_snapshots`: history metadata.
- `POST /api/v1/projects/{project_id}/backlink-snapshots` / `import_backlink_snapshot`: import `{expected_domain,dataset,complete_sample,links}`.
- `GET /api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}?state=all&limit=100&offset=0` / `get_backlink_snapshot`: evidence, summary and groups.
- `DELETE /api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}` / `delete_backlink_snapshot`: delete one sample.

MCP takes explicit `project_id`/`snapshot_id` fields and uses the same domain services as REST. Only admins can import/delete; cross-project/organization IDs cannot access evidence. Imports require no external provider. Discovery may use configured free SearXNG or optional BYOK search.

## Verification

`python scripts/e2e-backlinks.py http://127.0.0.1:3401 seoseo-m5links-db` targets a disposable password-auth stack with open signup and a named test DB container. It verifies baseline/change/empty/reappearance semantics, aggregates, validation, member/tenant/project isolation, stable historical diffs, concurrent baseline/100-snapshot quota enforcement, website changes, archived reads and MCP parity. It writes fixture members/snapshots directly through `docker exec psql`; never point it at production.

`scripts/e2e-backlinks-browser.cjs` tests baseline/change imports and lost-state filtering in a temporary headless Chrome profile, captures desktop/mobile screenshots and checks for browser errors and page overflow. Install `puppeteer-core` outside the repository and set `PUPPETEER_MODULE` to its module path; set `CHROME_PATH` if needed. Run `node scripts/e2e-backlinks-browser.cjs http://127.0.0.1:3400 OUTPUT_DIRECTORY` against the disposable stack only.

## Discovery and verified rechecks

`POST /projects/{project_id}/backlink-discoveries` / MCP `discover_backlinks` accepts `{expected_domain,search,source_urls}`. Search is an explicit opt-in; up to 20 additional public external source URLs may be supplied. Without search, rechecks work from the previous crawl sample or explicit sources. `GET` / `list_backlink_discoveries` returns the last 20 job statuses, polled by the UI. Only admins of active projects start work; readers remain tenant/current-website scoped. There is one active discovery per project, a one-minute cooldown and a 100-snapshot project quota.

Search discovers candidates with a domain-reference query, then verifies actual HTML hyperlinks. Prior crawl source pages are checked first. At most 40 pages, 1 MiB each, 10-second requests; robots status 200/404/410 required, exclusions and crawl delays respected (minimum one second; delays above 30 seconds skipped). DNS/SSRF protection, no proxies or redirects, default ports, public source/target URLs, website identity checks and stop-on-401/403/429 apply. HTML challenges, meta refresh, tiny SPA shells, extraction limits, unsupported responses and network failures are inconclusive. This is **SERP/explicit-source discovery**, not a Common Crawl inbound index: a CC URL-prefix index finds source URLs, not all pages linking to a target.

Each pair records `verification`, last `verified_at`, relation and risk evidence. `verified` means read on a successful source page; `unconfirmed` preserves a prior pair whose source could not be conclusively checked; `missing` means absent on successfully read HTML or a definitive 404/410 source. A SERP omission alone never establishes loss. Unknown rows retain their prior observation date but have no current risk score. Imported and crawl histories are separate even with the same dataset name. A job's completed status and its full snapshot commit atomically. A sample exceeding 1,000 current pairs fails while preserving the previous snapshot.

Risk is a transparent 0–100 heuristic for verified current pairs: 40 for a source below 100 words, 40 for over 80% external links when there are at least 20 links, 20 for anchor text over 120 characters. Saved reason codes explain each contribution. The summary averages scored current pairs and reports their coverage. A zero is a computed result; null means unscored. This is not malware detection, link quality authority or a search penalty verdict.

`GET /projects/{project_id}/dashboard/backlinks` / `get_dashboard_backlinks` returns the current website's latest full summary plus history. Migration 0014 is additive. The UI exposes discovery polling, provenance/coverage, verification dates, risk reasons, sample filters and grouping alongside manual imports. End-to-end verification is documented in [M5 acceptance](17-m5-acceptance.md).
