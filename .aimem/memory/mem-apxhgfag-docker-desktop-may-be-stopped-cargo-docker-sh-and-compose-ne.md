---
aimem: 1
id: mem-apxhgfag
type: memory
title: Docker Desktop may be stopped; cargo-docker.sh and compose need it running
summary: On a dockerDesktopLinuxEngine pipe error, start Docker Desktop and wait about 30 s
status: active
trust: agent
roles: [all]
tags: [dev, docker]
author: agent:claude-code:fable-5.2
created: 2026-11-10T17:15:12Z
updated: 2026-11-10T17:15:12Z
kind: gotcha
---

The dev machine has no local Rust toolchain: scripts/cargo-docker.sh is the only way to run cargo, and it needs the Docker daemon. If docker fails with "dockerDesktopLinuxEngine ... cannot find the file", start Docker Desktop and poll `docker info` (about 30 s) before retrying.
