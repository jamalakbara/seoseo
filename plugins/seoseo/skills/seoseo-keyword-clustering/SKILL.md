---
name: seoseo-keyword-clustering
description: "Group SeoSeo keywords by intent and page purpose to produce an evidence-labelled page map."
---

# SeoSeo Keyword Clustering

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `cluster_keywords`, `list_saved_keywords`, `get_keyword_research`, `get_serp_results`.

Start with `cluster_keywords` for a read-only view of cached project evidence. It samples up to 500 keywords in one country/language, uses snapshots from the last seven days and compares distinct top-ten URLs. Every pair in a group must share min_shared URLs (2–10, default 3); providers and exactness never mix. Review coverage, unavailable keywords and ungrouped results. The alphabetical representative is not a demand or priority recommendation.

Deduplicate variants without merging distinct intents. Use a small number of explicitly authorized SERP checks when shared ranking URLs would materially change a cluster, then call `cluster_keywords` again. Label semantic-only groups separately from SERP-supported groups. Return the reviewed page mapping or a requested report; cluster storage is not exposed, so do not invent a cluster-save tool.
