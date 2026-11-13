#!/usr/bin/env python3
"""Bing Webmaster project isolation, encryption, selection, reports and MCP regression."""
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin, database = sys.argv[1:3]
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
    assert response.status == status, (path, response.status, raw[:500])
    for secret in [b"fixture-bing-key", b"do-not-leak-auth-code", b"do-not-leak-dns-code", b"do-not-leak-provider-body"]:
        assert secret not in raw, (path, raw)
    return json.loads(raw) if raw else None


def signup(client):
    return api("/auth/sign-up", "POST", {"email": "bing-" + uuid.uuid4().hex + "@example.com",
        "name": "Bing test", "password": "disposable-bing-password"}, 201, client)


signup(owner)
signup(outsider)
project = api("/projects", "POST", {"name": "Bing regression", "domain": "example.com"}, 201)
root = "/projects/" + project["id"] + "/bing"
assert not api(root)["connected"]
api(root, client=outsider, status=404)
api(root, "PUT", {"api_key": "wrong-fixture-key"}, 409)
connection = api(root, "PUT", {"api_key": "fixture-bing-key"})
assert connection["connected"] and connection["site_url"] is None and "api_key" not in connection
sites = api(root + "/sites")
assert sites == [{"url": "https://example.com/", "verified": True}, {"url": "https://unverified.example/", "verified": False}]
api(root + "/site", "PUT", {"site_url": "https://unverified.example/"}, 422)
api(root + "/site", "PUT", {"site_url": "https://example.com/"})
for kind in ["queries", "pages", "crawl", "inbound_links"]:
    report = api(root + "/reports/" + kind, "POST", {"page": 1})
    assert report["site_url"] == "https://example.com/" and "__type" not in json.dumps(report)
assert report["data"]["RequestedPage"] == 1 and report["data"]["TotalPages"] == 2

key = api("/api-keys", "POST", {"name": "Bing MCP"}, 201)["secret"]
def mcp(name, arguments):
    result = api("/mcp", "POST", {"jsonrpc": "2.0", "id": uuid.uuid4().hex,
        "method": "tools/call", "params": {"name": name, "arguments": arguments}}, bearer=key)
    return json.loads(result["result"]["content"][0]["text"])

assert mcp("list_bing_webmaster_sites", {"project_id": project["id"]})["ok"]
assert mcp("get_bing_webmaster_report", {"project_id": project["id"], "kind": "queries"})["ok"]
stored = subprocess.run(["docker", "exec", database, "psql", "-At", "-U", "seoseo", "-d", "seoseo", "-c",
    "SELECT encode(api_key_enc,'escape') FROM bing_webmaster_connections WHERE project_id='" + project["id"] + "'"],
    check=True, capture_output=True, text=True).stdout
assert "fixture-bing-key" not in stored and stored.strip()
api(root, "DELETE", status=204)
assert not api(root)["connected"]
print("PASS: tenant isolation, pre-save validation, encrypted key, secret-safe sites, verified selection, scoped REST/MCP reports, paging and disconnect")
