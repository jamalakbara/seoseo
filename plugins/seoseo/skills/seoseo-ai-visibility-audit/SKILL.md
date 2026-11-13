---
name: seoseo-ai-visibility-audit
description: "Audit SeoSeo AI samples, engine citations, literal brand mentions and coverage-aware trends."
---

# SeoSeo Ai Visibility Audit

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `list_ai_visibility_trackers`, `get_ai_visibility_tracker`, `get_ai_visibility_run`, `get_ai_visibility_summary`, `list_ai_visibility_models`, `lookup_ai_brand`, `explore_ai_prompts`.

Prefer saved runs for read-only audits. Explicit new collection can use create_ai_visibility_tracker and run_ai_visibility_tracker after inspecting approved engines. Report successful/expected/failed counts; unsaved cells may be pending, interrupted or unattempted. Mention rate uses successful answers; share of voice uses tracked brand-answer matches, not total market visibility. Text URLs are unverified; structured citations retain provider provenance. API samples are not consumer UI measurements.

Search the current website's retained index with `lookup_ai_brand` and `explore_ai_prompts` for read-only brand/prompt questions. This is project-owned model evidence in a frozen market, not a global prompt-demand source. Missing/deleted history and unchecked cells cannot establish market-wide absence.
