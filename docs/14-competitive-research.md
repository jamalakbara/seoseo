# Competitive research: first M5 delivery

The project's **Competitive** tab and `get_domain_overview` MCP tool inspect a domain against locally available search evidence. This begins DO-01 through DO-05; M5 remains in progress. Backlink discovery, domain age and AI visibility are not included in this delivery.

## Evidence and limits

- Scope is the union of saved keywords and research ideas belonging to the authorized project, filtered to one country/language. The project market is the default. Other projects' keyword lists never enter the scope.
- Sample the first 500 distinct keywords alphabetically. `scope_keywords` reports the full scope and `sampled_keywords` the bounded sample.
- Use the newest cached snapshot per keyword from the last 30 days, including empty results. Newer metasearch snapshots replace older exact snapshots. Each keyword includes provider, exact flag and timestamp.
- Only positions 1–30 count. Group by registrable domain using existing public-suffix rules; choose the best position once per domain/keyword. Subdomains roll up to the same domain.
- `checked_keywords` includes recent empty snapshots; missing/expired snapshots are unchecked. Coverage includes keywords where the requested domain was absent.
- Return observed keyword count, position distribution, best-result URLs, domains sharing observed keywords, and keyword gaps against the project's website. Gaps mean absence from the sampled page; they do not prove a domain does not rank.
- Keywords, pages and gaps are capped at 100 rows each; competitors at 50. Totals/distribution use the full bounded sample. Pages count only each keyword's best result URL.
- SeoSeo Rank uses the separately imported Common Crawl graph and its source/release metadata. Monthly traffic remains `null`. The domain sample has source `project_serp_sample` and confidence `relative`; it is not a comprehensive domain index.

Inspection is read-only and makes no external requests. Build coverage with keyword research or saved-keyword refresh, then reload evidence. No new migrations or credentials are required.

## API and MCP

`GET /api/v1/projects/{project_id}/domain-overview?domain=example.com&country=US&language=en`

All query parameters are optional; domain defaults to the project website. A project without a website must supply `domain`. Domains and HTTP(S) website URLs normalize to registrable domains. Members can read, including archived projects; cross-organization access returns 404.

MCP `get_domain_overview` takes `project_id` and the same optional query fields, returning the same result in the standard envelope. There are now 67 MCP tools.

## Verification

`cargo test -j 1 --workspace` covers aggregation, deduplication, gaps, unknown metrics, normalization and response limits. `python scripts/e2e-competitive.py http://127.0.0.1:3401 seoseo-m5-db` targets a disposable API/database with password auth and open signup. It seeds synthetic SERPs through the named database container and verifies latest/empty/expired snapshots, research-only scope, market/project/tenant isolation, validation, the 500-keyword bound and REST/MCP parity. Never run this fixture script against production.
