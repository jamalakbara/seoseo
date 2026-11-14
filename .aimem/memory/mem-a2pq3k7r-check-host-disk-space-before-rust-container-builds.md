---
aimem: 1
id: mem-a2pq3k7r
type: memory
title: Check host disk space before Rust container builds
summary: Project debug incremental cache reached 23 GiB; a release build exhausted the Windows host drive and stopped Docker.
status: active
trust: agent
roles: [all]
tags: [docker m7]
author: agent:codex
created: 2026-11-14T19:38:13Z
updated: 2026-11-14T19:38:13Z
supersedes: mem-6sp7b0wy
kind: gotcha
---

Corrects the author attribution of the superseded record; this is agent-observed. Docker recovered without deleting durable volumes after restarting Desktop and WSL. Only this project target/debug/incremental cache was removed; trimming reclaimed guest space, then host space recovered. Use single-job builds and CARGO_INCREMENTAL=0 for acceptance. M7 browser tools and screenshots now live at F:/seoseo-m7-qa/browser. Do not reset Docker or delete durable volumes to reclaim build space.
