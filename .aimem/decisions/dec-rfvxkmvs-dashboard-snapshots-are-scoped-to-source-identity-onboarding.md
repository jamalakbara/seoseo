---
aimem: 1
id: dec-rfvxkmvs
type: decision
title: Dashboard snapshots are scoped to source identity; onboarding preserves canonical memory
summary: Durable project-locked refresh jobs cache GSC for 24h/back off 15m; source changes hide old data. Onboarding reviews crawl evidence and uses existing memory CAS, without a duplicate document store.
status: proposed
trust: agent
roles: [all]
tags: [m4 dashboard onboarding]
paths: [backend/crates/domain/src/dashboard.rs, backend/crates/domain/src/onboarding.rs]
author: agent:codex
created: 2026-11-12T12:19:35Z
updated: 2026-11-12T12:19:35Z
---

The pulled M4 branch uses project_contexts/reports/report_templates. A pre-existing local dashboard draft used incompatible project_context/project_documents tables; adapt it to the pulled schema and retain the original draft in stash. Snapshot identity includes domain, market, selected GSC property and grant timestamp; reads and final writes reject stale identities. Project-row locking deduplicates refreshes and onboarding audits. Explicit review replaces only overview/goal and merges selected evidence pages; LLM synthesis is deferred rather than inferred from metadata. See docs/12-dashboard-and-onboarding.md.
