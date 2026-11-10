# 08 — Site Audit: How It Works (M1)

Design record for the shipped site audit. Read this before changing the crawler, rules or audit job.

## Flow

```
POST /projects/{id}/audits ──► audits row (queued) + jobs row (site_audit)
                                   │
worker claims job ─► probe ─► discover ─► crawl ─► analyze ─► completed
                      │          │          │          │
                      │          │          │          └ link graph → inlinks, click depth (BFS),
                      │          │          │            site rules, issue counts, health score
                      │          │          └ frontier batches (depth-first order by depth,url),
                      │          │            robots check, fetch (concurrency N, delay), extract,
                      │          │            page rules, store page+links+issues in one tx
                      │          └ robots.txt sitemaps (or /sitemap.xml), nested ≤3 levels/300 docs
                      └ fetch start URL; follow redirects (http→https, apex→www) to set scope
UI: GET …/audits/{id}/events (SSE, 1 s DB poll) with 5 s polling fallback
```

## Where the code lives
| Concern | File |
|---|---|
| SSRF guard (URL + resolved IP) | `backend/crates/crawler/src/policy.rs` |
| Fetcher (manual redirects, caps, classification) | `backend/crates/crawler/src/fetch.rs` |
| robots.txt / sitemaps | `crawler/src/robots.rs`, `crawler/src/sitemap.rs` |
| HTML extraction (lol_html, streaming) | `crawler/src/extract.rs` |
| Issue catalog, page rules, site rules, score | `crawler/src/rules.rs` |
| Orchestration (phases, resume) | `backend/crates/jobs/src/site_audit.rs` |
| Persistence | `backend/crates/db/src/audits.rs`, migration `0002_site_audit.sql` |
| API | `backend/crates/api/src/routes/audits.rs` |
| UI | `frontend/app/(app)/p/[projectId]/audit/**` |

## Invariants
- **SSRF**: every URL passes `UrlPolicy::check_url`; the reqwest DNS resolver drops private/reserved IPs, so redirects and DNS rebinding cannot reach internal hosts. `CRAWLER_ALLOW_PRIVATE=true` exists only for the `badseo` test overlay.
- **Resumable**: frontier, pages, links and issues live in Postgres. Pages are unique per `(audit_id, url_hash)` and inserted with their links/issues in one transaction, at most once. Site rules are deleted and recomputed in `analyze`. A killed worker's job is requeued when its lease expires (≤ ~3 min) and continues from the frontier.
- **Redirecting URLs** are stored with their chain but no content, links or content issues; the in-scope destination is crawled as its own page.
- **Click depth** comes from BFS over the stored link graph in `analyze`, not from discovery order (sitemap URLs would otherwise look shallow). Unreachable pages have depth `-1`.
- **Budgets**: `max_pages` (≤10 000), frontier capped at 5× pages, 2 h wall clock, stop after 3 consecutive 429s, robots `Crawl-delay` honoured (≤10 s), HTML parsed up to 1 MiB, response read up to 8 MiB, links/images ≤1000 per page.
- **Thresholds** match OpenSEO: title 10–60, description 70–160, thin < 150 words, deep ≥ 5 clicks, slow > 1.5 s.

## Issue catalog (38 types)
Served at `GET /api/v1/audit-issue-types` with explanation and fix text. Critical: blocked-page, server-error, broken-internal-link, missing-title, redirect-loop. Warnings include duplicates, thin content, missing description/H1, canonical conflicts, orphan pages, mixed content, invalid JSON-LD, JS-rendering suspected. Notices include title/description length, heading skips, noindex, canonicalized, deep pages, slow responses, robots-disallowed, sitemap-non-indexable.

Health score = 100 − Σ(affected-page share × weight), weights critical 30 / warning 8 / info 1.

## Testing
- Unit: `crawler` crate (URL policy, normalization, robots, sitemap parsing incl. entities/CDATA, extraction, every rule, score).
- Fixture: `deploy/test/badseo/` (nginx) with one page per defect; `deploy/compose.test.yaml` adds it.
- E2E: `scripts/e2e-audit.sh` asserts all 26 fixture issue types are found and a clean page has none; `scripts/e2e-team.sh` covers invitations.
- Crash-resume was verified manually by SIGKILLing the worker mid-crawl (job attempt 2, identical results, no duplicate pages).

## Known gaps (later milestones)
JavaScript rendering + Lighthouse (M2), hreflang validation, soft-404 detection, URL hygiene and asset checks (SA-06), audit comparison and scheduled audits (SA-13), internal-link graph view (SA-12), per-host adaptive throttling.
