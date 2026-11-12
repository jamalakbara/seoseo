# Competitive research: first M5 delivery

The project's **Competitive** tab and `get_domain_overview` MCP tool inspect a domain against locally available search evidence. This begins DO-01 through DO-05; M5 remains in progress. On-demand domain age (DO-07) is available separately. Backlink discovery and AI visibility remain unfinished.

## Evidence and limits

- Scope is the union of saved keywords and research ideas belonging to the authorized project, filtered to one country/language. The project market is the default. Other projects' keyword lists never enter the scope.
- Sample the first 500 distinct keywords alphabetically. `scope_keywords` reports the full scope and `sampled_keywords` the bounded sample.
- Use the newest cached snapshot per keyword from the last 30 days, including empty results. Newer metasearch snapshots replace older exact snapshots. Each keyword includes provider, exact flag and timestamp.
- Only positions 1–30 count. Group by registrable domain using existing public-suffix rules; choose the best position once per domain/keyword. Subdomains roll up to the same domain.
- `checked_keywords` includes recent empty snapshots; missing/expired snapshots are unchecked. Coverage includes keywords where the requested domain was absent.
- Return observed keyword count, position distribution, best-result URLs, domains sharing observed keywords, and keyword gaps against the project's website. Gaps mean absence from the sampled page; they do not prove a domain does not rank.
- Keywords, pages and gaps are capped at 100 rows each; competitors at 50. Totals/distribution use the full bounded sample. Pages count only each keyword's best result URL.
- SeoSeo Rank uses the separately imported Common Crawl graph and its source/release metadata. Monthly traffic remains `null`. The domain sample has source `project_serp_sample` and confidence `relative`; it is not a comprehensive domain index.

SERP inspection is read-only and makes no external requests. Build coverage with keyword research or saved-keyword refresh, then reload evidence. No new migrations or credentials are required.

## Domain registration age (DO-07)

Click **Check registration age** after inspecting a domain to request public registry metadata. This action is separate from SERP inspection; it also works when the project has no keyword evidence. `POST /api/v1/projects/{project_id}/domain-age` accepts `{ "domain": "example.com" }`, or `{}` to use the project website. MCP `get_domain_age` takes `project_id` and optional `domain` (68 tools total).

Discovery uses the fixed [IANA DNS bootstrap](https://data.iana.org/rdap/dns.json), matching the longest domain-label suffix per [RFC 9224](https://www.rfc-editor.org/rfc/rfc9224.html). Only public HTTPS registry endpoints on port 443 are supported, with guarded DNS, no proxies, no redirects/referrals, a 1 MiB response limit, 15-second request timeouts and one-second pacing. The domain's own website is never fetched. HTTP-only registries and registries requiring redirects are reported as unsupported or unavailable; 403/429 stop the lookup.

Only a matching domain object and its top-level `registration` events from [RFC 9083](https://www.rfc-editor.org/rfc/rfc9083.html) are used. Last-changed, transfer and expiration dates never substitute for registration. Invalid/future registration dates are ignored; if multiple valid events exist, the earliest is used. This is the registry's reported registration date, not proof of website age or continuous ownership. No record (404), missing date and unsupported service leave `age_days` null; a 404 does not establish domain availability.

The existing shared provider cache stores the IANA bootstrap and minimal parsed registration records for 24 hours, including negative/unsupported results. No raw RDAP response, entity/contact data or registrar details are stored. Age is recalculated in whole days at read time; no forced-refresh bypass is exposed. Network, malformed-response and block errors are surfaced without caching a false unknown record. Every read authorizes the project before using the shared cache or network. No new schema or operator credentials are needed.

## API and MCP

`GET /api/v1/projects/{project_id}/domain-overview?domain=example.com&country=US&language=en`

All query parameters are optional; domain defaults to the project website. A project without a website must supply `domain`. Domains and HTTP(S) website URLs normalize to registrable domains. Members can read, including archived projects; cross-organization access returns 404.

MCP `get_domain_overview` takes `project_id` and the same optional query fields, returning the same result in the standard envelope.

## Verification

`cargo test -j 1 --workspace` covers aggregation, deduplication, gaps, unknown metrics, normalization, RDAP discovery/date parsing, response limits and HTTP block/redirect handling. `python scripts/e2e-competitive.py http://127.0.0.1:3401 seoseo-m5-db` targets a disposable API/database with password auth and open signup. It seeds synthetic SERPs and cache metadata through the named database container and verifies latest/empty/expired snapshots, research-only scope, market/project/tenant isolation, validation, the 500-keyword bound, domain-age caching/expiry/unknown handling and REST/MCP parity. Never run this fixture script against production.
