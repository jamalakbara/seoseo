---
aimem: 1
id: dec-d4wn3d9k
type: decision
title: M7 compares stored observations and uses a single-attempt transactional outbox
summary: Atomic evaluation/event/job commits deduplicate alerts; external attempts are never replayed after ambiguous outcomes.
status: superseded
trust: verified
roles: [all]
tags: [m7 alerts digests]
author: human:akbar
created: 2026-11-14T19:23:56Z
updated: 2026-11-14T19:38:12Z
verified: 2026-11-14
superseded_by: dec-qw6gcx0b
---

Compare adjacent completed runs only when provider, exactness, depth and scope match; absent observations stay unknown. Key-page comparisons require matching audit start URL/render mode and successful HTML. Single-attempt delivery claims sending before I/O; retries could duplicate external mail/webhook messages after ambiguous disconnects. Destination/subscription revisions and project scope cancel stale queued work. Weekly email goes only to the member account email in UTC. Business writes remain local drafts with optional transient read-only Places preview.
