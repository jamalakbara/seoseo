# Stored evidence research: cannibalization and AI index (RT-07, AI-05)

These read-only features use existing project data. They require no configured search/model provider, do not create jobs, and never call external engines. Members can read them, including archived projects. Another project or tenant's evidence is not included.

## Multi-URL candidates

The **Rank tracking** page now offers **Cannibalization candidates**. Choose a country/language and click **Find competing URLs** to see keywords where multiple distinct URLs from the project registrable domain appeared in the same cached search results. Each candidate includes best observed positions, provider, exactness and snapshot time.

`GET /api/v1/projects/{project_id}/cannibalization`

MCP `get_cannibalization_flags` accepts project_id and optional country, language, limit and offset. Market defaults to the project. Limit is 1–100 (default 25). The project website is required.

Scope is the existing union of saved/researched/tracked/imported keywords in the selected market, capped at the first 500 alphabetically. Full scope size, sampled, checked, matched and flagged counts remain visible independently of pagination. The newest snapshot within thirty days is authoritative, including empty result sets. Only positions 1–30 participate. URL parsing normalizes hosts/default ports, removes fragments and rejects credentials/non-HTTP(S) URLs; query strings and distinct paths remain separate. Duplicate results collapse to one URL at its best position. Actual result URL hosts must belong to the project registrable domain, including subdomains; a provider's domain field alone is not trusted.

Multiple useful search results from one site can be legitimate. Candidates do not establish harmful cannibalization, simultaneous competition in Google when the provider is aggregated, or a reason to merge pages automatically. Unchecked/stale keyword evidence remains unknown. Review intent and page purpose before editing.

## Your prompt and brand index

The **AI visibility** page now has **Your prompt and brand index**. Search retained successful answer text by literal brand, optionally filter saved prompt text by topic, and choose saved prompts or answer excerpts. Market controls apply to the frozen run country/language, not the current model configuration. The index remains usable when model providers are no longer configured.

`GET /api/v1/projects/{project_id}/ai-index`

Optional parameters: view=prompts (default) or answers, brand (1–80 characters), topic (1–120), country, language, limit (1–100, default 25) and nonnegative offset. Blank/control-character search terms and invalid ranges/markets receive 422. Omit brand/topic to browse the retained index.

MCP `explore_ai_prompts` returns grouped saved prompts with optional brand/topic filters. MCP `lookup_ai_brand` requires a brand and returns matching successful saved-answer excerpts. Neither generates prompts or calls models. This is the project's own retained evidence, not a global prompt-demand or consumer-chat database.

The repository joins the current project website identity to its trackers, then expands frozen run prompts/models into expected answer cells. Deleted history and previous-website trackers are outside the index. Saved answers from incomplete/failed runs remain valid observations; failed cells and unattempted cells are reported separately. Extra answer rows outside a frozen run configuration do not enter its denominator.

Summary counts describe the selected market/topic scope before filtering to brand matches. Mention rates use successful saved answer cells only and stay null with no successful observations or no requested brand. A failed/unattempted cell is not a measured absence. Grouped prompts count matching retained runs and expected/successful/failed/unchecked cells. Brand-filtered prompt results require at least one matching saved answer; unfiltered prompt browsing also includes prompts with no saved successful answer.

Brand terms are regex-escaped and matched case-insensitively with alphanumeric boundaries. Topic filters use literal case-insensitive substrings, escaping LIKE wildcard characters. Input does not become a regex or SQL expression. Answer excerpts are capped at 500 characters around the first literal-boundary match (or the beginning when browsing without a brand). Engine, grounding, timestamp and gateway fingerprint preserve provenance. Text-only answers remain text-only observations. Queries share a repeatable-read read-only snapshot so summary, totals and paged results are consistent.

## Verification

`scripts/e2e-evidence-research.py API_ORIGIN M6CHECK_DB` seeds synthetic SERP/model-answer evidence in a disposable password-auth/open-signup stack. It checks duplicates/fragments, foreign URLs, stale/empty/depth evidence, scoped counts, success/failure/unchecked denominators, literal/Unicode matching, regex and LIKE escaping, frozen markets, previous websites, project/tenant/member/archive isolation, paging, all three MCP tools and absence of new jobs. Database names must start with seoseo-m6check; never run against a normal deployment.

`scripts/e2e-evidence-research-browser.cjs WEB_ORIGIN M6CHECK_DB OUTPUT_DIRECTORY` uses a temporary puppeteer-core installation through PUPPETEER_MODULE and local Chrome. It verifies source/position labels, brand/prompt controls, denominators and excerpts, desktop/mobile layouts and no new SERP/model calls. It saves screenshots for visual review.

Verified on 2026-11-13: all 88 workspace tests, formatting, workspace/all-targets clippy with warnings denied, app build, frontend typecheck/lint/production build, API and desktop/mobile browser regressions passed. Screenshots were visually reviewed. All 14 portable skills validate against 95 MCP tools.

These features add no migration. The M5 live-cloud acceptance gate and remaining M6 integrations/local SEO work are separate.
