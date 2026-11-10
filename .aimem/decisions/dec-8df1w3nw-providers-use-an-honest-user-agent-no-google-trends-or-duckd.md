---
aimem: 1
id: dec-8df1w3nw
type: decision
title: Providers use an honest User-Agent; no Google Trends or DuckDuckGo adapter
summary: Endpoints that only answer browser-UA spoofing plus cookie handshakes count as bot-check circumvention and are not built
status: proposed
trust: agent
roles: [all]
tags: [providers, ethics, m2]
paths: [backend/crates/providers/**]
author: agent:claude-code:fable-5.2
created: 2026-11-10T17:15:08Z
updated: 2026-11-10T17:15:08Z
---

The unofficial Google Trends API needs a browser User-Agent and a NID cookie handshake; DuckDuckGo suggest returns empty to a non-browser UA. Both mean working around bot detection, which conflicts with the rule that scraping is opt-in and never bypasses bot checks.

Chosen instead: Google + Bing suggest (work with our real UA), Wikipedia pageviews (official API) for labelled volume and trend, relative popularity 0-100 for everything else, volume null when there is no measured signal.

Rejected: UA spoofing; inventing volume numbers from autocomplete rank. Revisit Trends only via an official or BYOK route.
