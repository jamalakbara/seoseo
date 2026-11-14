---
aimem: 1
id: mem-6sp7b0wy
type: memory
title: Docker disk exhaustion during M7 release build
summary: Check Windows C free space before Rust image builds; project debug incremental cache alone reached 23 GiB.
status: superseded
trust: verified
roles: [all]
tags: [docker m7]
author: human:akbar
created: 2026-11-14T19:29:53Z
updated: 2026-11-14T19:38:13Z
verified: 2026-11-14
kind: gotcha
superseded_by: mem-a2pq3k7r
---

A release build exhausted Windows C and disconnected the Docker engine. Ordinary restart remained stopping; restarting Desktop processes plus WSL shutdown recovered the engine without deleting volumes. Removing only seoseo-target/debug/incremental and trimming the Docker data filesystem recovered about 23 GiB inside Docker; host reclaim followed later. Use CARGO_BUILD_JOBS=1 and CARGO_INCREMENTAL=0 for acceptance rebuilds. M7 browser tools/screenshots were moved to F:/seoseo-m7-qa/browser. Do not reset Docker or delete durable volumes to solve this.
