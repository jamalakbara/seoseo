# SeoSeo agent workflows

This package adapts the twelve OpenSEO workflow categories to the actual self-hosted SeoSeo MCP interface, plus backlink monitoring and rank-tracking workflows. Skills use the **SeoSeo** server; they do not route requests through the separately installed OpenSEO credit-based connector.

Connect the client to your instance's `/mcp` endpoint with a workspace API key or MCP OAuth. Copy selected directories from `skills/` into the client's supported skill directory, or point a client with directory-based discovery at this package. No global client configuration is modified by this repository. Each directory contains a self-contained `SKILL.md` and optional Codex display metadata in `agents/openai.yaml`.

The skill names use a `seoseo-` prefix to avoid collisions with OpenSEO skills. Invoke a workflow by its full name, such as `$seoseo-keyword-research` or `$seoseo-ai-visibility-audit`. The runtime MCP schemas remain the source of truth for field names and capabilities.

Available categories: site audit, keyword research, keyword clustering/page mapping, competitor analysis, competitive landscape, link prospecting, local SEO evidence review, AI prompt research, AI visibility audit, project setup, HTML reporting, coaching, backlink monitoring and rank tracking. Keyword clustering uses the read-only cached-SERP tool; reviewed page assignments are returned to the user or a requested report. Local Maps/grid functionality remains M6 work; those skills explicitly limit their current behavior to available evidence rather than inventing tools.

Validate the package with `python scripts/validate-agent-skills.py` from the repository root. It checks frontmatter, display metadata and tool references against the actual MCP implementation. Skills retain the user's existing authorization; they do not independently authorize messages, sharing, recurring cloud usage or unrelated mutations.

Workflow categories were inspired by the MIT-licensed OpenSEO project; its license is reproduced in `UPSTREAM-LICENSE`.
