---
aimem: 1
id: mem-vv3g73fk
type: memory
title: Redirecting URLs must not keep the destination's content
summary: Storing a redirect with its destination's facts caused false duplicate-title/content; redirected pages now skip content, links and indexability
status: active
trust: verified
roles: [all]
tags: [audit, crawler]
paths: [backend/crates/jobs/src/site_audit.rs]
author: human:akbar
created: 2026-11-10T14:18:16Z
updated: 2026-11-10T14:18:16Z
verified: 2026-11-10
kind: gotcha
---

Found by the badseo e2e (about.html flagged duplicate with /old-page). Also: frontier upserts must dedupe url_hash within a batch or Postgres errors 'ON CONFLICT DO UPDATE command cannot affect row a second time'.
