# 05 — Free Data Strategy (replacing DataForSEO)

OpenSEO's research features are 100% DataForSEO (paid). Our promise is **"completely free"**: free software, free to self-host, and **usable with zero paid API keys**. This doc maps every paid OpenSEO data need to a free source, and states honestly where free data is weaker.

## Principles
1. **Free-first, BYOK-optional.** Every data capability is a Rust trait (`Provider`) with a free default adapter. Users may plug in paid keys (DataForSEO, SerpApi, Serper, Brave paid, OpenAI…) to upgrade quality. Nothing in the product *requires* money.
2. **Compute, don't buy.** We own a crawler + link graph + SERP cache. Metrics like Keyword Difficulty, Domain Authority, traffic estimates are *computed* by us from raw signals, with documented formulas.
3. **First-party data beats estimates.** GSC (real impressions/clicks/positions), GA4, Bing Webmaster Tools are free and authoritative for the user's own sites — surfaced everywhere.
4. **Label confidence.** Every metric carries `source` + `confidence` (`measured` | `estimated` | `relative`). Unknown = `null`, never 0 (OpenSEO invariant).
5. **Respect ToS & robots.** Adapters that scrape third-party sites are opt-in, rate-limited, off by default, and documented with their legal caveats. The user chooses.
6. **Cache aggressively.** Shared SERP / keyword / domain caches with TTLs so repeated research costs nothing.

## Capability map

| Capability | OpenSEO (paid) | Our free default | Free/optional upgrades | Quality note |
|---|---|---|---|---|
| **Keyword ideas / suggestions** | Labs keyword_suggestions/ideas/related | Autocomplete expansion (Google, Bing, YouTube, Amazon, DuckDuckGo suggest) with a–z / question / preposition modifiers; "People also ask" + "related searches" from SERP fetches; GSC queries for own sites; Wikipedia related titles | DataForSEO / Keywords Everywhere BYOK | Breadth ≈ paid tools for long-tail. |
| **Search volume** | Google Ads search_volume | **Estimated volume model**: Google Trends relative interest (unofficial endpoint, cached) × anchor calibration (GSC impressions of connected sites, Wikipedia pageviews for entity terms) → bucketed estimate; autocomplete rank as weak signal | DataForSEO / Google Ads API (user's own Ads account, own use) | Labeled `estimated`; show buckets (10–100, 100–1K …) not fake precision. |
| **Trend (12–60 mo)** | monthly_searches | Google Trends (relative 0–100) | — | Good. |
| **CPC / competition** | Google Ads | Not available free → `null`; optional heuristic from ad-count in SERP (ads present = commercial) | BYOK | Honest null. |
| **Keyword Difficulty** | Labs KD | **Computed**: f(our Domain Authority of top-10 URLs, page-level link counts, SERP features, presence of big brands/UGC) 0–100 | — | Same method most tools use. |
| **Search intent** | Labs | Rule classifier (modifiers: buy/price/best/how/what/near me, brand list) + SERP-feature signals (shopping, local pack, featured snippet) + optional local LLM (Ollama) | Any LLM key | Good. |
| **SERP results / rank tracking** | serp/google/organic | **Pluggable SERP fetcher**: (a) self-hosted **SearXNG** container (bundled in compose) ; (b) direct headless Chromium fetch of Google/Bing with polite rate limits & user's own IP/proxy (opt-in, ToS caveat) ; (c) Brave Search API / Mojeek API free tiers if available | SerpApi, Serper, DataForSEO, Brave paid BYOK | Positions from (a) are aggregated, not exact Google; (b) is exact but rate-limited. Rank tracking defaults to (b) for small keyword sets with jittered schedule. |
| **SERP locations / geo** | serp/google/locations | **GeoNames** dump (cities ≥1000 pop, admin regions) + Google `uule` encoder for local SERP | — | Full parity. |
| **Domain Authority** | Labs domain rank / Ahrefs DR | **Our "SeoSeo Rank" (SR)**: harmonic-centrality / PageRank from **Common Crawl host-level web graph** (free, quarterly) normalized 0–100; refreshed from our own crawl link graph | Open PageRank API (free key) as secondary signal | Strong & transparent. |
| **Backlinks (list)** | backlinks/backlinks | (1) **Common Crawl**: CC index + WAT link extraction job for a target domain (batch, async, cached) ; (2) **Bing Webmaster Tools API** inbound links for verified own sites (free) ; (3) our own crawler's discovered edges ; (4) GSC "Links" not in API → manual CSV import | DataForSEO / Ahrefs BYOK | Coverage lower than Ahrefs; first-party sites near-complete via Bing WMT. New/lost computed by diffing snapshots. |
| **Referring domains, anchors, top pages, history** | backlinks/* | Derived from link store above | — | — |
| **Spam score** | DataForSEO | **Computed** heuristic (TLD, outbound link ratio, thin pages, link farms, anchor over-optimization) | — | Documented formula. |
| **Domain overview: traffic, ranked keywords, top pages, competitors** | Labs ranked_keywords etc. | **Our SERP cache index**: every SERP we ever fetch (any user on this instance, opt-in sharing) is stored → inverted index domain→keywords/positions. Traffic estimate = Σ est_volume × CTR(position). Competitors = domains co-occurring in same SERPs. Tranco + Cloudflare Radar top-domain ranks as popularity prior. For own sites: GSC exact. | DataForSEO BYOK | Grows with usage; strongest for niches the user researches. Seed packs (public keyword lists) optional. |
| **Content gap / keyword gap** | Labs | Diff of ranked-keyword sets from SERP index + GSC | — | — |
| **Site audit crawl** | own crawler (free) | **Own Rust crawler** | — | Better (faster, more checks). |
| **JS rendering** | Cloudflare Browser / Context.dev (paid) | **Headless Chromium** (chromiumoxide) in a `renderer` container | — | Full parity, free. |
| **Lighthouse / CWV lab** | on_page/lighthouse (paid) | **Lighthouse CLI** in renderer container; or **PageSpeed Insights API** (free key, generous quota) | — | Parity. |
| **CWV field data** | — (missing) | **CrUX API** (free key) | — | New feature. |
| **Local pack / Maps rank grid** | serp/google/maps | Headless Maps/local-finder fetch per grid point with `uule`/geo override (opt-in, rate-limited); OSM **Overpass/Nominatim** for business lookup & competitors | Google Places API (user key, free monthly allowance), DataForSEO BYOK | Grid scans are request-heavy → small grids (3×3/5×5) by default. |
| **Business profile / reviews / Q&A** | business_data/* | Google Places API (user key) for details & 5 reviews; OSM tags | DataForSEO BYOK | Partial. |
| **AI visibility (LLM answers)** | ai_optimization llm_scraper | **BYO LLM keys** (OpenAI w/ web-search tool, Anthropic w/ web search, Gemini w/ Google grounding, Perplexity Sonar) **or local Ollama models** **or OpenRouter free models**. Google AI Overview captured from SERP fetch when present. | — | Answers via API differ slightly from consumer UI; documented. Tool is free; LLM usage is user's own account or local. |
| **LLM mentions database / prompt explorer** | llm_mentions/* | Built from our own stored AI-visibility answers (instance-wide index) | — | Grows with usage. |
| **Domain age / WHOIS** | — | **RDAP** (free, IANA bootstrap) | — | Parity. |
| **Tech stack detection** | — (missing) | Open Wappalyzer-compatible fingerprints (webappanalyzer) | — | New. |
| **Indexing** | IndexNow (free) | IndexNow + Google Indexing API (for eligible types) + sitemap ping | — | Parity+. |
| **GSC / GA4** | free | Same (user's Google OAuth client) | — | Parity. |
| **Bing Webmaster** | — | Bing WMT API (free key) | — | New. |
| **In-app AI agent** | OpenRouter (paid) | Ollama (local, bundled optional profile) / OpenRouter free models / any OpenAI-compatible endpoint | — | Free with local models. |
| **Maps basemap** | Google Map Tiles (paid) | **OpenStreetMap** tiles (or self-hosted protomaps PMTiles) via MapLibre | — | Free. |
| **Email** | Loops | SMTP (any) / Mailpit in dev | — | Free. |

## Data freshness defaults
| Data | TTL |
|---|---|
| SERP (research) | 7 days |
| SERP (rank tracking) | per schedule, never cached across runs |
| Autocomplete / trends | 30 days |
| Domain rank (Common Crawl graph) | per CC release (~quarterly) |
| Backlink snapshot (CC) | per CC release; Bing WMT daily |
| Keyword metrics | 30 days |
| Lighthouse | per audit |

## Risks & mitigations
- **Search-engine scraping ToS / blocks** → opt-in adapter, low default rates, jitter, backoff, cache-first, CAPTCHA = stop (never bypass), SearXNG default, BYOK path documented.
- **Common Crawl size** (TB-scale) → never download full crawl; use host-graph ranks (GB) + CC Index API per-domain queries + targeted WAT range requests; precompute in background worker; optional "lite" mode skips CC.
- **Volume accuracy** → buckets + confidence labels; GSC calibration when connected.
