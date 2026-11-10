---
aimem: 1
id: mem-brh4zmks
type: memory
title: utoipa handler fn names become operationIds; keep them globally unique
summary: Duplicate fn names (list/create) across route modules clobber generated TS types; api test operation_ids_are_unique guards it
status: active
trust: verified
roles: [all]
tags: [backend, openapi]
paths: [backend/crates/api/src/routes/**]
author: human:akbar
created: 2026-11-10T13:10:12Z
updated: 2026-11-10T13:10:12Z
verified: 2026-11-10
kind: gotcha
---

openapi-typescript keys operations by operationId; utoipa defaults operationId to the handler fn name. Name handlers like list_projects, create_api_key. Verified 2026-11-10 by the test in backend/crates/api/src/lib.rs.
