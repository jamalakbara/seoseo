---
aimem: 1
id: dec-qqwn9rdk
type: decision
title: M4 uses separate GA4 grants, explicit model review, guarded IndexNow and resource-bound MCP OAuth
summary: GA4 stays independent of GSC; model drafts never save context; IndexNow needs exact-host ownership; MCP public-client OAuth tokens are workspace-bound and valid only at /mcp.
status: proposed
trust: agent
roles: [all]
tags: [m4 oauth ga4 indexnow]
paths: [backend/crates/domain/src/oauth.rs, backend/crates/domain/src/ga4.rs, backend/crates/domain/src/integrations.rs]
author: agent:codex
created: 2026-11-12T13:16:21Z
updated: 2026-11-12T13:16:21Z
---

M4 completion follows the existing project-owned encrypted Google-grant pattern rather than user-shared grants, so disconnect cannot affect another project or GSC. Nine GA4 reports have fixed dimensions/metrics and explicit channel metadata; restricted values remain null. Optional operator-configured chat-completions models see bounded untrusted crawl evidence, have no tools, and only return drafts for explicit review/save. IndexNow uses guarded DNS/no redirects, per-host root-key verification and a database cooldown. MCP OAuth uses DCR, exact redirects, mandatory resource and S256 proof, session+CSRF consent for the active workspace, hashed opaque tokens, refresh rotation/replay family revocation and live membership lookup; OAuth tokens do not authenticate REST. Dashboard link activity is recorded evidence, not automatic M5 backlink discovery. See docs/13-m4-integrations-and-oauth.md.
