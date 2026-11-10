---
aimem: 1
id: dec-4m2za5vp
type: decision
title: "Free-first data: replace DataForSEO with free/computed providers, paid APIs only as BYOK"
summary: Every research feature must work with zero paid keys; trait-based providers with free defaults and optional paid adapters
status: proposed
trust: verified
roles: [all]
tags: [data, providers, product]
paths: [docs/05-free-data-sources.md, backend/crates/providers/**]
author: human:akbar
created: 2026-11-10T12:23:59Z
updated: 2026-11-10T12:23:59Z
verified: 2026-11-10
---

OpenSEO bills every research feature through DataForSEO. Our promise is completely free. Free defaults: autocomplete expansion, Google Trends, SearXNG + opt-in headless SERP fetch, Common Crawl host graph for authority (SR) and backlinks, Bing WMT, GSC/GA4, PSI/CrUX/Lighthouse CLI, RDAP, OSM, BYO LLM/Ollama. Volume/KD are estimated/computed and labeled with confidence. See docs/05.
