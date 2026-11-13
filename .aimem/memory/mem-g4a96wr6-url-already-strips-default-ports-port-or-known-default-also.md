---
aimem: 1
id: mem-g4a96wr6
type: memory
title: Url already strips default ports; port_or_known_default also returns explicit custom ports
summary: Fixed crawler normalization to preserve non-default ports. Validate disallowed ports before normalization in root-only lookups.
status: active
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-13T03:46:52Z
updated: 2026-11-13T03:46:52Z
kind: gotcha
---

Technology URL validation exposed crawler::urls::normalize removing port 1234: port() == port_or_known_default() is true for explicit non-default ports too. Removed that block; Url parses default ports canonically already. Regression verifies http://example.com:8080/a retains port 8080 while dropping fragments. Existing stored audits are unchanged. New technology lookup independently rejects custom ports before normalization.
