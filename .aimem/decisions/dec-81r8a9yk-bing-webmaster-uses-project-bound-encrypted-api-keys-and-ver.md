---
aimem: 1
id: dec-81r8a9yk
type: decision
title: Bing Webmaster uses project-bound encrypted API keys and verified-site reads
summary: Validate before save; encrypt per project; expose only verified sites and bounded read-only first-party reports.
status: proposed
trust: agent
roles: [all]
tags: [m6, bing, security, integrations]
paths: [backend/crates/domain/src/bing.rs, backend/crates/db/src/bing.rs, backend/crates/db/migrations/0016_bing_webmaster.sql]
author: agent:codex
created: 2026-11-13T04:52:04Z
updated: 2026-11-13T04:52:04Z
---

Bing supports OAuth and user-level API keys. For the self-hosted first delivery, accept a user-generated API key so operators do not need to register a global OAuth client. Validate it with GetUserSites before any write, encrypt it with APP_ENCRYPTION_KEY, bind the encrypted credential to one project, and clear the selected site whenever the key is replaced. Only an exact verified site returned by Bing can be selected. Members may read top-query, top-page, crawl and paged inbound-link reports; only admins connect, select or disconnect. Provider type metadata and verification codes are stripped, response bodies and errors are bounded/sanitized, and missing/provider-sampled data is never presented as exhaustive.
