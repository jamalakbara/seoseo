# Local business discovery, scans and city context

Use the project **Local SEO** page to find an OpenStreetMap business, save a tracker with 1–50 unique keywords, and preview a 3×3, 5×5, 7×7 or 9×9 grid with 0.1–10 km point spacing. There are at most ten trackers per project. Discovery is explicit, cached for 30 days, attributed, and globally paced to one uncached request per second; it is never used for autocomplete or grid scans. The curated category list is not Google's taxonomy. Operators can change `NOMINATIM_URL` to an approved alternative endpoint.

## Rank collection and evidence imports

If discovery has no match, expand **Enter business details manually** and supply a verified name, address and coordinates. This saves an explicitly manual business identity without a lookup or rank request. Supply the separate exact Google provider ID when starting a scan.

Configure **Settings → Data providers → SerpApi (Maps and AI Overviews)** with an encrypted BYOK key and enable it explicitly. Collection is off by default and is separate from organic SERP fallback. Saving a tracker makes no rank requests.

A scan freezes the tracker geometry, selected saved keywords, country/language, exact target provider ID, depth, requester and source. Review the budget before starting: keywords × grid points, at most 250 cells. One scan can be active per tracker, and twenty runs can be retained; delete old inactive runs to free space. A durable worker queries the first Google Maps result page at each point at zoom 14, with the requested search-origin coordinates. This is a provider sample, not an exhaustive list of nearby businesses. See [SerpApi Maps documentation](https://serpapi.com/google-maps-api).

Enter the exact Google `place_id` (or `data_id` when the response has no place ID). It is different from the OSM ID. No fuzzy name matching is used. Each response retains returned positions and identities, up to 20 competitors. A successful response with no target means not returned in that sample; a failed or unchecked point remains unknown.

The worker records a cell attempt before making its request. On crash recovery, recorded cells are skipped, including an interrupted request whose result was not saved. This keeps retries from spending twice on the same cell; interrupted cells stay unknown. A provider failure stops the remaining scan, including 429, access rejection and malformed results. There is no retry around a block. The worker rechecks project access, requester membership and provider configuration before collection. Terminal queue failures are reconciled to failed scans while retaining evidence.

The free path is **Import local observations**. Supply source-labelled measured results from an export or manual observation; no provider requests or paid key are needed. The server validates saved keywords, grid bounds, dates, unique cell coordinates, unique competitor IDs/positions, depth and safe website links. Imports commit their status and cells atomically. Omitted cells stay unchecked.

```json
{
  "target_business_id": "example-place-id",
  "keywords": ["coffee"],
  "depth": 20,
  "observed_at": "2026-11-01T00:00:00Z",
  "source_note": "Manual Google Maps observation at the frozen grid point",
  "cells": [{
    "keyword": "coffee", "row": 0, "column": 0, "measured": true,
    "checked_at": "2026-11-01T00:00:00Z", "error": null,
    "results": [{"position": 2, "business_id": "example-place-id", "name": "Cafe", "address": null, "website": null}]
  }]
}
```

## Reading maps and metrics

Select a saved run and keyword. The accessible heatmap and optional MapLibre map distinguish ranks 1–3, 4–10, 11–20, not returned, and unchecked/failed points. Clicking a point shows that cell's competitors. Source, timestamp, status, depth and coverage remain visible. JSON export includes frozen configuration and evidence.

- **ARP**: mean position among points where the target was found; unknown if never found.
- **ATRP**: mean over measured points, assigning depth+1 to non-returned targets. This is an explicit penalty, not an observed rank.
- **SoLV**: percentage of measured points where the target was in the top three.

Failed and unchecked points are excluded from all metric denominators. Always compare coverage and identical grids, keywords, depths and sources. Missing evidence is not a zero score or verified ranking loss.

The geographic map loads only on explicit display. It uses [OpenStreetMap tiles](https://operations.osmfoundation.org/policies/tiles/) with visible attribution, browser caching and referrer, no bulk download or offline prefetch. Set Compose `MAP_TILE_URL` and rebuild web, or set build-time `NEXT_PUBLIC_MAP_TILE_URL`, to choose an approved tile template (`none` disables base tiles for offline browser QA). The numeric grid works without tiles or WebGL.

The frontend dev/build scripts copy the installed MapLibre worker and its shared module into ignored `public/maplibre` assets. The map explicitly uses this same-origin, version-matched worker because bundling relocates MapLibre's entry module.

## City demand context (KW-10)

Import a source export through the Local SEO page or REST/MCP. Provide city, source/method, start/end dates and up to 500 unique keyword rows. Each row can contain explicit city searches/month, national searches/month and/or a normalized city relative-interest value from 0–100. An empty city search count remains null; an explicit zero remains zero. Relative interest is not search volume and is comparable only within the source export's normalization. National volume is never multiplied into an invented city estimate.

City datasets are scoped to the current project website, country and language. Changing those hides older data; restoring the exact scope restores retained data. The most recent 100 datasets per research kind are retained across website identities. Importing does not replace saved-keyword volume or trigger external collection.

## API and MCP

All REST endpoints are under `/api/v1`:

- `GET /local/categories`
- `POST /projects/{project_id}/local/businesses/search`
- `GET/POST /projects/{project_id}/local-trackers`
- `GET /projects/{project_id}/local-trackers/{tracker_id}/grid`
- `GET/POST /projects/{project_id}/local-trackers/{tracker_id}/scans`
- `POST /projects/{project_id}/local-trackers/{tracker_id}/scans/import`
- `GET/DELETE /projects/{project_id}/local-trackers/{tracker_id}/scans/{run_id}`
- `GET/POST /projects/{project_id}/city-demand`

MCP tools: `list_local_categories`, `search_local_businesses`, `create_local_tracker`, `list_local_trackers`, `get_local_grid_preview`, `start_local_scan`, `import_local_scan`, `list_local_scans`, `get_local_scan`, `import_city_demand`, `list_city_demand`. Writes require member role; every operation checks project access. Live start requires an exact reviewed request count; use saved history for read-only research.

Provider endpoints are fixed defaults or operator environment settings; project input cannot supply endpoints. `SERPAPI_URL` exists only for operator-approved proxies and offline fixtures. See [M6 acceptance](27-m6-acceptance.md).
