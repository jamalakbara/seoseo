---
name: seoseo-link-prospecting
description: "Find relevant link prospects from SeoSeo search and backlink evidence, and draft outreach when asked."
---

# SeoSeo Link Prospecting

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `get_serp_results`, `discover_backlinks`, `list_backlink_discoveries`, `get_backlink_snapshot`, `find_link_intersect`.

Judge topical relevance and actual source evidence rather than authority alone. Poll discovery and inspect its snapshot coverage. Imports are unverified and inconclusive checks are not proof of live links or loss. Spam flags are relative source-page heuristics, not penalties. Draft outreach grounded in the source; sending requires explicit user authorization and an available messaging tool.

Use `find_link_intersect` for a read-only comparison of one to five accessible competitor projects with distinct websites. Inspect sample scope/dates and the ten-dataset cap. A prospect is absent only from the selected own-site samples, not proven missing from the live site. No automatic contact lookup or outreach occurs.
