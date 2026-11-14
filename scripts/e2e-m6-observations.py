#!/usr/bin/env python3
"""Offline end-to-end M6 regression. ORIGIN DB_CONTAINER FIXTURE_ORIGIN [WORKER_CONTAINER]."""
import concurrent.futures
import http.cookiejar
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

origin, database, fixture = sys.argv[1:4]
worker = sys.argv[4] if len(sys.argv) > 4 else None
assert database.startswith("seoseo-m6check")
owner = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
outsider = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def api(path, method="GET", body=None, status=200, client=owner, bearer=None):
    headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream", "x-seoseo-csrf": "1"}
    if bearer:
        headers["Authorization"] = "Bearer " + bearer
    req = urllib.request.Request(origin + (path if path == "/mcp" else "/api/v1" + path), method=method,
        headers=headers, data=json.dumps(body).encode() if body is not None else None)
    try:
        response = client.open(req, timeout=60)
    except urllib.error.HTTPError as e:
        response = e
    raw = response.read()
    assert response.status in ((status,) if isinstance(status, int) else status), (path, response.status, raw[:800])
    assert b"fixture-serpapi-key" not in raw and b"do-not-leak-provider-body" not in raw
    assert b"fixture-token" not in raw
    return json.loads(raw) if raw else None


def sql(statement):
    return subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"],
        input=statement, text=True, capture_output=True, check=True).stdout.strip()


def counts():
    return json.load(urllib.request.urlopen(fixture + "/counts"))


email = "m6-" + uuid.uuid4().hex + "@example.com"
password = "disposable-m6-password"
api("/auth/sign-up", "POST", {"email": email, "name": "M6 QA", "password": password}, 201)
api("/auth/sign-up", "POST", {"email": "outsider-" + uuid.uuid4().hex + "@example.com", "name": "Other", "password": password}, 201, outsider)
p = api("/projects", "POST", {"name": "M6 acceptance", "domain": "example.com"}, 201)
root = "/projects/" + p["id"]
tracker = api(root + "/local-trackers", "POST", {"name": "M6 cafe", "business_source": "manual", "business_id": "node:123",
    "business_name": "Fixture Cafe", "address": "Jakarta", "latitude": -6.2, "longitude": 106.8,
    "grid_size": 3, "spacing_km": 1, "keywords": ["coffee", "blocked", "interrupted"]}, 201)
scans = root + "/local-trackers/" + tracker["id"] + "/scans"
now = "2026-11-01T00:00:00Z"
result = {"position": 2, "business_id": "target", "name": "Fixture Cafe"}
cell = {"keyword": "coffee", "row": 0, "column": 0, "measured": True, "results": [result], "checked_at": now}
upload = {"target_business_id": "target", "keywords": ["coffee"], "depth": 20, "source_note": "Manual fixture evidence", "observed_at": now,
    "cells": [cell, cell | {"column": 1, "results": []}, cell | {"column": 2, "measured": False, "results": [], "error": "unavailable"}]}
before = counts()
api(scans + "/import", "POST", upload, 404, outsider)
api(scans + "/import", "POST", upload | {"cells": [cell, cell]}, 422)
api(scans + "/import", "POST", upload | {"cells": [cell | {"row": 9}]}, 422)
api(scans + "/import", "POST", upload | {"cells": [cell | {"measured": False}]}, 422)
api(scans + "/import", "POST", upload | {"cells": [cell | {"results": [result | {"website": "javascript:alert(1)"}]}]}, 422)
imported = api(scans + "/import", "POST", upload)
assert imported["status"] == "partial" and counts() == before
view = api(scans + "/" + imported["id"])
m = view["metrics"][0]
assert (m["expected"], m["measured"], m["found"], m["arp"], m["atrp"], m["solv"]) == (9, 2, 1, 2, 11.5, 50)
api(scans + "/" + imported["id"], client=outsider, status=404)
budget = {"target_business_id": "target", "keywords": ["coffee"], "expected_requests": 9}
api(scans, "POST", budget, 409)  # no key
api("/providers/serpapi_observations", "PUT", {"enabled": True, "api_key": "fixture-serpapi-key"})
assert api("/providers/serpapi_observations/test", "POST")["ok"]
api(scans, "POST", budget | {"expected_requests": 8}, 409)
live = api(scans, "POST", budget)
api(scans, "POST", budget, 409)


def wait_run(run):
    for _ in range(100):
        v = api(scans + "/" + run["id"])
        if v["run"]["status"] not in ("queued", "running"):
            return v
        time.sleep(0.5)
    raise AssertionError("local run did not finish")


v = wait_run(live)
assert v["run"]["status"] == "completed" and v["metrics"][0]["measured"] == 9
assert all(c["results"][1]["business_id"] == "target" for c in v["cells"])
before_block = counts().get("q:blocked", 0)
blocked = api(scans, "POST", budget | {"keywords": ["blocked"]})
v = wait_run(blocked)
assert v["run"]["status"] == "failed" and v["metrics"][0]["measured"] == 0
assert counts()["q:blocked"] == before_block + 1

# AIO continuation, no-data, cache and tenant scoping.
aio = root + "/ai-overviews"
query = {"keyword": "token " + uuid.uuid4().hex, "expected_max_requests": 2}
api(aio, "POST", query, 404, outsider)
a = api(aio, "POST", query)
assert a["requests"] == 2 and a["observation"]["data"]["overview"]["status"] == "present"
before = counts()
assert api(aio, "POST", query)["cached"] and counts() == before
absent = api(aio, "POST", query | {"keyword": "absent " + uuid.uuid4().hex})
assert absent["observation"]["data"]["overview"]["status"] == "absent"
api(aio, "POST", query | {"expected_max_requests": 1}, 409)
api(aio, "POST", query | {"keyword": "blocked"}, 503)
api(aio + "/import", "POST", {"keyword": "unknown", "observed_at": now, "source_note": "imported fixture", "response": {}}, 503)
manual_aio = api(aio + "/import", "POST", {"keyword": "Imported summary", "observed_at": now, "source_note": "Saved Google result", "response": {
    "ai_overview": {"page_token": "fixture-token", "text_blocks": [{"snippet": "Observed summary, not an LLM draft."}],
        "references": [{"title": "Unsafe", "link": "javascript:alert(1)"}, {"title": "Source", "link": "https://example.com/source"}]}}})
assert len(manual_aio["data"]["overview"]["sources"]) == 1
assert api(aio)

city = root + "/city-demand"
demand = {"city": "Jakarta", "source_note": "Google Trends fixture export", "period_start": "2026-10-01", "period_end": "2026-10-31",
    "rows": [{"keyword": "coffee", "relative_interest": 75, "national_monthly_searches": 10000}, {"keyword": "tea", "monthly_searches": 0}]}
api(city, "POST", demand, 404, outsider)
api(city, "POST", demand | {"rows": [{"keyword": "coffee", "relative_interest": 101}]}, 422)
saved = api(city, "POST", demand)
assert saved["data"]["rows"][0]["monthly_searches"] is None and saved["data"]["rows"][1]["monthly_searches"] == 0
assert len(api(city)) == 1

key = api("/api-keys", "POST", {"name": "M6 test"}, 201)["secret"]
def mcp(name, args):
    r = api("/mcp", "POST", {"jsonrpc": "2.0", "id": uuid.uuid4().hex, "method": "tools/call", "params": {"name": name, "arguments": args}}, bearer=key)
    return json.loads(r["result"]["content"][0]["text"])
assert mcp("get_local_scan", {"project_id": p["id"], "tracker_id": tracker["id"], "run_id": imported["id"]})["ok"]
assert mcp("list_city_demand", {"project_id": p["id"]})["ok"]
assert mcp("list_google_ai_overviews", {"project_id": p["id"]})["ok"]
assert mcp("capture_google_ai_overview", {"project_id": p["id"], **query})["ok"]
assert mcp("import_google_ai_overview", {"project_id": p["id"], "keyword":"MCP import", "observed_at":now,
    "source_note":"MCP fixture", "response":{"search_metadata":{"status":"Success"}}})["ok"]
assert mcp("import_city_demand", {"project_id": p["id"], "observation":demand})["ok"]
mcp_run=mcp("start_local_scan", {"project_id":p["id"],"tracker_id":tracker["id"],**budget,"keywords":["blocked"]})
assert mcp_run["ok"], mcp_run
wait_run(mcp_run["data"])

# Current website scoping: changing the website must hide historical research.
api(root, "PATCH", {"domain": "changed.example"})
assert api(aio) == [] and api(city) == []
api(root, "PATCH", {"domain": "example.com"})
assert len(api(aio)) >= 3 and len(api(city)) == 2
api(scans + "/" + blocked["id"], "DELETE", status=204)

# Recovery skips an already-attempted cell after a worker dies in a provider request.
if worker:
    before_crash = counts().get("q:interrupted", 0)
    crash = api(scans, "POST", budget | {"keywords": ["interrupted"]})
    for _ in range(100):
        cells = api(scans + "/" + crash["id"])["cells"]
        if cells:
            break
        time.sleep(0.1)
    subprocess.run(["docker", "kill", worker], check=True, capture_output=True)
    attempted = len(cells)
    sql("UPDATE jobs SET status='queued',locked_by=NULL,locked_until=NULL WHERE id='" + crash["job_id"] + "';")
    subprocess.run(["docker", "start", worker], check=True, capture_output=True)
    v = wait_run(crash)
    assert len(v["cells"]) == 9 and v["metrics"][0]["measured"] >= 9 - attempted
    assert counts()["q:interrupted"] - before_crash <= 9

print("PASS: local import validation/coverage, exact identity, budget/active-run guards, durable provider scan, stop on 429, AIO continuation/cache/absence, safe citations, city unknown vs zero, website/tenant isolation, MCP and worker recovery")
print(json.dumps({"project_id": p["id"], "email": email, "password": password, "tracker_id": tracker["id"], "import_run_id": imported["id"]}))
