---
aimem: 1
id: dec-45am7w5j
type: decision
title: TypeScript pinned to 6.0.x (not 7) in frontend
summary: TS 7 native compiler has no JS API; openapi-typescript and other tooling crash. Use latest 6.x until ecosystem supports TS 7
status: proposed
trust: verified
roles: [all]
tags: [frontend, tooling]
paths: [frontend/package.json]
author: human:akbar
created: 2026-11-10T13:09:55Z
updated: 2026-11-10T13:09:55Z
verified: 2026-11-10
---

openapi-typescript 7.13 fails with TS 7.0.2: ts.factory undefined. Re-evaluate when openapi-typescript/typescript-eslint support TS 7.
