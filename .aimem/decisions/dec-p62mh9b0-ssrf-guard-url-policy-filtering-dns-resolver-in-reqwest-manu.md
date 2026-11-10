---
aimem: 1
id: dec-p62mh9b0
type: decision
title: SSRF guard = URL policy + filtering DNS resolver in reqwest; manual redirects
summary: Every fetch checks URL (scheme, creds, internal names, IP literals) and the resolver drops private/reserved IPs, covering redirects and DNS rebinding
status: proposed
trust: verified
roles: [all]
tags: [security, crawler]
paths: [backend/crates/crawler/src/policy.rs, backend/crates/crawler/src/fetch.rs]
author: human:akbar
created: 2026-11-10T14:18:14Z
updated: 2026-11-10T14:18:14Z
verified: 2026-11-10
---

CRAWLER_ALLOW_PRIVATE=true only via deploy/compose.test.yaml for the badseo fixture. API also validates start URLs (422).
