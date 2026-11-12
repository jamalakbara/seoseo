# Backlink evidence and imported snapshots

M5's backlink foundation adds a **Backlinks** project tab, REST endpoints and four MCP tools (72 tools total). It accepts user-provided JSON samples and compares consecutive imports. It does not discover backlinks, crawl source pages, verify link presence or compute spam scores. Existing M4 individual link observations stay separate.

## Import and comparison rules

An admin of an active project imports a complete JSON array with at most 1,000 distinct source/target URL pairs, within the API's 2 MiB request limit. The UI accepts a file or pasted JSON. Example row:

```json
{"source_url":"https://publisher.example/article","target_url":"https://your-project.example/page","anchor":"Optional anchor text","nofollow":null}
```

Each import supplies a stable lowercase `dataset` key, the exact current project website in `expected_domain`, and `complete_sample: true`. The confirmation means these rows replace this dataset's entire sample, including an empty array. Do not use the same dataset key for partial/differently scoped exports.

Only public HTTP(S) URLs with default ports and no credentials are accepted; URL fragments are removed. Target hosts must equal the project website or one of its subdomains with a label-boundary match. Sources must have a different registrable domain. URLs are capped at 2,048 bytes each, anchors at 500 characters, and normalized duplicate source/target pairs are rejected. Links are counted as URL pairs, not individual hyperlink occurrences. Anchor and nofollow values remain null when unspecified; an empty anchor is distinct from unknown.

The first import for a website/dataset is a baseline: all pairs have state `live`, with no new/lost claims. Later imports compare against the most recent remaining snapshot for that exact website/dataset:

- `live`: pair remains in the sample; current metadata replaces previous metadata.
- `new`: pair is present now but was absent from the previous sample.
- `lost`: pair was in the previous sample but is absent now. Its metadata comes from that previous sample.

`live_count` includes both `live` and `new`. These states describe **sample membership**; a lost pair does not establish that a link was removed from the web. A recurring pair becomes new if it was absent from the immediately previous sample. Old lost rows do not accumulate across snapshots.

Imports, quota checks, baseline selection and deletions serialize under the project-row lock. Snapshot timestamps use the actual insertion clock after acquiring that lock, so concurrent imports form an ordered chain. A snapshot and all its rows commit atomically. Reads use a repeatable-read transaction for consistent metadata/rows during deletion or website changes. Migration `0011_backlink_snapshots.sql` is additive.

## Reading and retention

Members can read. Details are available only for the current project website, including while archived. History metadata lists up to 100 snapshots across all website identities so admins can delete old samples and recover quota. Changing the project website starts an independent comparison history and hides old evidence details. Stale imports are rejected under the write lock.

The table supports `all`, `live` (includes new), `new`, and `lost` filters with up to 200 rows per page (default 100). A snapshot has at most 1,000 current pairs plus 1,000 lost pairs. Referring domains, top target pages and anchor groups aggregate only current membership, independently of pagination/filtering, with the top 100 groups returned for each. Spam score remains null. All views label the evidence source `user_import` and confidence `relative`.

At most 100 snapshots per project are retained, including previous websites. An admin can delete a snapshot in an active project. Other snapshots retain their recorded counts and comparison ID/timestamp even if the comparison snapshot is deleted. Future imports compare against the latest remaining snapshot in that dataset/website. Deleting the latest snapshot therefore changes the next import's baseline. Project deletion cascades to snapshots and evidence.

## REST and MCP

- `GET /api/v1/projects/{project_id}/backlink-snapshots` / `list_backlink_snapshots`: history metadata.
- `POST /api/v1/projects/{project_id}/backlink-snapshots` / `import_backlink_snapshot`: import `{expected_domain,dataset,complete_sample,links}`.
- `GET /api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}?state=all&limit=100&offset=0` / `get_backlink_snapshot`: evidence, summary and groups.
- `DELETE /api/v1/projects/{project_id}/backlink-snapshots/{snapshot_id}` / `delete_backlink_snapshot`: delete one sample.

MCP takes explicit `project_id`/`snapshot_id` fields and uses the same domain services as REST. Only admins can import/delete; cross-project/organization IDs cannot access evidence. No external provider requests or credentials are required.

## Verification

`python scripts/e2e-backlinks.py http://127.0.0.1:3401 seoseo-m5links-db` targets a disposable password-auth stack with open signup and a named test DB container. It verifies baseline/change/empty/reappearance semantics, aggregates, validation, member/tenant/project isolation, stable historical diffs, concurrent baseline/100-snapshot quota enforcement, website changes, archived reads and MCP parity. It writes fixture members/snapshots directly through `docker exec psql`; never point it at production.

`scripts/e2e-backlinks-browser.cjs` tests baseline/change imports and lost-state filtering in a temporary headless Chrome profile, captures desktop/mobile screenshots and checks for browser errors and page overflow. Install `puppeteer-core` outside the repository and set `PUPPETEER_MODULE` to its module path; set `CHROME_PATH` if needed. Run `node scripts/e2e-backlinks-browser.cjs http://127.0.0.1:3400 OUTPUT_DIRECTORY` against the disposable stack only.
