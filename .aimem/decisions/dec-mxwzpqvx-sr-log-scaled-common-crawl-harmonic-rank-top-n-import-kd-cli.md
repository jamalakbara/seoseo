---
aimem: 1
id: dec-mxwzpqvx
type: decision
title: SR = log-scaled Common Crawl harmonic rank (top-N import); KD = click-weighted SR of top 10
summary: Stream only the first N lines of the sorted rank file; SR and KD are null when unknown
status: proposed
trust: agent
roles: [all]
tags: [metrics, m2]
paths: [backend/crates/scoring/**, backend/crates/jobs/src/cc_ranks.rs]
author: agent:claude-code:fable-5.2
created: 2026-11-10T17:15:11Z
updated: 2026-11-10T17:15:11Z
---

SR = 100 * (1 - log10(rank) / log10(total_domains)) using the harmonic-centrality position in the latest cc-main hyperlinkgraph release. The rank file is sorted, so cc_rank_import streams the gzip and stops after N lines (1M domains is about 30 MB) instead of downloading 2.5 GB.

Domains outside the imported list show SR null in the UI; inside the KD formula they count as sr_floor = weakest imported SR / 2. KD = click-weighted average SR of the top 10 results, null with fewer than 5 results or no rank data.

serp_snapshots / serp_items are instance-wide (public data, 7-day cache) and are meant to feed domain overview in M5.
