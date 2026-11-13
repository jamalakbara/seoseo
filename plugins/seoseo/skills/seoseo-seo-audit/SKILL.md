---
name: seoseo-seo-audit
description: "Audit a website with self-hosted SeoSeo and prioritize observed technical fixes."
---

# SeoSeo Seo Audit

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `run_site_audit`, `list_site_audits`, `get_audit_status`, `get_audit_issues`, `get_audit_pages`, `get_internal_link_graph`, `compare_site_audits`, `get_audit_schedule`, `set_audit_schedule`.

Inspect prior audits for a read-only review; start a fresh crawl only when requested or necessary for the authorized audit. Poll the durable audit before interpreting issues. Distinguish blocked/unread pages from pages checked without findings. Prioritize by actual affected URLs and business relevance, retaining the requested crawl budget and robots policy.

For a completed audit, inspect `get_internal_link_graph` before proposing internal links. Distinguish full stored-audit source counts from the bounded graph sample; no observed inlinks never proves website-wide orphan status. Suggestions require eligible pages and shared title/H1 words, exclude existing links including nofollow, and need editorial review. They do not automatically edit pages. If graph edges are capped, suggestions are disabled; report that coverage limit.

Use `compare_site_audits` for completed audits with the same start URL. Missing URLs and failed collection never prove deletion or a resolved issue. Preserve unconfirmed absences and configuration/coverage warnings. `get_audit_schedule` is read-only. Use `set_audit_schedule` only when recurring crawls are explicitly authorized, after reviewing the baseline budget and robots/render settings; use the current revision and website identity. Manual disables the configured schedule.
