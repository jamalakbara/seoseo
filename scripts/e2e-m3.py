#!/usr/bin/env python3
"""Offline M3 integration checks against the throwaway compose.test.yaml stack.
Run after smoke.sh: python scripts/e2e-m3.py EMAIL PASSWORD [http://localhost:3100]
Covers rank tracking (fixture SERP: example.com #1, example.org #3, 5 results), the job
dashboard API and the MCP endpoint (auth, tool list, tool calls through the web proxy).
Requires Docker access to pause the throwaway worker for the active-run check.
Override E2E_WORKER_CONTAINER if the test compose project is not seoseo-ci.
Run sequentially with other E2E scripts: provider settings are workspace-wide.
"""
import http.cookiejar
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

email, password = sys.argv[1:3]
origin = sys.argv[3] if len(sys.argv) > 3 else "http://localhost:3100"
base = origin + "/api/v1"
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def api(path, method="GET", body=None, expected=200):
    req = urllib.request.Request(base + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "X-Seoseo-Csrf": "1"})
    try:
        response = opener.open(req, timeout=120)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == expected, (path, response.status, raw.decode())
    return json.loads(raw) if raw else None


def wait_run(root, tid, seconds=120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        view = api(f"{root}/rank-trackers/{tid}")
        run = view["runs"][0]
        if run["status"] in ("completed", "failed"):
            return view
        time.sleep(1)
    raise AssertionError("rank run timed out")


def mcp(method, params=None, key=None, expected=200, notification=False):
    """One JSON-RPC message to /mcp through the web proxy. Returns the result object."""
    msg = {"jsonrpc": "2.0", "method": method}
    if not notification:
        msg["id"] = str(uuid.uuid4())
    if params is not None:
        msg["params"] = params
    headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream",
               "MCP-Protocol-Version": "2025-06-18"}
    if key:
        headers["Authorization"] = "Bearer " + key
    req = urllib.request.Request(origin + "/mcp", method="POST", data=json.dumps(msg).encode(), headers=headers)
    try:
        response = urllib.request.urlopen(req, timeout=120)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read().decode()
    assert response.status == expected, (method, response.status, raw[:400])
    if expected != 200 or notification or not raw.strip():
        return response
    if response.headers.get("Content-Type", "").startswith("text/event-stream"):
        raw = next(line[5:] for line in raw.splitlines() if line.startswith("data:") and line[5:].strip())
    reply = json.loads(raw)
    assert "error" not in reply, reply
    return reply["result"]


def tool(name, args, key):
    result = mcp("tools/call", {"name": name, "arguments": args}, key)
    payload = json.loads(result["content"][0]["text"])
    assert payload["ok"] == (not result.get("isError", False)), result
    return payload


api("/auth/sign-in", "POST", {"email": email, "password": password})
project = api("/projects", "POST", {"name": "M3 offline integration", "domain": "www.example.org"}, 201)
other = api("/projects", "POST", {"name": "M3 other project"}, 201)
root = "/projects/" + project["id"]
key_row = api("/api-keys", "POST", {"name": "m3 e2e"}, 201)
key = key_row["secret"]
key_revoked = False
try:
    # ---- rank tracking: validation
    api(f"/projects/{other['id']}/rank-trackers", "POST", {}, 422)  # no domain anywhere
    api(root + "/rank-trackers", "POST", {"schedule": "hourly"}, 422)
    api(root + "/rank-trackers", "POST", {"hour_utc": 24}, 422)
    tracker = api(root + "/rank-trackers", "POST",
                  {"schedule": "weekly", "weekday": 3, "hour_utc": 5,
                   "keywords": ["Fixture  Keyword A", "fixture keyword a", "fixture keyword b"]}, 201)
    tid = tracker["id"]
    assert tracker["domain"] == "example.org" and tracker["keyword_count"] == 2, tracker
    assert tracker["next_run_at"] and tracker["schedule"] == "weekly"
    api(f"/projects/{other['id']}/rank-trackers/{tid}", expected=404)  # project isolation
    est = api(f"{root}/rank-trackers/{tid}/estimate")
    assert est["requests_per_run"] == 2 and est["runs_per_month"] == 4 and est["requests_per_month"] == 8
    assert est["provider"] == "serp_searxng" and est["note"] is None, est

    view = api(f"{root}/rank-trackers/{tid}")
    assert all(k["position"] is None and k["checked_at"] is None for k in view["keywords"]), "unchecked must be null"

    # ---- no provider: the run fails with an instruction and stores nothing
    api("/providers/serp_searxng", "PUT", {"enabled": False})
    assert api(f"{root}/rank-trackers/{tid}/estimate")["provider"] is None
    api(f"{root}/rank-trackers/{tid}/runs", "POST", {}, 202)
    view = wait_run(root, tid)
    assert view["runs"][0]["status"] == "failed" and "provider" in view["runs"][0]["error"], view["runs"][0]
    assert all(k["checked_at"] is None for k in view["keywords"])
    api("/providers/serp_searxng", "PUT", {"enabled": True})

    # ---- a real run
    # Hold the throwaway worker so a fast fixture cannot finish between requests.
    worker = os.environ.get("E2E_WORKER_CONTAINER", "seoseo-ci-worker-1")
    subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
    try:
        run = api(f"{root}/rank-trackers/{tid}/runs", "POST", {}, 202)
        assert run["keywords_total"] == 2 and run["job_id"]
        api(f"{root}/rank-trackers/{tid}/runs", "POST", {}, 409)
    finally:
        subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)
    view = wait_run(root, tid)
    assert view["runs"][0]["status"] == "completed" and view["runs"][0]["keywords_done"] == 2, view["runs"][0]
    for k in view["keywords"]:
        assert k["position"] == 3 and k["url"] == "https://example.org/three", k
        assert k["depth"] == 5 and k["provider"] == "serp_searxng" and k["exact"] is False, k
        assert k["previous_position"] is None and k["best_position"] == 3
    assert view["tracker"]["last_run_at"]

    # ---- add a keyword, subset run, pin, history, trend
    change = api(f"{root}/rank-trackers/{tid}/keywords", "POST", {"keywords": ["fixture keyword c", "fixture keyword a"]})
    assert change["changed"] == 1 and change["keyword_count"] == 3, change
    view = api(f"{root}/rank-trackers/{tid}")
    kw_a = next(k for k in view["keywords"] if k["keyword"] == "fixture keyword a")
    kw_c = next(k for k in view["keywords"] if k["keyword"] == "fixture keyword c")
    run = api(f"{root}/rank-trackers/{tid}/runs", "POST", {"keyword_ids": [kw_a["keyword_id"]]}, 202)
    assert run["keywords_total"] == 1
    view = wait_run(root, tid)
    kw_a = next(k for k in view["keywords"] if k["keyword"] == "fixture keyword a")
    assert kw_a["previous_position"] == 3 and kw_a["previous_checked_at"], kw_a
    assert next(k for k in view["keywords"] if k["keyword"] == "fixture keyword c")["checked_at"] is None
    api(f"{root}/rank-trackers/{tid}/keywords/pin", "POST", {"ids": [kw_c["keyword_id"]]})
    assert api(f"{root}/rank-trackers/{tid}")["keywords"][0]["keyword"] == "fixture keyword c", "pinned first"
    history = api(f"{root}/rank-trackers/{tid}/history?keyword_id={kw_a['keyword_id']}")
    assert [p["position"] for p in history] == [3, 3]
    trend = api(f"{root}/rank-trackers/{tid}/trend")
    assert len(trend) == 2 and trend[0]["checked"] == 2 and trend[0]["top3"] == 2 and trend[0]["avg_position"] == 3
    removed = api(f"{root}/rank-trackers/{tid}/keywords/delete", "POST", {"ids": [kw_c["keyword_id"]]})
    assert removed["changed"] == 1 and removed["keyword_count"] == 2

    # ---- a domain that does not rank: measured "not found", not zero and not "unchecked"
    absent = api(root + "/rank-trackers", "POST", {"domain": "not-ranking.dev", "keywords": ["fixture keyword a"]}, 201)
    api(f"{root}/rank-trackers/{absent['id']}/runs", "POST", {}, 202)
    row = wait_run(root, absent["id"])["keywords"][0]
    assert row["position"] is None and row["checked_at"] and row["depth"] == 5 and row["url"] is None, row
    t = api(f"{root}/rank-trackers/{absent['id']}/trend")[0]
    assert t["ranking"] == 0 and t["avg_position"] is None, t
    updated = api(f"{root}/rank-trackers/{tid}", "PATCH", {"schedule": "manual"})
    assert updated["next_run_at"] is None
    print("PASS rank tracking: validation, isolation, runs, subset, null semantics, history, trend", flush=True)

    # ---- job dashboard
    summary = api("/jobs/summary")
    assert summary["completed"] >= 3 and summary["failed"] == 0, summary  # a refused run is not a crashed job
    jobs = api("/jobs?status=completed&limit=5")
    assert 0 < len(jobs) <= 5 and all(j["status"] == "completed" for j in jobs)
    assert any(j["kind"] == "rank_check" for j in api("/jobs"))
    print("PASS job dashboard: summary and status filter", flush=True)

    # ---- MCP
    mcp("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "e2e", "version": "1"}}, key=None, expected=401)
    mcp("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "e2e", "version": "1"}}, key="seo_not_a_real_key", expected=401)
    init = mcp("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                              "clientInfo": {"name": "e2e", "version": "1"}}, key)
    assert init["serverInfo"]["name"] == "seoseo" and "tools" in init["capabilities"], init
    assert "null" in init["instructions"]
    tools = mcp("tools/list", {}, key)["tools"]
    names = {t["name"] for t in tools}
    for expected_tool in ("whoami", "list_projects", "research_keywords", "get_serp_results", "run_site_audit",
                          "get_audit_issues", "create_rank_tracker", "run_rank_tracker", "get_rank_history",
                          "estimate_rank_tracker_cost", "list_jobs"):
        assert expected_tool in names, expected_tool
    assert len(tools) >= 29 and all(t["inputSchema"]["type"] == "object" and t.get("description") for t in tools)

    who = tool("whoami", {}, key)
    assert who["ok"] and who["data"]["via"] == "api_key" and who["data"]["user"]["email"] == email, who
    listed = tool("list_projects", {}, key)["data"]
    assert any(p["id"] == project["id"] for p in listed)
    got = tool("get_rank_tracker", {"project_id": project["id"], "tracker_id": tid}, key)
    assert got["ok"] and got["data"]["tracker"]["domain"] == "example.org" and len(got["data"]["keywords"]) == 2
    hist = tool("get_rank_history", {"project_id": project["id"], "tracker_id": tid,
                                     "keyword_id": kw_a["keyword_id"]}, key)["data"]
    assert len(hist["history"]) == 2 and len(hist["trend"]) == 2
    serp = tool("get_serp_results", {"project_id": project["id"], "keyword": "fixture keyword a"}, key)
    assert serp["ok"] and serp["data"]["items"][0]["domain"] == "example.com" and serp["data"]["kd"] is None, serp
    # Errors are data with a code and an instruction, not protocol failures.
    bad = tool("get_rank_tracker", {"project_id": "nope", "tracker_id": tid}, key)
    assert not bad["ok"] and bad["error"]["code"] == "invalid_input" and "list" in bad["error"]["message"], bad
    missing = tool("get_rank_tracker", {"project_id": str(uuid.uuid4()), "tracker_id": tid}, key)
    assert not missing["ok"] and missing["error"]["code"] == "not_found", missing
    made = tool("create_rank_tracker", {"project_id": project["id"], "domain": "example.com",
                                        "keywords": ["fixture keyword z"]}, key)
    assert made["ok"] and made["data"]["keyword_count"] == 1, made
    started = tool("run_rank_tracker", {"project_id": project["id"], "tracker_id": made["data"]["id"]}, key)
    assert started["ok"] and started["data"]["status"] == "queued", started
    assert wait_run(root, made["data"]["id"])["keywords"][0]["position"] == 1
    # A revoked key stops working immediately.
    api("/api-keys/" + key_row["key"]["id"], "DELETE", expected=204)
    key_revoked = True
    mcp("tools/list", {}, key, expected=401)
    print(f"PASS MCP: auth, {len(tools)} tools, tool calls, instructional errors, key revocation", flush=True)
finally:
    api("/providers/serp_searxng", "PUT", {"enabled": True})
    if not key_revoked:
        api("/api-keys/" + key_row["key"]["id"], "DELETE", expected=204)
    for p in (project, other):
        api("/projects/" + p["id"] + "/archive", "POST")
        api("/projects/" + p["id"], "DELETE", expected=204)
