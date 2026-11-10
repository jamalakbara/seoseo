---
aimem: 1
id: dec-hzv4rfn9
type: decision
title: Runtime sqlx queries with static SQL via concat! macros; no compile-time query! macros
summary: sqlx 0.9 rejects dynamic SQL strings (SqlSafeStr); shared column lists are macro_rules literals joined with concat!
status: proposed
trust: verified
roles: [all]
tags: [backend, db]
paths: [backend/crates/db/src/**]
author: human:akbar
created: 2026-11-10T13:09:59Z
updated: 2026-11-10T13:09:59Z
verified: 2026-11-10
---

Avoids needing a live DB or .sqlx offline cache to compile (no local Rust/DB on dev machine; builds in Docker). Never build SQL from user input; use QueryBuilder if dynamic SQL is ever needed.
