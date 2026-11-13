---
name: seoseo-rank-tracking
description: "Create or inspect SeoSeo rank trackers and interpret histories, providers and unknown positions correctly."
---

# SeoSeo Rank Tracking

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `list_rank_trackers`, `get_rank_tracker`, `create_rank_tracker`, `add_rank_tracking_keywords`, `estimate_rank_tracker_cost`, `run_rank_tracker`, `get_rank_history`, `get_cannibalization_flags`.

Respect domain, project market and keyword scope. New trackers/runs follow requested tracking work, not a read-only report. Estimate provider requests, poll completion and inspect exactness, depth and partial failures. A null position in a measured row means absent from checked depth; no row means not checked. Do not promise Google/device/city precision unsupported by the chosen provider.

Use `get_cannibalization_flags` to inspect same-snapshot multi-URL candidates without new searches. Preserve provider/exactness, date and sampled/unchecked coverage. Multiple URLs can be legitimate; do not call them proven ranking harm or recommend merging pages without reviewing intent.
