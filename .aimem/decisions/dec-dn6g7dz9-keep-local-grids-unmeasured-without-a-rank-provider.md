---
aimem: 1
id: dec-dn6g7dz9
type: decision
title: Keep local grids unmeasured without a rank provider
summary: Use Nominatim only for explicit cached discovery; derived grid points stay unknown until a real local-results provider measures them.
status: proposed
trust: agent
roles: [all]
tags: [local-seo, nominatim, provider-boundary]
paths: [backend/crates/domain/src/local.rs, backend/crates/db/migrations/0017_local_seo.sql]
author: agent:codex
created: 2026-11-14T11:06:38Z
updated: 2026-11-14T11:06:38Z
links: [mem-q5qkwy6e]
---

Nominatim is used only for explicit user-submitted business discovery with attribution, a switchable operator endpoint, 30-day caching, per-query cooldown, and an instance-wide one-request-per-second gate. It is never queried over the generated grid. Grid geometry is provider-neutral and returns rank=null plus measured=false until a real local SERP/Maps provider supplies evidence. This avoids mislabeling OpenStreetMap proximity or result order as Google Maps/local-pack rank and respects the public Nominatim usage policy.
