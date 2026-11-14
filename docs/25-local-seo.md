# Local business discovery and grid planning

The project Local SEO page supports explicit business discovery through OpenStreetMap/Nominatim, saving a tracker with 1–50 unique keywords, and previewing a 3×3, 5×5, 7×7 or 9×9 grid with 0.1–10 km point spacing. The project market supplies country and language. Each project can save up to ten trackers; tracker and keyword creation commit atomically under a project lock.

Discovery is submitted explicitly, cached for 30 days, and globally paced to one uncached request per second. A repeated uncached lookup has a cooldown. It must never be used for autocomplete or systematic grid queries. Results include OpenStreetMap attribution and source links. Operators may set `NOMINATIM_URL` on the API to an approved alternative endpoint; the default is `https://nominatim.openstreetmap.org/search`.

This release plans geometry only. No local-results provider runs scans. Every point has `rank: null` and `measured: false`; there are no competitor rankings, heatmap scores, ARP/ATRP/SoLV or rank history. Discovery order is not Google Maps ranking. Longitude wraps at the antimeridian.

REST endpoints (under `/api/v1`):

- `GET /local/categories`
- `POST /projects/{project_id}/local/businesses/search` with `query`, `near`, optional `category` and `limit` (1–10).
- `GET /projects/{project_id}/local-trackers`
- `POST /projects/{project_id}/local-trackers` with business identity/address/coordinates, keywords, optional name/category/grid_size/spacing_km. `business_source` supports `openstreetmap` or `manual`.
- `GET /projects/{project_id}/local-trackers/{tracker_id}/grid`

MCP tools: `list_local_categories`, `search_local_businesses`, `create_local_tracker`, `list_local_trackers`, `get_local_grid_preview`. Discovery and creation require member role; reads require access to the project.

Offline regression: run `scripts/local-business-fixture.py` in an isolated network, configure a disposable API with `NOMINATIM_URL=http://fixture:8080/search` and `ALLOW_SIGNUP=true`, then run `python scripts/e2e-local.py http://localhost:<api-port>`. This checks caching, safe website filtering, tenant/project isolation, validation, keyword normalization, antimeridian wrapping, concurrent quota enforcement and MCP preview. Never point the regression at a real geocoder.
