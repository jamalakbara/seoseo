---
aimem: 1
id: dec-xdah5yft
type: decision
title: M5 native AI engines distinguish grounded and plain answers
summary: Ollama/compatible/Gemini are text-only; Anthropic requests search/citations. UTC schedules claim atomically, pause at 20 runs; cloud-live acceptance remains separate.
status: proposed
trust: verified
roles: [all]
tags: [m5 ai]
author: human:akbar
created: 2026-11-12T23:10:30Z
updated: 2026-11-12T23:10:30Z
verified: 2026-11-12
supersedes: dec-dggz25sq
---

Trackers support compatible gateway plus native Ollama, Anthropic and Gemini, immutable input snapshots and preserved partial results. Anthropic basic web search preserves provider citations. Gemini deliberately omits Google Search grounding: its published use restrictions prohibit the automated collection/analysis needed by this answer database (https://ai.google.dev/gemini-api/terms#grounding-with-google-search); unexpected grounding metadata fails closed. Rates/SoV and citation domains use explicit denominators and retained-run coverage. Native cloud fixtures are not live verification. Real smollm2:135m Ollama passed; the exact three-live-engine roadmap gate needs operator cloud credentials. Scheduled daily/weekly/monthly UTC due claims serialize with next-run advancement under the project lock; retention pauses instead of deleting evidence.
