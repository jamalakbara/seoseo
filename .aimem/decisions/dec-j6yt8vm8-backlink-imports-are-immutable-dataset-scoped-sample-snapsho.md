---
aimem: 1
id: dec-j6yt8vm8
type: decision
title: Backlink imports are immutable dataset-scoped sample snapshots
summary: Diff complete imports only within the same project website and dataset; sample absence is not verified link loss. Project locks serialize baselines and quotas.
status: proposed
trust: agent
roles: [all]
tags: [m5 backlinks]
author: agent:codex
created: 2026-11-12T16:26:58Z
updated: 2026-11-12T16:26:58Z
---

M5 backlink foundation uses immutable user-imported samples rather than converting mutable M4 observations or claiming automatic discovery. First import is a baseline; later same-dataset/domain imports diff source/target pairs. Current membership includes new; missing pairs are lost only relative to the previous imported sample. Empty complete samples are explicit. Max 1000 pairs/import, 100 snapshots/project; deletion retains other snapshots original comparison metadata. History metadata includes prior website identities for cleanup, but their evidence details are hidden when not current. Writes serialize under the project row, timestamps use clock_timestamp after lock acquisition, and reads use repeatable read for consistent metadata/rows. Imports do not fetch URLs or compute a spam score.
