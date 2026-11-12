---
aimem: 1
id: mem-kpwt9tg8
type: memory
title: utoipa schema type names must be globally distinct
summary: Two ToSchema structs called Competitor collided in generated OpenAPI; competitive research uses SerpCompetitor and an API regression test.
status: active
trust: agent
roles: [all]
tags: [openapi m5]
author: agent:codex
created: 2026-11-12T14:26:11Z
updated: 2026-11-12T14:26:11Z
kind: gotcha
---

ToSchema derives use the Rust type name as the component key even across modules. Reusing memory::Competitor for a different competitive-research shape silently left the generated TS type with memory fields. Rename the SERP shape to SerpCompetitor. api::tests::competitive_and_memory_competitor_schemas_are_distinct now guards both shapes and the DomainOverview reference.
