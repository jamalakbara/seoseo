---
aimem: 1
id: dec-tn72jqw3
type: decision
title: Renderer uses Puppeteer and Lighthouse behind a DNS-pinning egress proxy
summary: One isolated Node browser service serves DOM audits, Lighthouse and opt-in Bing SERPs; Rust keeps orchestration and tenant policy.
status: proposed
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-10T22:29:42Z
updated: 2026-11-10T22:29:42Z
---

Use Puppeteer rather than chromiumoxide so Lighthouse shares the same Chromium process and network boundary. Renderer has only an internal Docker network; a separate forward proxy validates all resolved addresses and connects to the validated IP. No public service ports. Browser SERP is disabled by default, respects robots, uses an honest UA, and stops on challenges with an hour cooldown. Lab results live in audit_pages.browser_result, not audit issues, to avoid penalizing successful measurements.
