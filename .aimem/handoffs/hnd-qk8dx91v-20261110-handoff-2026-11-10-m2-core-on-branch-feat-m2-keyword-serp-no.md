---
aimem: 1
id: hnd-qk8dx91v
type: handoff
title: "Handoff 2026-11-10: M2 core on branch feat/m2-keyword-serp (not committed): new crates `providers` (Google/Bing autocom…"
summary: "Done: M2 core on branch feat/m2-keyword-serp (not committed): new crates `providers` (Google/Bing autocomplete, SearXNG/Brave/Serper SERP, Wikipedia pageviews, Common Crawl rank stream, catalog, regi…"
status: final
trust: agent
roles: [all]
author: agent:claude-code:fable-5.2
created: 2026-11-10T17:15:28Z
updated: 2026-11-10T17:15:28Z
branch: feat/m2-keyword-serp
---

## Done
M2 core on branch feat/m2-keyword-serp (not committed): new crates `providers` (Google/Bing autocomplete, SearXNG/Brave/Serper SERP, Wikipedia pageviews, Common Crawl rank stream, catalog, registry) and `scoring` (SR, KD, intent, popularity, volume buckets, PSL domains); common::crypto (AES-256-GCM SecretBox); migration 0003 (provider_settings, provider_cache, serp_snapshots/items, domain_ranks, keyword_researches, keyword_ideas, saved_keywords); jobs keyword_research (ideas -> demand -> serp) and cc_rank_import; domain + API for providers (list/update/test), authority (status/import/lookup), keyword research, ideas (filters/sort/CSV), saved keywords (tags/delete/CSV), serp-analysis; frontend Keywords tab (research form + history, results table, SERP dialog), saved keywords page, Settings -> Data providers; compose `serp` profile with SearXNG; scripts/e2e-keywords.sh. Verified: clippy clean, 49 unit tests, frontend lint/typecheck/build, smoke, e2e-audit, e2e-team, e2e-keywords 39 checks on a fresh stack with SearXNG (154 ideas for "standing desk", KD computed, SR import 20k), UI checked in browser.

## Not done
Commit. M2 leftovers: SA-06 rest (hreflang, soft-404, URL hygiene, asset checks), SA-07 JS rendering, SA-08 Lighthouse (renderer container), opt-in Chromium SERP adapter, bulk refresh job for saved keywords (KW-09), Google Trends (dropped: needs bot-check workaround, see dec-8df1w3nw). Volume estimates exist only for keywords matching a Wikipedia article. e2e-keywords is not in CI (needs internet).

## Next
Commit M2 core. Then renderer container (chromiumoxide + Lighthouse CLI) for SA-07/SA-08, finish SA-06, bulk saved-keyword refresh. Then M3: rank tracking, GSC OAuth (gives measured volume anchors), MCP server.

## Blocked
none

## Notes for next agent
Docker Desktop must be running for scripts/cargo-docker.sh. The live user stack (project `seoseo`) needs a rebuild to get migration 0003; set APP_ENCRYPTION_KEY in .env before adding BYOK keys, and `--profile serp` + SEARXNG_URL=http://searxng:8080 for SERP/KD. Intent rules are English-first; "email marketing software" classifies as informational (no modifier) - candidates for an LLM pass.
