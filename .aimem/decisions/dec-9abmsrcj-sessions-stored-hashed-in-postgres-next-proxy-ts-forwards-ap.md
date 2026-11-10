---
aimem: 1
id: dec-9abmsrcj
type: decision
title: Sessions stored hashed in Postgres; Next proxy.ts forwards /api to Rust API at runtime
summary: Opaque session cookie (SHA-256 at rest), CSRF via x-seoseo-csrf header on cookie writes; web reaches API same-origin through proxy.ts rewrite using API_INTERNAL_URL
status: proposed
trust: verified
roles: [all]
tags: [auth, frontend, security]
paths: [backend/crates/api/src/auth.rs, frontend/proxy.ts]
author: human:akbar
created: 2026-11-10T13:10:02Z
updated: 2026-11-10T13:10:02Z
verified: 2026-11-10
---

next.config rewrites are baked at build time; proxy.ts reads env at runtime so one image works anywhere. Valkey reserved for cache/rate limits/pubsub later.
