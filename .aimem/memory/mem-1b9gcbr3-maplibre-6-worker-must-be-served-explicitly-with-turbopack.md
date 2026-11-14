---
aimem: 1
id: mem-1b9gcbr3
type: memory
title: MapLibre 6 worker must be served explicitly with Turbopack
summary: Dev/build copies matching worker and shared ESM module to same-origin public assets; default sibling worker URL fails after bundling.
status: active
trust: verified
roles: [all]
tags: [m6 frontend maplibre]
paths: [frontend/components/local/grid-map.tsx frontend/scripts/prepare-map-worker.mjs]
author: human:akbar
created: 2026-11-14T14:08:03Z
updated: 2026-11-14T14:08:03Z
verified: 2026-11-14
kind: gotcha
---

Production browser QA found Worker failed to load while raster tiles and DOM markers still rendered. MapLibre 6 resolves a sibling ESM worker relative to its entry module; Turbopack relocates that module. prepare-map-worker.mjs copies installed worker/shared modules during dev/build; GridMap sets the same-origin worker URL with the installed version. Rebuilt mobile map rendered without new worker errors.
