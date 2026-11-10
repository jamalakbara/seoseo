---
aimem: 1
id: dec-zee6q5z6
type: decision
title: "Stack: Rust axum backend + Next.js 16 frontend + Postgres 18 + Valkey, Docker Compose"
summary: One Rust binary (serve/worker/migrate/admin), Next.js App Router web, Postgres-backed job queue; latest stable versions
status: proposed
trust: verified
roles: [all]
tags: [stack, architecture]
paths: [docs/04-architecture.md]
author: human:akbar
created: 2026-11-10T12:23:58Z
updated: 2026-11-10T12:23:58Z
verified: 2026-11-10
---

User requirement: Rust backend, Next.js frontend, latest versions, dockerized. Rejected: OpenSEO's TanStack Start on Cloudflare Workers (vendor lock-in) and dual SQLite/Postgres Drizzle (complexity). Postgres-only, SKIP LOCKED queue instead of separate broker. Versions verified 2026-11-10: axum 0.8.9, tokio 1.54, sqlx 0.9, rmcp 3.9, utoipa 6, Next 16.4, React 19.3, Tailwind 4.3, Postgres 18.6, Rust 1.99. Re-verify at scaffold.
