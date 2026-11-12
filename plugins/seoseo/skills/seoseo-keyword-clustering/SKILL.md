---
name: seoseo-keyword-clustering
description: "Group SeoSeo keywords by intent and page purpose to produce an evidence-labelled page map."
---

# SeoSeo Keyword Clustering

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `list_saved_keywords`, `get_keyword_research`, `get_serp_results`.

Deduplicate variants without merging distinct intents. Use a small number of SERP checks when shared ranking URLs would materially change a cluster. Label semantic-only groups separately from SERP-supported groups. Automated clustering/storage is not exposed yet: return the mapping or a requested report; do not invent a cluster-save tool.
