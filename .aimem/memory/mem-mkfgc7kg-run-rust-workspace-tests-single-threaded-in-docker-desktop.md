---
aimem: 1
id: mem-mkfgc7kg
type: memory
title: Run Rust workspace tests single-threaded in Docker Desktop
summary: Parallel lib-test linking exceeded the 3.8 GiB Docker limit; CARGO_BUILD_JOBS=1 and cargo test -j 1 passed all 57 tests.
status: active
trust: agent
roles: [all]
tags: [rust, docker, tests, oom]
paths: [backend/**, scripts/cargo-docker.sh]
author: agent:codex
created: 2026-11-12T07:23:14Z
updated: 2026-11-12T07:23:14Z
kind: gotcha
---

On Apple Silicon Docker Desktop with a 3.8 GiB VM limit, cargo test --workspace --lib linked several large test binaries concurrently and ld was killed with signal 9. Use DOCKER_ARGS='-e CARGO_BUILD_JOBS=1' bash scripts/cargo-docker.sh test --workspace --lib -j 1. This completed all 57 tests. Clippy can use the same setting for predictable memory use.
