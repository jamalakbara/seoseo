---
aimem: 1
id: dec-cmazw237
type: decision
title: MCP server is stateless rmcp Streamable HTTP behind the REST auth layer
summary: NeverSessionManager + JSON responses; Actor injected via request extensions; tools call domain services only; Host allow-list from WEB_BASE_URL
status: proposed
trust: agent
roles: [all]
tags: [mcp, m3]
paths: [backend/crates/mcp/**, backend/crates/api/src/mcp_mount.rs]
author: agent:claude-code:fable-5.2
created: 2026-11-11T16:30:12Z
updated: 2026-11-11T16:30:12Z
---

The MCP endpoint uses rmcp 3.9 StreamableHttpService with NeverSessionManager, legacy_session_mode=false and json_response=true: no Mcp-Session-Id, plain JSON replies, so any API replica can serve any request and nothing is lost on restart. Rejected: LocalSessionManager (in-memory sessions break behind several replicas) and a hand-rolled JSON-RPC endpoint (less protocol coverage).

Auth is an axum layer in api/src/mcp_mount.rs that reuses the REST Auth extractor and puts the Actor in the request extensions; tools read it from RequestContext -> http Parts. Tools never see credentials and only call domain services, so RBAC and tenant scoping are identical to REST.

rmcp's default Host allow-list is loopback only. We pass WEB_BASE_URL host + localhost + compose names (api, web). A deployment reachable under another hostname must have WEB_BASE_URL set to it or MCP answers 403.

Tool inputs are separate schemars structs (ids as strings) converted to the domain inputs by a JSON round trip, so domain types need no schemars derive.
