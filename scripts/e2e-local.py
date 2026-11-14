#!/usr/bin/env python3
"""Offline local discovery, tracker isolation, quota and MCP regression."""
import http.cookiejar
import json
import sys
import urllib.error
import urllib.request
import uuid

origin = sys.argv[1]
owner = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
outsider = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def api(path, method="GET", body=None, status=200, client=owner, bearer=None):
    headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream", "x-seoseo-csrf": "1"}
    if bearer:
        headers["Authorization"] = "Bearer " + bearer
    request = urllib.request.Request(origin + (path if path == "/mcp" else "/api/v1" + path), method=method,
        headers=headers, data=json.dumps(body).encode() if body is not None else None)
    try:
        response = client.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status in ((status,) if isinstance(status, int) else status), (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def signup(client):
    return api("/auth/sign-up", "POST", {"email": "local-" + uuid.uuid4().hex + "@example.com",
        "name": "Local test", "password": "disposable-local-password"}, 201, client)


signup(owner)
signup(outsider)
project = api("/projects", "POST", {"name": "Local regression", "domain": "example.com"}, 201)
root = "/projects/" + project["id"]
query = {"query": "Fixture Cafe " + uuid.uuid4().hex, "near": "Jakarta"}
api(root + "/local/businesses/search", "POST", query, 404, outsider)
result = api(root + "/local/businesses/search", "POST", query)
assert not result["cached"] and result["businesses"][0]["website"] is None
assert api(root + "/local/businesses/search", "POST", query)["cached"]
assert api("/local/categories")
body = {"business_source": "openstreetmap", "business_id": "node:123", "business_name": "Fixture Cafe",
        "address": "Jakarta", "latitude": -6.2, "longitude": 179.99, "grid_size": 9, "spacing_km": 10,
        "keywords": ["Coffee", "coffee", "cafe"]}
api(root + "/local-trackers", "POST", body | {"grid_size": 4}, 422)
t = api(root + "/local-trackers", "POST", body, 201)
assert t["keyword_count"] == 2
api(root + "/local-trackers", "POST", body, 409)
api(root + "/local-trackers/" + t["id"] + "/grid", client=outsider, status=404)
grid = api(root + "/local-trackers/" + t["id"] + "/grid")
assert grid["status"] == "unmeasured" and len(grid["points"]) == 81
assert all(p["rank"] is None and not p["measured"] and -180 <= p["longitude"] <= 180 for p in grid["points"])
other = api("/projects", "POST", {"name": "Other", "domain": "other.example"}, 201)
api("/projects/" + other["id"] + "/local-trackers/" + t["id"] + "/grid", status=404)
from concurrent.futures import ThreadPoolExecutor

def create(i):
    client = urllib.request.build_opener()
    return api(root + "/local-trackers", "POST", body | {"name": "concurrent-" + str(i)}, (201, 409), client, key)

key = api("/api-keys", "POST", {"name": "Local MCP"}, 201)["secret"]
with ThreadPoolExecutor(max_workers=12) as executor:
    list(executor.map(create, range(12)))
assert len(api(root + "/local-trackers")) == 10
result = api("/mcp", "POST", {"jsonrpc": "2.0", "id": "local", "method": "tools/call",
    "params": {"name": "get_local_grid_preview", "arguments": {"project_id": project["id"], "tracker_id": t["id"]}}}, bearer=key)
assert json.loads(result["result"]["content"][0]["text"])["data"]["status"] == "unmeasured"
print("PASS: cached discovery, safe websites, validation, atomic keyword saving, tenant/project isolation, grid wrapping, concurrent quota and MCP preview")
