---
name: seoseo-local-seo
description: "Assess SeoSeo evidence for a local business without inventing Maps or grid measurements."
---

# SeoSeo Local Seo

Use the connected **SeoSeo** MCP server, not the similarly named OpenSEO credit-based connector. Inspect live tool schemas for arguments and filters. Resolve project IDs with `list_projects` and read `get_project_context` when relevant. Envelopes are `{ok,data}` or `{ok,error}`; inspect errors and partial coverage. Existing user authorization persists; the skill does not authorize unrelated writes, sharing, recurring cloud requests or messages.

Tools: `get_project_context`, `search_local_businesses`, `list_local_categories`, `list_local_trackers`, `get_local_grid_preview`, `list_local_scans`, `get_local_scan`, `list_city_demand`, `get_search_console_performance`.

Ground the work in the website, service area and language. Use explicit cached OpenStreetMap discovery to find candidate business identities; OSM IDs and discovery order are not Google place IDs or Maps positions. Never use discovery for autocomplete or grid scans.

Read saved scans first. Inspect provider/import provenance, frozen grid/keywords/depth, exact target ID, observation time and measured/expected coverage. Geometry previews contain no measurements. Failed and unchecked points remain unknown. ARP averages found positions; ATRP assigns depth+1 to non-returned targets over measured points; SoLV is top-three coverage among measured points. Compare only matching source and configuration with comparable coverage. Per-point results are bounded samples, not exhaustive nearby competitors.

Explicitly requested new collection can use `create_local_tracker` and `start_local_scan`; inspect schemas and the budget first. Live scans require enabled SerpApi BYOK, an exact provider target ID and a reviewed keyword-count × grid-point request count (at most 250). `import_local_scan` saves source-labelled exported/manual evidence without provider requests. Do not invent ranks or fuzzy-match names to provider IDs.

Use `list_city_demand` for source-labelled city volume/interest context. `import_city_demand` saves an explicit source export when requested. Missing city searches stay unknown; relative interest is normalized within its source export and is not searches/month. Never scale a national estimate into a city figure. Business Profile reviews, Q&A and updates remain outside this release.
