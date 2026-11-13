---
aimem: 1
id: mem-qgxh1jd8
type: memory
title: Deferred pages can hydrate after layout queries populate client cache
summary: Use a stable client-ready fallback for cache-dependent initial page trees; unique sibling keys are also required.
status: active
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T03:46:51Z
updated: 2026-11-13T03:46:51Z
kind: gotcha
---

Observed browser hydration failures when Competitive inserted its new cache-dependent first card after the layout project query finished before deferred page hydration. Added frontend/lib/use-client-ready.ts using useSyncExternalStore server=false/client=true and a stable initial fallback on Competitive/Backlinks. Existing Backlinks discovery/import siblings also reused identical keys; they now have distinct prefixes. Repeated browser tests passed with no hydration or duplicate-key errors.
