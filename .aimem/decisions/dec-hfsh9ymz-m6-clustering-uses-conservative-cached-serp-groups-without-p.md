---
aimem: 1
id: dec-hfsh9ymz
type: decision
title: M6 clustering uses conservative cached SERP groups without provider calls
summary: Seven-day top-ten URL overlap, same market/provider/exactness, deterministic complete-link groups; explicit missing evidence and 500-keyword project bound.
status: proposed
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T00:05:00Z
updated: 2026-11-13T00:05:00Z
---

KW-11 reuses tenant-scoped db::competitive::evidence over saved/research/rank/import keywords. Choose cached read-only clustering rather than silent bulk SERP calls or semantic-only groups. Latest snapshot is authoritative, including empty results. Every pair shares min_shared distinct normalized HTTP(S) URLs, avoiding connected-component chains. Alphabetical representatives are not priority recommendations; no stored page assignments. REST/UI/CSV/MCP share the same service; cloud-dependent M5 acceptance remains open while M6 progresses.
