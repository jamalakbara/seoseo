---
aimem: 1
id: mem-q3q9rmt6
type: memory
title: Base UI buttons require explicit submit types for form actions
summary: Shared Button preserves the Base UI default type=button. Form actions using only onSubmit must set type=submit explicitly.
status: active
trust: agent
roles: [all]
author: agent:codex
created: 2026-11-14T13:22:57Z
updated: 2026-11-14T13:22:57Z
---

Observed in real M6 browser QA: CrUX and AI Overview buttons were inside forms but rendered type=button, so clicks produced no request and no error. Fixed the four M6 CrUX/AIO/city submit buttons explicitly; keep the shared component default unchanged to avoid accidental submissions by secondary controls. Typecheck, lint and Docker production build passed; browser confirmation follows application recovery.
