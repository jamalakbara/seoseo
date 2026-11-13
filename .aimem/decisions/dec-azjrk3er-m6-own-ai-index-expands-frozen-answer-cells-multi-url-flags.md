---
aimem: 1
id: dec-azjrk3er
type: decision
title: M6 own AI index expands frozen answer cells; multi-URL flags remain candidates
summary: Index only current-website retained runs in the selected market, with failed/unchecked coverage. Flag distinct same-snapshot site URLs without claiming ranking harm.
status: proposed
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T03:14:54Z
updated: 2026-11-13T03:14:54Z
---

RT-07 reuses bounded project SERP evidence (latest 30 days, top 30, first 500 keywords), validates actual URL hosts and deduplicates fragments. Multiple URLs are review candidates, not proof of harmful cannibalization. AI-05 builds expected cells from frozen run prompts/models, joins saved answers, hides prior website identities, uses literal regex-escaped alphanumeric brand boundaries and literal LIKE topic filters, and rates only successful cells. Summary/totals/pages use repeatable-read read-only transactions. No new SERP/model calls, no global prompt-demand claim, no writes/jobs.
