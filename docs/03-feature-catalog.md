# 03 — Feature Catalog & Backlog

Every feature with an ID, OpenSEO parity reference, priority, data source, and milestone.
Priority: **P0** MVP · **P1** parity · **P2** beyond parity · **P3** later.
Status column is updated as we build (`todo` / `wip` / `done`). Last update: 2026-11-12 (M4 integrations, onboarding synthesis and MCP OAuth).

## Workspace & identity
| ID | Feature | OpenSEO ref | Pri | Data | MS | Status |
|---|---|---|---|---|---|---|
| WS-01 | Email/password sign-up/in, first user = admin | Better Auth `password` mode | P0 | — | M0 | done |
| WS-02 | Auth modes: `password`, `none` (trusted network), optional OIDC/Google | AUTH_MODE | P0 | — | M0 | wip (password+none done; OIDC later) |
| WS-03 | Password reset & email verify (SMTP) | forgot/reset routes | P1 | SMTP | M1 | todo |
| WS-04 | Organizations, roles owner/admin/member | spec 0011 | P1 | — | M1 | done |
| WS-05 | Invitations via link/email | accept-invitation | P1 | — | M1 | done (link-based; email delivery needs SMTP, WS-03) |
| WS-06 | API keys (org-bound) | @better-auth/api-key, spec 0017 | P0 | — | M0 | done |
| WS-07 | Account deletion w/ grace period | account-deletion | P2 | — | M7 | todo |
| WS-08 | Projects CRUD, archive/restore | projects.ts | P0 | — | M0 | done |
| WS-09 | Project market (country/lang/city) | projects.location_code | P0 | GeoNames | M0 | wip (country+lang done; city picker M3) |
| WS-10 | Project memory (sections, competitors, key pages, research log) | spec 0010 | P1 | — | M4 | done (bounded revisioned document; REST/UI/MCP; append-only research log) |
| WS-11 | Onboarding wizard + website research | spec 0005, researchProjectWebsite | P1 | crawler + LLM | M4 | done |
| WS-12 | Team page, transfer ownership, switch org | organization.ts | P1 | — | M1 | wip (team page + remove member done; transfer ownership & org switcher UI todo) |
| WS-13 | Settings: provider adapters & keys UI | (new) | P0 | — | M1 | done (M2: provider catalog, on/off, encrypted BYOK keys, live test, SR import) |

## Dashboard
| DB-01 | Setup checklist | spec 0015 | P1 | — | M4 | done |
| DB-02 | Overview cards (health, GSC new queries, link activity, rank movers) | getDashboardOverview | P1 | GSC/links | M4 | done (recorded link evidence; automatic discovery in M5) |
| DB-03 | Keywords tab (GSC new/top/striking distance) | getDashboardKeywords | P1 | GSC | M4 | done |
| DB-04 | Backlinks tab (new/lost/live) | getDashboardBacklinks | P1 | link store | M5 | todo |
| DB-05 | Refresh claims (24 h) | data_refresh_claims | P1 | — | M4 | done |

## Keyword research
| KW-01 | Seed → ideas (autocomplete a–z, questions, prepositions) | researchKeywords | P0 | autocomplete | M2 | done (Google + Bing autocomplete; a-z, questions, prepositions; en/es/de/fr/pt/id modifiers) |
| KW-02 | Related searches + PAA from SERP | related_keywords | P0 | SERP | M2 | wip (related searches from SearXNG/Serper and PAA from Serper/Brave become ideas for the seed SERP; SearXNG has no PAA) |
| KW-03 | Estimated volume (buckets, confidence) + trend | keyword_overview | P0 | Trends + calibration | M2 | wip (relative popularity 0-100 for every idea; volume bucket + 12-mo trend only where a Wikipedia article matches; Google Trends adapter not built, needs bot-check workaround) |
| KW-04 | Computed KD | Labs KD | P0 | SERP + SR | M2 | done (click-weighted SR of top 10; null without SERP or authority data) |
| KW-05 | Intent classifier | IntentBadge | P0 | rules (+LLM) | M2 | done (rules + brand + SERP features; LLM refinement later) |
| KW-06 | SERP features detection | serp_features | P0 | SERP | M2 | done (features exposed by the SERP provider) |
| KW-07 | SERP analysis card | getSerpAnalysis | P0 | SERP | M2 | done (SERP dialog: results + SR, features, related, PAA, KD, intent; 7-day shared cache) |
| KW-08 | Filters/sort/paging/export | keyword table | P0 | — | M2 | done (search, intent, questions filter, sort, paging, CSV; API also filters popularity/kd/volume/words) |
| KW-09 | Saved keywords + tags + refresh + export | saved-keywords | P0 | — | M2 | done (save/manual, tags, CSV, per-keyword SERP refresh and durable bulk refresh with progress; available demand estimates are refreshed through the pageview cache) |
| KW-10 | Local (city) volume context | spec 0008 | P2 | Trends geo | M6 | todo |
| KW-11 | Keyword clustering (SERP overlap) | skill only | P2 | SERP | M6 | todo |
| KW-12 | CPC/competition (BYOK only) | google_ads | P3 | BYOK | M8 | todo |

## Competitive research
| DO-01 | Domain overview (SR, traffic est., kw count, distribution) | getDomainOverview | P1 | SERP index + CC graph | M5 | wip (project SERP sample + SR; traffic unknown) |
| DO-02 | Ranked keywords table | ranked_keywords | P1 | SERP index / GSC | M5 | wip (observed project keywords with provider/age/coverage) |
| DO-03 | Top pages | relevant_pages | P1 | SERP index | M5 | wip (best-result URLs in sample) |
| DO-04 | SERP competitors | serp_competitors | P1 | SERP index | M5 | wip (shared observed keywords in sample) |
| DO-05 | Domain keyword suggestions / keyword gap | getDomainKeywordSuggestions | P1 | SERP index | M5 | wip (sampled gaps against project website) |
| DO-06 | Tech stack detection | (new) | P2 | fingerprints | M6 | todo |
| DO-07 | Domain age (RDAP) | free tool | P1 | RDAP | M5 | todo |

## Backlinks
| BL-01 | Overview + history | backlinks summary/history | P1 | CC + Bing WMT | M5 | todo |
| BL-02 | Backlinks table | backlinks/backlinks | P1 | CC | M5 | todo |
| BL-03 | Referring domains | referring_domains | P1 | CC | M5 | todo |
| BL-04 | Top linked pages, anchors | domain_pages_summary | P1 | CC | M5 | todo |
| BL-05 | Spam score | dashboard filter | P1 | computed | M5 | todo |
| BL-06 | New/lost via snapshot diff | backlink_snapshots | P1 | — | M5 | todo |
| BL-07 | Link prospecting (link intersect) | skill link-prospecting | P2 | CC | M6 | todo |
| BL-08 | Domain authority "SR" from CC host graph | Ahrefs DR lookup | P0 | CC host graph | M2 | done (top-N import of Common Crawl domain ranks, SR = log-scaled harmonic rank, lookup API + UI) |

## Rank tracking
| RT-01 | Tracker config (domain, market, city, devices, depth, schedule) | rank_tracking_configs | P0 | — | M3 | wip (domain, market, schedule done; city, device and depth settings todo: providers expose no device or geo dimension yet) |
| RT-02 | Keyword add/remove/pin/tag/bulk import | add/remove/pin tools | P0 | — | M3 | wip (add/remove/pin, tags on add, bulk paste done; file import todo) |
| RT-03 | Scheduler (daily/weekly/monthly at time+tz) | selfhost-scheduler | P0 | — | M3 | wip (daily/weekly/monthly at an hour in UTC; per-tracker timezone todo) |
| RT-04 | Run full/subset, per-device positions, URL, features | RankCheckWorkflow | P0 | SERP fetcher | M3 | wip (full or subset runs, fresh SERP per keyword, position, URL, features, provider + exact flag; per-device todo) |
| RT-05 | Latest table + deltas, history chart, trend, matrix | getRank* | P0 | — | M3 | wip (latest table with change/best, keyword history chart, per-run trend; keyword x run matrix todo) |
| RT-06 | Cost/"request budget" estimate (requests not $) | estimateRankCheckCost | P1 | — | M3 | done (requests per run and month, provider, rough duration) |
| RT-07 | Cannibalization flags | (new) | P2 | — | M6 | todo |
| RT-08 | Movement alerts | (new) | P2 | — | M7 | todo |
| RT-09 | SERP volatility index | (new) | P3 | own data | M8 | todo |

## Site audit
| SA-01 | Start audit (config) | startAudit | P0 | crawler | M1 | done |
| SA-02 | Discovery robots/sitemaps | discovery.ts | P0 | — | M1 | done |
| SA-03 | SSRF url-policy, throttle, 429 cooldown | url-policy, crawl-throttle | P0 | — | M1 | done |
| SA-04 | Page analyzer (all audit_pages fields) | page-analyzer | P0 | — | M1 | done |
| SA-05 | OpenSEO issue rules (27) | issues/* | P0 | — | M1 | done |
| SA-06 | Extended rules (H1, hreflang, mixed content, orphans, broken links, schema validation, soft-404, …) | (new) | P1 | — | M2 | done (H1, mixed content, JSON-LD, links, sitemap, hreflang syntax/self/reciprocal/indexability, heuristic soft-404, URL hygiene, bounded same-origin asset checks) |
| SA-07 | JS rendering (headless Chromium) | rendered-page | P1 | renderer | M2 | done (opt-in rendered DOM, isolated browser + guarded egress proxy, static fallback with explicit issue) |
| SA-08 | Lighthouse (CLI or PSI API) | lighthouse.ts | P1 | renderer/PSI | M2 | done (desktop lab categories, metrics and opportunities stored per page and shown in page detail) |
| SA-09 | CrUX field data | (new) | P2 | CrUX API | M6 | todo |
| SA-10 | Progress (live SSE), history, delete | getCrawlProgress | P0 | — | M1 | done |
| SA-11 | Results UI: summary, pages, issues, page detail, export | results/* | P0 | — | M1 | done |
| SA-12 | Internal link graph + suggestions | (new) | P2 | — | M6 | todo |
| SA-13 | Audit comparison + scheduled re-audit | (new) | P2 | — | M6 | todo |
| SA-14 | Signed crawler credentials (Shopify web-bot-auth) | crawlerCredentials | P3 | — | M8 | todo |
| SA-15 | Log-file analyzer | (new) | P3 | upload | M8 | todo |
| SA-16 | Change monitoring of key pages | (new) | P2 | crawler | M7 | todo |

## Local SEO
| LO-01 | Business lookup | search_local_businesses | P1 | OSM / Places key | M6 | todo |
| LO-02 | Grid tracker + scan workflow | LocalRankWorkflow | P1 | local SERP fetch | M6 | todo |
| LO-03 | Heatmap (MapLibre + OSM tiles), ARP/ATRP/SoLV | local-seo UI | P1 | OSM | M6 | todo |
| LO-04 | Competitors per point | local_rank_businesses | P1 | — | M6 | todo |
| LO-05 | Business profile, reviews, Q&A, updates | business tools | P2 | Places key | M7 | todo |
| LO-06 | Categories list | list_business_categories | P2 | static list | M6 | todo |

## AI visibility
| AI-01 | Tracker config (engines, market, schedule) | ai_trackers | P1 | — | M5 | todo |
| AI-02 | Prompt generation/research | generateAiVisibilityPrompts | P1 | LLM | M5 | todo |
| AI-03 | Run workflow (answers, sources, matches) | AiVisibilityWorkflow | P1 | LLM APIs / Ollama | M5 | todo |
| AI-04 | Results, SoV, trend, sources, answer viewer, export | getAiVisibility* | P1 | — | M5 | todo |
| AI-05 | Brand lookup, prompt explorer (own index) | brand-lookup, prompt-explorer | P2 | own index | M6 | todo |
| AI-06 | Google AI Overview capture | google_ai_overview | P2 | SERP | M6 | todo |

## Integrations
| IN-01 | GSC OAuth + property + performance explorer + export | spec 0003 | P0 | GSC API | M3 | done (project grant, PKCE/state, encrypted tokens, verified property, bounded queries, CSV; live Google client setup required) |
| IN-02 | GSC URL inspection | inspect_urls | P1 | GSC API | M3 | done (1–10 URLs, indexed-version results, partial URL errors, REST/UI/MCP) |
| IN-03 | GA4 connection + 9 reports | spec 0007 | P1 | GA4 Data API | M4 | done |
| IN-04 | Bing Webmaster Tools | (new) | P2 | Bing API | M6 | todo |
| IN-05 | IndexNow setup/verify/submit | spec 0016 | P1 | IndexNow | M4 | done |
| IN-06 | Webhooks / Slack / Discord | (new) | P2 | — | M7 | todo |

## Reports
| RP-01 | Report store + sandboxed viewer | spec 0012 | P1 | — | M4 | done (500 KiB self-contained HTML; REST/UI/MCP; CSP + iframe sandbox) |
| RP-02 | Templates (≤10/project, branding) | spec 0013 | P1 | — | M4 | done (CRUD instruction briefs, maximum 10/project; branding encoded in template instructions) |
| RP-03 | Public share links | spec 0014 | P1 | — | M4 | done (192-bit hashed revocable tokens; editing revokes; noindex public viewer) |
| RP-04 | Built-in report generators (audit, rank, kw, links, AI) | (new) | P2 | — | M7 | todo |
| RP-05 | PDF export (headless Chromium) | print | P2 | renderer | M7 | todo |
| RP-06 | Scheduled email digests | (new) | P2 | SMTP | M7 | todo |

## AI assistant & content
| AG-01 | In-app chat agent w/ tools + memory | SAM | P1 | Ollama/OpenAI-compat | M7 | todo |
| AG-02 | Content brief generator | (new) | P2 | SERP + LLM | M7 | todo |
| AG-03 | On-page optimizer score | (new) | P2 | SERP terms | M7 | todo |

## MCP / API / skills
| MC-01 | MCP streamable HTTP + API-key auth | mcp server | P0 | — | M3 | done (stateless Streamable HTTP at /mcp, API-key or session auth, Host allow-list) |
| MC-02 | MCP OAuth 2.1 (DCR, PKCE, consent, org-bound) | workers-oauth-provider | P1 | — | M4 | done |
| MC-03 | Tool parity (~90 tools) | mcp/tools | P1 | — | M3–M6 | wip (43 tools: existing M3 set plus project memory and report/template/sharing tools) |
| MC-04 | MCP Apps widgets (SERP/GSC views) | widget-tools | P3 | — | M8 | todo |
| MC-05 | Agent skills (12 ported + new) | plugins/openseo/skills | P1 | — | M5 | todo |
| AP-01 | REST API + OpenAPI 3.1 (utoipa) | — | P0 | — | M0 | done |

## Free public tools
| FT-01..11 | Keyword generator, SERP simulator, backlink checker, DA checker, domain age, robots tester, sitemap validator, meta checker, schema validator, redirect checker, hreflang checker | web/ free tools | P2 | — | M8 | todo |

## Ops
| OP-01 | Docker Compose (api, worker, web, db, valkey, renderer, searxng, [ollama]) | deploy/ | P0 | — | M0 | wip (db, cache, api, worker, web, test overlay, searxng profile, renderer + egress proxy profile; ollama/caddy todo) |
| OP-02 | Health, Prometheus metrics, JSON logs | /health | P0 | — | M0 | done |
| OP-03 | Job dashboard (queued/running/failed, retry) | (new) | P1 | — | M3 | wip (status counts, filter, progress, errors; retry from the dashboard todo, re-run from the feature page) |
| OP-04 | Backup/restore script | (new) | P1 | — | M7 | todo |
| OP-05 | Opt-in telemetry (off by default) | self-host-telemetry | P3 | — | M8 | todo |
