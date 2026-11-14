---
aimem: 1
id: dec-gxbjb3j7
type: decision
title: M7 assistant streams durable drafts and requires atomic reviewed memory approvals
summary: Real model SSE feeds private labelled drafts; no-transform prevents Next compression buffering. Admin-reviewed notes append with memory CAS and one-use proposal marking in one transaction.
status: proposed
trust: agent
roles: [all]
tags: [m7 assistant memory streaming]
author: agent:codex
created: 2026-11-14T17:05:55Z
updated: 2026-11-14T17:05:55Z
supersedes: dec-jqvfq115
---

Supersedes the initial read-only assistant delivery only for streaming and reviewed writes. Read tools remain bounded and cannot mutate. Worker inference stays single-attempt with a five-call/four-tool budget; dropped streams fail and clear drafts, never replay. Complete final actions are validated before answers/proposals are final. Private SSE reads recheck membership and never dispatch model calls. Only a chat owner with admin rights can explicitly approve an editable 2000-character note into canonical research memory. Revision, active website/market, one-use approval and memory append are checked under the same transaction. Normal local stack remains on earlier code; other M7 features stay open.
