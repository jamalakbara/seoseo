---
aimem: 1
id: dec-r2qwhazb
type: decision
title: Invitations are link-based capabilities with email binding; invitees join as admin
summary: Token hashed at rest, 7-day expiry, single use; accept/sign-up requires the invited email; invite sign-up bypasses closed sign-up and creates no personal workspace
status: proposed
trust: verified
roles: [all]
tags: [auth, team]
paths: [backend/crates/domain/src/team.rs, backend/crates/db/src/invitations.rs]
author: human:akbar
created: 2026-11-10T14:18:15Z
updated: 2026-11-10T14:18:15Z
verified: 2026-11-10
---

Matches OpenSEO spec 0011 (every invitee admin). Email delivery waits for SMTP (WS-03); the UI shows a copyable link.
