---
aimem: 1
id: mem-4ek7y1d3
type: memory
title: Workspace UUID enables v7, not v4
summary: Tests and new IDs must use common::new_id()/Uuid::now_v7(); Uuid::new_v4() does not compile.
status: active
trust: agent
roles: [all]
tags: [rust, uuid, tests]
paths: [backend/Cargo.toml, backend/crates/common/src/lib.rs, backend/crates/domain/src/*.rs]
author: agent:codex
created: 2026-11-13T04:57:57Z
updated: 2026-11-13T04:57:57Z
kind: gotcha
---

The workspace pins uuid 1.29 with features v7 and serde only. Existing M6 tests that called Uuid::new_v4() failed when the full local test profile compiled. Use common::new_id() for production and test identifiers, matching the project's time-ordered UUID v7 convention; do not enable v4 just for fixtures.
