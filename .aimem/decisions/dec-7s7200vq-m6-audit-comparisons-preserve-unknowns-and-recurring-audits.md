---
aimem: 1
id: dec-7s7200vq
type: decision
title: M6 audit comparisons preserve unknowns and recurring audits claim atomically
summary: Compare completed same-start observations; absent URLs/issues stay uncertain. Recurring crawls freeze reviewed budgets, use revision CAS and serialize due jobs under project locks.
status: proposed
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T01:05:35Z
updated: 2026-11-13T01:05:35Z
---

SA-13 compares full metadata fingerprints and observed issue sets in one repeatable-read read-only transaction. Failed, redirected, absent or method-changed pages cannot resolve prior issues. Schedules copy a completed current-website audit, require admin and revision CAS, and tick in the worker every 60 seconds. Audit+job+next occurrence commit atomically with the project row lock used by manual starts. Active audits skip one occurrence; archives, website changes, renderer/policy failures or 100 retained audits pause. No automatic deletion or catch-up backlog. Scheduled jobs freeze expected domain and recheck it before running/further crawl batches.
