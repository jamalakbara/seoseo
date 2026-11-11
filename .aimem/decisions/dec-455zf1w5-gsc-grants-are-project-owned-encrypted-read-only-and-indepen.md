---
aimem: 1
id: dec-455zf1w5
type: decision
title: GSC grants are project-owned, encrypted, read-only and independent
summary: Each project owns a separate Google grant; admins connect/select, members read; disconnect deletes local grant and pending state.
status: proposed
trust: verified
roles: [all]
author: human:akbar
created: 2026-11-11T17:44:30Z
updated: 2026-11-11T17:44:30Z
verified: 2026-11-11
---

Use single-use hashed OAuth state bound to user/workspace/project plus S256 PKCE. Refresh tokens and verifiers are AES-GCM encrypted. Do not revoke the whole Google grant on disconnect because it can affect other project connections. Requests refresh access on demand; no shared cross-project token cache. Endpoint overrides are operator-only and used by the local GSC fixture. Google remains the authority for verified properties and URL ownership. Performance enforces row/date/filter bounds and reports measured source metadata.
