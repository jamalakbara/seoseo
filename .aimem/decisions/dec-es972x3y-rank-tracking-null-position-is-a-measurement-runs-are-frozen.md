---
aimem: 1
id: dec-es972x3y
type: decision
title: "Rank tracking: null position is a measurement, runs are frozen and resumable, scheduler ticks in the worker"
summary: Row with NULL position = not in top depth; no row = unchecked; fresh SERP per check; UTC schedules claimed with SKIP LOCKED
status: proposed
trust: agent
roles: [all]
tags: [rank-tracking, m3]
paths: [backend/crates/jobs/src/rank_check.rs, backend/crates/db/src/rank.rs]
author: agent:claude-code:fable-5.2
created: 2026-11-11T16:30:16Z
updated: 2026-11-11T16:30:16Z
---

rank_results has one row per keyword per run. position NULL with a row means the domain was not in the first `depth` results (a measurement); no row means never checked. UI and MCP keep these apart (">20" vs a dash). Rank checks call jobs::serp::fetch_snapshot directly: always a fresh fetch, never the 7-day research cache, and every fetch still lands in serp_snapshots for the future domain index.

A run freezes keyword_ids at creation and resumes by skipping keywords that already have a row. A missing provider, block or rate limit ends the run as failed with the reason and keeps partial results; the job itself completes, so the watchdog does not retry requests the provider just refused.

Scheduling is a tick in the worker's maintenance loop (every 60 s): claim_due uses FOR UPDATE SKIP LOCKED and pushes next_run_at forward before enqueueing, so several workers cannot double-start. Times are UTC hours; per-tracker timezones are not implemented. Devices, city and depth are not modelled because no current SERP provider exposes them.
