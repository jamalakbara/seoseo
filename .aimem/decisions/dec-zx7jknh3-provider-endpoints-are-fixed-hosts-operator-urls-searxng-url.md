---
aimem: 1
id: dec-zx7jknh3
type: decision
title: Provider endpoints are fixed hosts; operator URLs (SEARXNG_URL) come from env only
summary: Admins toggle providers and store encrypted keys in the UI but cannot set a provider URL, so settings cannot become an SSRF vector
status: proposed
trust: agent
roles: [all]
tags: [providers, security, m2]
paths: [backend/crates/jobs/src/provider_ctx.rs, backend/crates/domain/src/providers.rs]
author: agent:claude-code:fable-5.2
created: 2026-11-10T17:15:09Z
updated: 2026-11-10T17:15:09Z
---

provider_settings holds enabled + secret_enc (AES-256-GCM with APP_ENCRYPTION_KEY) per organization. There is no per-org base URL: SearXNG usually lives on an internal host, so letting a workspace admin point it elsewhere would bypass the SSRF guard of the crawler. Without APP_ENCRYPTION_KEY, storing a key returns 409 with instructions. SERP preference order is fixed: serper, then brave, then searxng.
