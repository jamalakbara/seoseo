---
name: seoseo-seo-project-setup
description: "Set up SeoSeo project context from a user brief and reviewed website evidence while preserving revisions."
---

# SeoSeo Seo Project Setup

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `list_projects`, `create_project`, `get_project_context`, `update_project_context`, `research_project_website`, `get_project_website_setup`, `save_project_website_setup`.

Find the intended project before creating another. Use provided website/market/positioning/goal; ask only for material missing information. Poll website research and inspect evidence before treating it as fact. Use current revision and expected website on updates so stale changes cannot overwrite another agent. Keep model drafts and inferred goals distinct from confirmed canonical context.
