#!/usr/bin/env python3
"""Disposable DO-06/BL-07 regression: API_ORIGIN M6CHECK_DB TECH_FIXTURE_CONTAINER.
Requires deploy/test/m6-tech/nginx.conf with its documented network aliases.
"""
import http.cookiejar
import concurrent.futures
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin, database, fixture = sys.argv[1:4]
assert database.startswith("seoseo-m6check") and fixture.startswith("seoseo-m6check")


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, status=200, client=owner):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method, data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1", "Accept": "application/json, text/event-stream"})
    try:
        response = client.open(request, timeout=35)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status in ((status,) if isinstance(status, int) else status), (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def logs():
    return subprocess.run(["docker", "logs", fixture], text=True, capture_output=True, check=True).stdout


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"tech-links-{uuid.uuid4().hex}@example.com", "password": "disposable-tech-links-password", "name": "Tech and prospect regression"}, 201, client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
jobs_before = sql("SELECT count(*) FROM jobs;")


def project(name, domain, client=owner):
    return api("/projects", "POST", {"name": name, "domain": domain}, 201, client)


p = project("Homepage tech regression", "good.tech-fixture.example")
root = "/projects/" + p["id"]
lookup = root + "/technology"
url = "http://good.tech-fixture.example/"
# Expire test-only cache keys if the script is rerun against the same disposable stack.
sql("DELETE FROM provider_cache WHERE kind IN ('technology_homepage_v1','technology_lookup_claim_v1','technology_project_claim_v1');")
before_logs = logs()
r = api(lookup, "POST", {"url": url})
assert not r["cached"] and r["record"]["status"] == "observed", r
assert [t["name"] for t in r["record"]["technologies"]] == ["Next.js", "Shopify", "WordPress"], r
observed_logs = logs()[len(before_logs):]
assert observed_logs.count("good.tech-fixture.example /") == 2, observed_logs
cached = api(lookup, "POST", {"url": url})
assert cached["cached"] and cached["record"] == r["record"]
assert logs() == before_logs + observed_logs, "Cache hit made a network request"
assert api(lookup, "POST", {"url": url}, client=member) == cached
api(lookup, "POST", {"url": url}, 404, outsider)
api(lookup, "POST", {"url": url}, 401, session())
api(lookup, "POST", {"url": "http://cooldown.tech-fixture.example/"}, 409)
for invalid in ["file:///etc/passwd", "https://user:secret@example.com/", "https://example.com/path", "https://example.com/?token=secret", "http://example.com:1234/"]:
    api(lookup, "POST", {"url": invalid}, 422)
for host, expected in [("robots", "robots_disallowed"), ("bot", "blocked"), ("redirect", "redirect"), ("robotblock", "unknown")]:
    other = project("Tech " + host, host + ".tech-fixture.example")
    before_logs = logs()
    result = api("/projects/" + other["id"] + "/technology", "POST", {"url": f"http://{host}.tech-fixture.example/"})
    assert result["record"]["status"] == expected and result["record"]["technologies"] == [], result
    current_logs = logs()[len(before_logs):]
    if host in ["robots", "robotblock"]:
        assert current_logs.strip() == host + ".tech-fixture.example /robots.txt", current_logs
    if host == "redirect":
        assert "good.tech-fixture.example" not in current_logs, current_logs
    assert api("/projects/" + other["id"] + "/technology", "POST", {"url": f"http://{host}.tech-fixture.example/"})["cached"]
race_project = project("Concurrent technology lookup", "cooldown.tech-fixture.example")
before_logs = logs()
with concurrent.futures.ThreadPoolExecutor(2) as pool:
    raced = list(pool.map(lambda _: api("/projects/" + race_project["id"] + "/technology", "POST", {"url": "http://cooldown.tech-fixture.example/"}, (200, 409)), range(2)))
successful = [value for value in raced if "record" in value]
assert sum(not value["cached"] for value in successful) == 1, raced
assert logs()[len(before_logs):].count("cooldown.tech-fixture.example /") == 2
print("PASS: guarded homepage fingerprints, robots/bot/redirect stops, shared positive/negative cache, concurrent cooldown, URL bounds and auth", flush=True)

own = project("Our link sample", "ours.com")
one = project("Competitor one", "rival-one.com")
two = project("Competitor two", "rival-two.com")


def imported(p, sources, dataset="sample"):
    return api("/projects/" + p["id"] + "/backlink-snapshots", "POST", {"expected_domain": p["domain"], "dataset": dataset, "complete_sample": True,
        "links": [{"source_url": "https://" + s + "/article", "target_url": "https://" + p["domain"] + "/page"} for s in sources]}, 201)


imported(own, ["existing-publisher.org"])
imported(one, ["shared-publisher.org", "existing-publisher.org", "unique-one.net"])
imported(two, ["shared-publisher.org", "unique-two.net"])
imported(two, ["old-sample.net"], "history")
imported(two, [], "history")
intersect = "/projects/" + own["id"] + "/link-intersect"
body = {"competitor_project_ids": [one["id"], two["id"]], "min_competitors": 2}
prospects = api(intersect, "POST", body)
assert prospects["total_matching"] == 1 and prospects["items"][0]["source_domain"] == "shared-publisher.org", prospects
assert prospects["items"][0]["competitor_count"] == 2 and prospects["items"][0]["imported_links"] == 2 and prospects["items"][0]["verified_links"] == 0
assert api(intersect, "POST", dict(body, min_competitors=1))["total_matching"] == 3
assert api(intersect, "POST", dict(body, offset=1))["items"] == []
assert api(intersect, "POST", body, client=member) == prospects
api(intersect, "POST", body, 404, outsider)
api(intersect, "POST", body, 401, session())
foreign = project("Foreign tenant", "foreign.com", outsider)
api(intersect, "POST", {"competitor_project_ids": [foreign["id"]]}, 404)
same = project("Same website alias", "www.ours.com")
for invalid_body in [{"competitor_project_ids": []}, {"competitor_project_ids": [own["id"]]}, {"competitor_project_ids": [one["id"], one["id"]]}, {"competitor_project_ids": [same["id"]]}, dict(body, min_competitors=3), dict(body, limit=101), dict(body, offset=-1)]:
    api(intersect, "POST", invalid_body, 422)
api("/projects/" + two["id"], "PATCH", {"domain": "changed.com"})
assert api(intersect, "POST", body)["total_matching"] == 0
api("/projects/" + two["id"], "PATCH", {"domain": "rival-two.com"})
cap = project("Dataset cap", "rival-cap.com")
imported(cap, ["omitted-oldest.org"], "dataset-00")
for number in range(1, 11):
    imported(cap, [], f"dataset-{number:02d}")
bounded = api(intersect, "POST", {"competitor_project_ids": [cap["id"]]})
scope = next(s for s in bounded["scopes"] if s["project_id"] == cap["id"])
assert (scope["available_datasets"], scope["sampled_datasets"]) == (11, 10) and bounded["total_matching"] == 0
assert any("omitted" in w for w in bounded["warnings"])
api("/projects/" + own["id"] + "/archive", "POST")
assert api(intersect, "POST", body) == prospects
assert jobs_before == sql("SELECT count(*) FROM jobs;")
key = api("/api-keys", "POST", {"name": "Tech prospects regression"}, 201)


def mcp(name, arguments):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"], "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.loads(response.read())
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp("detect_technologies", {"project_id": p["id"], "url": url}) == {"ok": True, "data": cached}
assert mcp("find_link_intersect", dict(body, project_id=own["id"])) == {"ok": True, "data": prospects}
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: sampled link intersect, baseline exclusion, distinct competitors, latest empty snapshots, current website scoping, ten-dataset cap, auth/member/archive controls, pagination, MCP parity and no jobs", flush=True)
