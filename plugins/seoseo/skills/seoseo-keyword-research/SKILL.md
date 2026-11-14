---
name: seoseo-keyword-research
description: "Research and prioritize keywords using SeoSeo saved terms, free providers and observed SERPs."
---

# SeoSeo Keyword Research

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `research_keywords`, `list_keyword_research`, `get_keyword_research`, `list_saved_keywords`, `get_serp_results`, `save_keywords`.

Start from project market and saved terms. Poll research completion, inspect intent/provider/coverage, and compare real top results for promising questions. Wikipedia pageviews are an entity-demand proxy, not measured search volume; missing volume/CPC stays unknown. Save selected terms/tags when the user requested saving; otherwise present a shortlist and page opportunities.


For city context use `list_city_demand`; source-labelled city exports can be saved with `import_city_demand` when requested. Distinguish explicit city searches/month from relative-interest scores; absent city counts remain unknown and national volume must not be scaled into a made-up city estimate.
