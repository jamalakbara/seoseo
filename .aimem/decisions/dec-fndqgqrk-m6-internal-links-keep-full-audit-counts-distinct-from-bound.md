---
aimem: 1
id: dec-fndqgqrk
type: decision
title: M6 internal links keep full-audit counts distinct from bounded graph evidence
summary: Completed-audit read-only graph; counts use all stored links, sample edges cap at 5000 and disable suggestions if truncated. Suggestions are lexical editorial hints.
status: proposed
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T00:40:34Z
updated: 2026-11-13T00:40:34Z
---

SA-12 uses a repeatable-read read-only transaction scoped to org/project/audit. Nodes are first 10-500 pages alphabetically, edges unique source-target pairs excluding self, with duplicate occurrences and any-followed evidence. Distinct incoming/followed counts use the full stored audit so excluded sample sources cannot create false low-link signals. Unfetched targets are unknown. Suggestions require eligible canonical indexable HTML 200 pages in crawl scope and >=2 shared title/H1 tokens, target <=1 followed source, excluding existing nofollow pairs. At most 3 sources/target and 50 total. Disable all suggestions when capped graph edges omit possible existing pairs. This avoids false website-wide orphan claims or automatic content mutations.
