---
aimem: 1
id: dec-1xjv232z
type: decision
title: Site audit state lives in Postgres (frontier/pages/links/issues), not in worker memory
summary: "Resumable crawl: pages unique per (audit,url_hash) inserted once per tx; site rules recomputed in analyze; leases requeue crashed jobs"
status: proposed
trust: verified
roles: [all]
tags: [audit, jobs, architecture]
paths: [backend/crates/jobs/src/site_audit.rs, backend/crates/db/src/audits.rs]
author: human:akbar
created: 2026-11-10T14:18:13Z
updated: 2026-11-10T14:18:13Z
verified: 2026-11-10
---

Lesson from OpenSEO spec 0009 (workflow memory limits, runs stuck in running). Verified by SIGKILL of the worker mid-crawl: attempt 2 resumed with identical results. Details in docs/08-site-audit.md.
