---
aimem: 1
id: dec-pv4m8d63
type: decision
title: Project market uses ISO country (alpha-2) + ISO 639 language, not DataForSEO location codes
summary: projects.country_code/language_code are ISO codes; provider adapters map to vendor codes (e.g. DataForSEO 2840) internally
status: proposed
trust: verified
roles: [all]
tags: [data, projects]
paths: [backend/crates/db/migrations/**, backend/crates/domain/src/validate.rs]
author: human:akbar
created: 2026-11-10T13:09:57Z
updated: 2026-11-10T13:09:57Z
verified: 2026-11-10
---

We are provider-agnostic (free-first). Vendor location codes live only in adapters.
