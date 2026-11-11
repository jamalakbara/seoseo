#!/usr/bin/env python3
"""Offline M2 integration checks against the throwaway compose.test.yaml stack.
Run after smoke.sh: python scripts/e2e-m2.py EMAIL PASSWORD [http://localhost:3100]
Uses the local fixture SERP endpoint; no paid keys or live search requests.
"""
import http.cookiejar
import json
import sys
import time
import urllib.error
import urllib.request
import uuid

email, password = sys.argv[1:3]
base = (sys.argv[3] if len(sys.argv) > 3 else "http://localhost:3100") + "/api/v1"
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


def wait(path, seconds=180):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = api(path)
        if value["status"] in ("completed", "failed", "cancelled"):
            assert value["status"] == "completed", value
            return value
        time.sleep(1)
    raise AssertionError("Timed out: " + path)


api("/auth/sign-in", "POST", {"email": email, "password": password})
providers = api("/providers")
browser = next(p for p in providers if p["id"] == "serp_chromium")
assert not browser["enabled"], "browser search must require opt-in"
api("/providers/wikipedia_pageviews", "PUT", {"enabled": False})
try:
    project = api("/projects", "POST", {"name": "M2 offline integration", "domain": "badseo.test"}, 201)
    root = "/projects/" + project["id"]
    api(root + "/saved-keywords/refresh", "POST", {}, 422)
    api(root + "/saved-keywords", "POST", {"keywords": [f"fixture keyword {i}" for i in range(12)], "tags": ["retain"]})
    rows = api(root + "/saved-keywords")["items"]
    api(root + "/saved-keywords/refresh", "POST", {"ids": [str(uuid.uuid4())]}, 422)
    other = api("/projects", "POST", {"name": "Other M2 project"}, 201)
    api("/projects/" + other["id"] + "/saved-keywords/refresh", "POST", {"ids": [rows[0]["id"]]}, 422)
    api("/providers/serp_searxng", "PUT", {"enabled": False})
    api(root + "/saved-keywords/refresh", "POST", {}, 409)
    api("/providers/serp_searxng", "PUT", {"enabled": True})
    job = api(root + "/saved-keywords/refresh", "POST", {}, 202)
    api(root + "/saved-keywords/refresh", "POST", {}, 409)
    done = wait("/jobs/" + job["id"])
    assert done["progress"]["done"] == 12
    rows = api(root + "/saved-keywords")["items"]
    assert all(k["metrics_updated_at"] and k["tags"] == ["retain"] for k in rows)
    assert all(k["kd"] is None for k in rows), "unknown authority must not become zero"
    assert all(k["est_volume"] is None for k in rows), "disabled demand source must not invent volume"
    selected = api(root + "/saved-keywords/refresh", "POST", {"ids": [rows[0]["id"], rows[0]["id"]]}, 202)
    assert wait("/jobs/" + selected["id"])["progress"]["done"] == 1
    print("PASS bulk refresh, duplicate suppression, selection, project isolation, tags and null metrics", flush=True)

    audit = api(root + "/audits", "POST", {"start_url": "http://badseo/rendered.html", "max_pages": 1,
        "render_js": True, "lighthouse": True, "delay_ms": 0}, 202)
    ar = root + "/audits/" + audit["id"]
    wait(ar)
    page = api(ar + "/pages")["items"][0]
    detail = api(ar + "/pages/" + page["id"])
    assert detail["page"]["h1"] == "Rendered heading", detail
    result = detail["page"]["browser_result"]
    assert result["rendered"] and result["lighthouse"]["scores"]["performance"] is not None, result
    assert not any(i["issue_type"] in ["render-failed", "lighthouse-failed", "javascript-rendering-suspected"] for i in detail["issues"])
    print("PASS rendered DOM, persisted Lighthouse scores, page detail API", flush=True)
    fallback = api(root + "/audits", "POST", {"start_url": "http://badseo/render-failure.html", "max_pages": 1,
        "render_js": True, "delay_ms": 0}, 202)
    ar = root + "/audits/" + fallback["id"]
    wait(ar)
    page = api(ar + "/pages")["items"][0]
    detail = api(ar + "/pages/" + page["id"])
    assert detail["page"]["h1"] == "Static fallback heading"
    assert any(i["issue_type"] == "render-failed" for i in detail["issues"])
    assert detail["page"]["browser_result"] is None
    print("PASS renderer size limit and visible static fallback", flush=True)
finally:
    api("/providers/wikipedia_pageviews", "PUT", {"enabled": True})
