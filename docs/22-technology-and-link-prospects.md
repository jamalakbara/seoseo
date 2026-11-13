# Homepage technology and sampled link prospects (DO-06, BL-07)

## Homepage technology hints

The **Competitive** page now offers an explicit **Inspect homepage technology** action. It observes public markup for WordPress, Next.js, Nuxt and Shopify hints. Results identify fingerprints, not verified technology versions, backend services, vulnerabilities or a complete stack. A site may customize or hide these hints, and another site can imitate them.

`POST /api/v1/projects/{project_id}/technology` / MCP `detect_technologies`

Input accepts an optional homepage URL or hostname; the project's HTTPS root is the default. Authorization precedes shared-cache or network access. HTTP(S) roots only: no credentials, non-root path, query parameters or custom ports. The normal URL/IP/DNS guard applies; private access remains an explicit operator-only test setting. Custom ports are rejected before normalization.

The lookup uses an honest SeoSeoBot User-Agent and checks exact robots.txt first. Only successful plain/non-HTML robots or missing robots (404/410) permit proceeding. Disallow, blocks, rate limits, inconclusive access or crawl delays over five seconds stop the lookup. Short declared delays are respected. It then fetches one exact homepage with a 1 MiB body cap and ten-second request timeouts. Redirects are returned as observations without following them. Bot challenges stop without retries, cookie handshakes, browser impersonation or bypasses. Referenced assets are inspected as markup strings; they are not downloaded or executed.

Meta-generator declarations and selected script/link/image asset paths are parsed as structured HTML rather than searched in article text. Shopify asset hosts must match exactly, not a lookalike suffix. Signals are fixed labels, with at most four per supported platform. Raw HTML, cookies, response headers and asset-query tokens are not retained.

Positive, unknown, redirected, robots-disallowed and blocked records are cached for 24 hours by canonical homepage URL. Cache hits make no network requests. Database-backed one-minute project and URL cooldown claims bound concurrent uncached requests without holding a connection while fetching. A crashed lookup's claim expires; there is no user refresh bypass. Unknown/empty results do not prove technology absence.

The signature references are the official descriptions of [WordPress content directories](https://developer.wordpress.org/advanced-administration/wordpress/wp-config/), [Next.js asset prefixes](https://nextjs.org/docs/app/api-reference/config/next-config-js/assetPrefix), [Nuxt build asset directories](https://github.com/nuxt/nuxt/blob/main/docs/4.api/6.nuxt-config.md) and [Shopify CDN assets](https://shopify.dev/docs/storefronts/themes/best-practices/performance/use-shopify-cdn). These paths can be customized or moved; they support hints only.

## Link-intersect prospects

The **Backlinks** page lets a member choose one to five accessible competitor projects, including archived projects, and compare their current-website backlink samples with this project's samples. Create competitor projects and use existing discovery/import workflows to populate evidence first. No new discovery, contact lookup, outreach or messages occur during comparison.

`POST /api/v1/projects/{project_id}/link-intersect` / MCP `find_link_intersect`

Input fields: competitor_project_ids, optional min_competitors (default 1, at most the number selected), limit (1–100, default 25), offset (nonnegative). IDs must be distinct, exclude this project, and belong to the current workspace. All selected sites need distinct registrable domains; duplicate aliases of the same site are rejected. Another tenant/project outside the caller's scope returns 404. Website identities are rechecked inside a read-only repeatable-read transaction; a concurrent website change returns 409.

For each current website, the repository selects the latest snapshot per dataset/source, including empty snapshots. It then samples the ten newest dataset/source snapshots per project. Historical website snapshots and lost sample rows are excluded. Scope metadata exposes available/sample dataset counts and snapshot date ranges; absent or omitted evidence has explicit warnings.

Prospects are referring source domains seen in at least the selected number of competitor samples and not seen in the current site's selected samples. The current site's own registrable domain is excluded. Competitor counts deduplicate project IDs; observed links deduplicate project/source/target pairs across datasets. At most three source URLs per prospect are returned, with verified/imported evidence counts and newest sample time. Category counts can overlap when one pair has evidence in multiple datasets. Results sort by competitor count then source domain; total counts remain independent of pagination.

“Not observed in these samples” does not establish that the live site lacks a link. Imported and inconclusive observations are not independent verification. This is a bounded free-first link-intersect workflow over user-imported and guarded-discovery evidence, not an exhaustive Common Crawl or commercial backlink index. Review source relevance and live evidence before any outreach.

## Verification and related fixes

The parser regression distinguishes actual generator/asset markup from article text and lookalike hosts. The disposable API regression additionally checks robots/bot/redirect stops, positive/negative cache hits with no extra requests, concurrent cooldown claims, URL bounds, scoped latest/empty backlink samples, baseline exclusion, dataset caps, identities, tenant/member/archive access, paging, MCP parity and no jobs.

```powershell
python scripts/e2e-tech-and-prospects.py http://127.0.0.1:3401 seoseo-m6check-db seoseo-m6check-tech-fixture
$env:PUPPETEER_MODULE = 'PATH_TO_TEMP_INSTALL/node_modules/puppeteer-core'
node scripts/e2e-tech-and-prospects-browser.cjs http://127.0.0.1:3400 seoseo-m6check-db OUTPUT_DIRECTORY
```

These scripts require a disposable password-auth/open-signup API with private crawler access and the nginx fixture at deploy/test/m6-tech/nginx.conf. Give the fixture aliases good.tech-fixture.example, robots.tech-fixture.example, bot.tech-fixture.example, robotblock.tech-fixture.example, redirect.tech-fixture.example and cooldown.tech-fixture.example. Only container names beginning with seoseo-m6check are accepted. Never target a normal deployment. The script resets only technology-cache keys in that disposable database when rerun.

Browser verification covers explicit homepage hints/cache, competitor/minimum selection, imported provenance, sample dates, desktop/mobile layouts and no discovery jobs. Deferred pages now wait for a stable client-ready fallback before reading layout-populated query cache, preventing hydration mismatches. Discovery/import component keys are unique.

Testing also found and corrected the crawler normalizer's custom-port bug: Url already removes default ports, while port_or_known_default also returns explicit custom ports. Custom ports now remain part of URL identity. Existing stored audit records are unchanged; the regression verifies explicit port 8080 is retained. Homepage technology lookup still rejects custom ports intentionally.

Verified on 2026-11-13: all 89 workspace tests, formatting, workspace/all-targets clippy with warnings denied, app build, frontend typecheck/lint/production build, API regression and repeated desktop/mobile development-browser checks passed. Screenshots were visually reviewed. All 14 skills validate against 97 MCP tools. A separate local production-preview startup was rejected by automatic approval review with only “blocked by policy” supplied, so no production-preview browser result is claimed.

This batch adds no migration. The combined M6 branch still includes SA-13 migration 0015. Remaining M6 data integrations/local SEO and the M5 live-cloud gate remain separate.
