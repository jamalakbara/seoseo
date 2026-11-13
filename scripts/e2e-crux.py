#!/usr/bin/env python3
"""CrUX disposable API/DB/HTTP contract regression. API_ORIGIN M6CHECK_DB FIXTURE."""
import concurrent.futures
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin, database, fixture = sys.argv[1:4]
assert database.startswith("seoseo-m6check") and fixture.startswith("seoseo-m6check")
owner = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
outsider = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def api(route, body=None, status=200, client=owner, method="POST"):
    req = urllib.request.Request(origin + "/api/v1" + route, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1", "Accept": "application/json, text/event-stream"})
    try:
        res = client.open(req, timeout=30)
    except urllib.error.HTTPError as e:
        res = e
    raw = res.read()
    assert res.status in ((status,) if isinstance(status, int) else status), (res.status, raw[:500])
    assert b"fixture-crux-key" not in raw and b"do-not-leak-provider-body" not in raw
    return json.loads(raw) if raw else None


def sql(statement):
    p = subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], input=statement, text=True, capture_output=True, check=True)
    return p.stdout.strip()


def logs():
    return subprocess.run(["docker", "logs", fixture], capture_output=True, text=True, check=True).stdout.splitlines()


for client in [owner, outsider]:
    api("/auth/sign-up", {"email": f"crux-{uuid.uuid4()}@example.com", "name": "CrUX test", "password": "disposable-crux-password"}, status=201, client=client)
p = api("/projects", {"name": "CrUX field data", "domain": "example.com"}, status=201)
path = "/projects/" + p["id"] + "/crux"
request = {"url": "https://example.com/" + str(uuid.uuid4()), "scope": "url", "form_factor": "PHONE"}
before = logs()
api(path, request, 404, outsider)
assert logs() == before
for change in [{"scope": "bad"}, {"form_factor": "mobile"}, {"url": "https://evil.example/"}, {"url": "https://example.com/?private=1"}, {"url": "https://example.com:8080/"}, {"scope": "origin"}]:
    api(path, request | change, 422)
assert logs() == before
r = api(path, request)
assert not r["cached"] and r["record"]["scope"] == "url" and r["record"]["status"] == "available", r
assert [m["p75"] for m in r["record"]["metrics"]] == [2400, None, 0.12], r
assert r["record"]["first_date"] == "2026-10-01" and r["record"]["last_date"] == "2026-10-28"
assert json.loads(logs()[-1]) == {"url": request["url"], "formFactor": "PHONE", "metrics": ["largest_contentful_paint", "interaction_to_next_paint", "cumulative_layout_shift"]}
after = logs()
assert api(path, request)["cached"] and logs() == after
api(path, request | {"form_factor": "DESKTOP"}, 409)
# Fixture-only cooldown reset to exercise independent cases without waiting minutes.
def reset():
    sql("DELETE FROM provider_cache WHERE kind IN ('crux_project_claim_v1','crux_target_claim_v1')")
reset()
root = request | {"url": "https://example.com/", "scope": "origin", "form_factor": "ALL"}
r = api(path, root)
assert r["record"]["target"] == "https://example.com" and r["record"]["form_factor"] == "ALL"
assert "formFactor" not in json.loads(logs()[-1]) and "origin" in json.loads(logs()[-1])
reset()
missing = request | {"url": "https://example.com/no-data-" + str(uuid.uuid4())}
r = api(path, missing)
assert r["record"]["status"] == "no_data" and all(m["p75"] is None for m in r["record"]["metrics"])
after = logs()
assert api(path, missing)["cached"] and logs() == after
for kind in ["quota", "invalid"]:
    reset()
    api(path, request | {"url": f"https://example.com/{kind}-{uuid.uuid4()}"}, 503)
reset()
race = request | {"url": "https://example.com/race-" + str(uuid.uuid4())}
before = len(logs())
with concurrent.futures.ThreadPoolExecutor(2) as pool:
    results = list(pool.map(lambda _: api(path, race, (200, 409)), range(2)))
assert sum("record" in x and not x["cached"] for x in results) == 1 and len(logs()) == before + 1
print("PASS: project isolation, exact scope/device, null missing metric, numeric CLS, period, cache/no-data cache, durable cooldown, concurrent claims, sanitized errors")
