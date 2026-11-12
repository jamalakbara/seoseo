#!/usr/bin/env python3
"""Dashboard/onboarding regression on a disposable password-auth stack + GSC fixture.

Usage: python scripts/e2e-dashboard.py [http://127.0.0.1:3200]
Creates isolated users/projects; never run against production.
"""
import concurrent.futures
import http.cookiejar
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:3200"
fixture = "http://127.0.0.1:3101"


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def session():
    return urllib.request.build_opener(
        urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()), NoRedirect()
    )


owner = session()


def request(path, method="GET", body=None, expected=200, client=owner, key=None):
    url = path if path.startswith("http") else origin + "/api/v1" + path
    headers = {"Content-Type": "application/json", "X-Seoseo-Csrf": "1", "Accept": "application/json, text/event-stream"}
    if key:
        headers["Authorization"] = "Bearer " + key
    req = urllib.request.Request(url, method=method, headers=headers,
                                 data=json.dumps(body).encode() if body is not None else None)
    try:
        response = client.open(req, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == expected, (path, response.status, raw[:500])
    if expected in (302, 303):
        return response.headers["Location"]
    return json.loads(raw) if raw else None


def signup(client):
    return request("/auth/sign-up", "POST", {
        "email": "dashboard-" + uuid.uuid4().hex + "@example.com",
        "password": "disposable-dashboard-test-password", "name": "Dashboard test"
    }, expected=201, client=client)


def wait_snapshot(root, error=False):
    for _ in range(60):
        data = request(root + "/dashboard/keywords")
        if not data["refreshing"] and (data["error"] if error else data["snapshot"]):
            return data
        time.sleep(1)
    raise AssertionError("dashboard refresh timed out")


signup(owner)
outsider = session()
signup(outsider)
project = request("/projects", "POST", {"name": "Dashboard regression", "domain": "badseo.test"}, 201)
root = "/projects/" + project["id"]
key = None
try:
    # No provider or audit data is represented honestly; scopes are checked on every surface.
    overview = request(root + "/dashboard/overview")
    assert not overview["latest_audit"] and not overview["keywords"]["snapshot"]
    for suffix in ("/dashboard/overview", "/dashboard/keywords", "/website-setup"):
        request(root + suffix, expected=404, client=outsider)
    request(root + "/dashboard/refresh", "POST", expected=409)
    request(root + "/dashboard/checklist", "PUT", {"step": "gsc", "skipped": True}, 204)
    assert next(s for s in request(root + "/dashboard/overview")["checklist"] if s["id"] == "gsc")["skipped"]
    request(root + "/dashboard/checklist", "PUT", {"step": "invalid", "skipped": True}, 422)

    # Concurrent onboarding visits must share one audit/job.
    request(root + "/website-research", "POST", {"start_url": "http://other.example/"}, 422)
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        audits = list(pool.map(lambda _: request(root + "/website-research", "POST", {"start_url": "http://badseo.test/"}), range(6)))
    assert len({a["id"] for a in audits}) == 1
    for _ in range(60):
        setup = request(root + "/website-setup")
        if setup["audit"]["status"] == "completed":
            break
        time.sleep(1)
    assert setup["evidence"] and setup["suggested_overview"], setup
    context = request(root + "/context", "PUT", {
        "revision": 0, "sections": [{"key": "audience", "title": "Audience", "content": "Small teams"}],
        "competitors": [{"domain": "competitor.com"}], "key_pages": [], "research_note": "Initial context"
    })
    save = {"revision": context["revision"], "expected_domain": "badseo.test", "business_overview": "Reviewed business", "current_goal": "Qualified leads", "key_pages": [setup["evidence"][0]["url"]]}
    updated = request(root + "/website-setup", "POST", save)
    assert updated["revision"] == 2 and len(updated["sections"]) == 3
    assert updated["competitors"][0]["domain"] == "competitor.com" and len(updated["research_log"]) == 2
    request(root + "/website-setup", "POST", save, 409)
    save["revision"] = updated["revision"]
    save["current_goal"] = "Updated goal"
    updated = request(root + "/website-setup", "POST", save)
    assert updated["revision"] == 3
    request(root, "PATCH", {"domain": "example.org"})
    request(root + "/website-setup", "POST", {**save, "revision": 3}, 409)
    assert request(root + "/dashboard/overview")["latest_audit"] is None
    print("PASS onboarding: deduplication, evidence, memory preservation, repeated updates, stale revisions/domain, tenant isolation", flush=True)

    # Offline OAuth connection and sampled keyword views, including truncated baseline verification.
    request(fixture + "/control", "POST", {"mode": "dashboard-truncated"})
    url = request(root + "/gsc/connect", "POST")["url"]
    callback = request(url, expected=302)
    parsed = urllib.parse.urlsplit(callback)
    request(origin + parsed.path + "?" + parsed.query, expected=303)
    request(root + "/gsc/site", "PUT", {"site_url": "sc-domain:example.org"})
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        claims = list(pool.map(lambda _: request(root + "/dashboard/refresh", "POST"), range(6)))
    assert len({r["job_id"] for r in claims}) == 1
    snapshot = wait_snapshot(root)
    assert snapshot["snapshot"]["truncated"]
    assert [r["keys"][0] for r in snapshot["snapshot"]["new_queries"]] == ["new query"]
    assert [r["keys"][0] for r in snapshot["snapshot"]["striking_distance"]] == ["opportunity"]
    queries = request(fixture + "/stats")["queries"]
    assert request(root + "/dashboard/refresh", "POST")["job_id"] == snapshot["job_id"]
    time.sleep(1)
    assert request(fixture + "/stats")["queries"] == queries
    request(root + "/gsc/site", "PUT", {"site_url": "https://example.org/"})
    assert request(root + "/dashboard/keywords")["snapshot"] is None
    request(fixture + "/control", "POST", {"mode": "quota"})
    request(root + "/dashboard/refresh", "POST")
    failure = wait_snapshot(root, error=True)
    assert failure["next_attempt_at"] and not failure["snapshot"]
    assert request(root + "/dashboard/refresh", "POST")["job_id"] == failure["job_id"]
    request(root + "/gsc", "DELETE", expected=204)
    assert request(root + "/dashboard/keywords")["snapshot"] is None
    print("PASS dashboard: concurrent claims, 24h cache, new/striking queries, truncation, source invalidation, provider error/backoff", flush=True)

    key = request("/api-keys", "POST", {"name": "Dashboard regression"}, 201)
    for name in ("get_dashboard_overview", "get_dashboard_keywords", "get_project_website_setup"):
        result = request(origin + "/mcp", "POST", {
            "jsonrpc": "2.0", "id": name, "method": "tools/call",
            "params": {"name": name, "arguments": {"project_id": project["id"]}}
        }, key=key["secret"])
        assert json.loads(result["result"]["content"][0]["text"])["ok"], result
    request(root + "/archive", "POST")
    request(root + "/website-setup", "POST", {**save, "expected_domain": "example.org", "revision": 3, "key_pages": []}, 409)
    print("PASS MCP dashboard/onboarding reads and archived-memory protection", flush=True)
finally:
    request(fixture + "/control", "POST", {"mode": "normal"})
    if key:
        request("/api-keys/" + key["key"]["id"], "DELETE", expected=204)
    request(root + "/archive", "POST")
    request(root, "DELETE", expected=204)
