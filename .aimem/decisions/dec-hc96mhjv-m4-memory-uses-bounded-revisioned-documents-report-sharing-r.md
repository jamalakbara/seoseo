---
aimem: 1
id: dec-hc96mhjv
type: decision
title: M4 memory uses bounded revisioned documents; report sharing revokes on edit
summary: Project locks + revisions prevent lost updates and quota races. Reports use an opaque CSP sandbox; public tokens are hashed and revoked on content edits.
status: proposed
trust: agent
roles: [all]
tags: [m4 memory reports]
author: agent:codex
created: 2026-11-12T01:37:39Z
updated: 2026-11-12T01:37:39Z
---

M4 first delivery covers WS-10 and RP-01-03. Project memory is a typed bounded JSONB document instead of multiple normalized list tables; the full context is read together and revision checks make replacement writes explicit. Research entries are appended and server-stamped. Reports/templates share a project-scoped document repository with project row locks for caps (1000 reports, 10 templates), explicit revisions, and unique template names. Public links contain 256-bit tokens stored only as hashes; report edits revoke rather than silently update published content. Raw HTML responses enforce a scriptless opaque-origin CSP sandbox and no-store. Remaining M4: dashboard/onboarding, GA4, IndexNow, MCP OAuth.
