---
aimem: 1
id: mem-d32j6jk0
type: memory
title: core.autocrlf=true rewrote migrations to CRLF and broke sqlx checksums
summary: .gitattributes forces eol=lf for all text; never edit an applied migration (sqlx compares bytes)
status: active
trust: verified
roles: [all]
tags: [db, git, windows]
paths: [.gitattributes, backend/crates/db/migrations/**]
author: human:akbar
created: 2026-11-10T13:57:49Z
updated: 2026-11-10T13:57:49Z
verified: 2026-11-10
kind: gotcha
---

Symptom: 'migration 1 was previously applied but has been modified' after a branch switch on Windows. Fixed 2026-11-10 with * text=auto eol=lf and converting the working tree to LF.
