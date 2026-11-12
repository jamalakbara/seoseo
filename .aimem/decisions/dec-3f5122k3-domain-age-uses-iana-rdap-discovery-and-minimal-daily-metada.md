---
aimem: 1
id: dec-3f5122k3
type: decision
title: Domain age uses IANA RDAP discovery and minimal daily metadata cache
summary: Allow only IANA-discovered public HTTPS registry endpoints with guarded DNS; no redirects, contacts or inferred dates. Cache results for 24h.
status: proposed
trust: agent
roles: [all]
tags: [m5 rdap]
author: agent:codex
created: 2026-11-12T15:32:17Z
updated: 2026-11-12T15:32:17Z
---

DO-07 is a separate on-demand POST/MCP action so ordinary SERP inspection never triggers registry requests. Use longest suffix matching from the fixed IANA DNS bootstrap, then a matching registry domain object registration event. Dynamic service URLs are an exception to fixed provider hosts only because they come from IANA; public DNS and HTTPS/port policy still apply. Reuse the guarded crawler exact-byte client for DNS filtering, no proxy/redirect behavior and streaming caps. Cache the bootstrap and minimal parsed records for 24 hours including 404/unsupported outcomes; no force bypass. Registration is not website age or proof of continuous ownership. No personal RDAP contact data is retained.
