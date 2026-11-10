---
aimem: 1
id: mem-9r6emjbq
type: memory
title: "No local Rust toolchain: run cargo via scripts/cargo-docker.sh"
summary: scripts/cargo-docker.sh '<cargo args>' runs in rust:1.99-slim-bookworm with cached registry/target volumes
status: active
trust: verified
roles: [all]
tags: [tooling, backend]
paths: [scripts/cargo-docker.sh]
author: human:akbar
created: 2026-11-10T13:10:03Z
updated: 2026-11-10T13:10:03Z
verified: 2026-11-10
kind: howto
---

Example: scripts/cargo-docker.sh 'clippy --workspace --all-targets -- -D warnings'. Regenerate OpenAPI: scripts/cargo-docker.sh 'run -q -p app -- openapi' > frontend/openapi.json then pnpm gen:api in frontend.
