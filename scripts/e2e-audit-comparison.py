#!/usr/bin/env python3
"""Disposable SA-13 regression. API_ORIGIN M6CHECK_DB WORKER1 WORKER2.
Workers must be created but stopped. Tests start both; all fixture writes stay in m6check.
API needs private crawler access, open signup and a configured renderer URL; workers
need private crawler access and no renderer. Network fixture: audit-fixture.example.
"""
import concurrent.futures
import http.cookiejar
import json
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

origin, database, *workers = sys.argv[1:]
assert database.startswith("seoseo-m6check") and len(workers) == 2
assert all(w.startswith("seoseo-m6check") for w in workers)


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, status=200, client=owner, return_status=False):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1", "Accept": "application/json, text/event-stream"})
    try:
        response = client.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status in ((status,) if isinstance(status, int) else status), (path, response.status, raw[:500])
    data = json.loads(raw) if raw else None
    return (response.status, data) if return_status else data


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"],
        input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"compare-{uuid.uuid4().hex}@example.com", "password": "disposable-audit-comparison-password", "name": "SA-13 regression"}, 201, client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "Comparison regression", "domain": "example.com"}, 201)
root = "/projects/" + p["id"]
config = {"max_pages": 3, "concurrency": 1, "delay_ms": 0, "respect_robots": True, "render_js": False, "lighthouse": False, "exclude_paths": []}


def audit(pid, start="https://example.com/", state="completed", cfg=None, score=70):
    aid = str(uuid.uuid4())
    payload = json.dumps(config if cfg is None else cfg).replace("'", "''")
    sql(f"INSERT INTO audits(id,project_id,start_url,status,phase,config,health_score,finished_at) VALUES ('{aid}','{pid}','{start}','{state}','done','{payload}',{score},now());")
    return aid


def page(aid, path, title="Stable title", issue=None, ok=True):
    nid = str(uuid.uuid4())
    title = title.replace("'", "''")
    sql(f"INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) VALUES ('{nid}','{aid}','https://example.com/{path}','{path}',{200 if ok else 503},'{"html" if ok else "error"}',1,'{title}');")
    if issue:
        sql(f"INSERT INTO audit_issues(id,audit_id,page_id,page_url,issue_type,severity) VALUES ('{uuid.uuid4()}','{aid}','{nid}','https://example.com/{path}','{issue}','warning');")
    return nid


before, after = audit(p["id"]), audit(p["id"], score=80)
page(before, "common", "X" * 400 + "a", "thin-content")
page(after, "common", "X" * 400 + "b", "missing-meta-description")
page(before, "missing", issue="missing-title")
page(before, "failed", issue="thin-content")
page(after, "failed", "", ok=False)
page(before, "same")
page(after, "same")
page(after, "new", issue="mixed-content")
sql(f"UPDATE audits SET stop_reason='page_limit',pages_failed=1 WHERE id='{after}';")
path = root + f"/audits/{after}/comparison?baseline_id={before}"
jobs_before = sql("SELECT count(*) FROM jobs;")
result = api(path)
assert result["counts"] == {"changed": 2, "unchanged": 1, "only_in_baseline": 1, "only_in_current": 1}, result
assert result["health_score_change"] == 10 and result["total_matching"] == 4
changes = {c["url"].rsplit("/", 1)[-1]: c for c in result["pages"]}
assert changes["common"]["changed_fields"] == ["title"]
assert changes["common"]["no_longer_observed_issues"] == ["thin-content"]
assert changes["common"]["newly_observed_issues"] == ["missing-meta-description"]
for name in ["missing", "failed"]:
    assert changes[name]["no_longer_observed_issues"] == [] and changes[name]["unconfirmed_absent_issues"]
assert changes["failed"]["changed_fields"] == ["fetch_class", "status_code"]
assert api(path + "&limit=1&offset=1")["pages"] == result["pages"][1:2]
assert api(path + "&kind=all")["total_matching"] == 5
assert api(path, client=member) == result
api(path, client=outsider, status=404)
api(path, client=session(), status=401)
for suffix in ["&kind=unknown", "&limit=0", "&limit=501"]:
    api(path + suffix, status=422)
api(root + f"/audits/{after}/comparison?baseline_id={after}", status=422)
foreign = api("/projects", "POST", {"name": "Other project"}, 201)
foreign_audit = audit(foreign["id"])
api(root + f"/audits/{after}/comparison?baseline_id={foreign_audit}", status=404)
wrong_scope = audit(p["id"], "https://other.example/")
api(root + f"/audits/{after}/comparison?baseline_id={wrong_scope}", status=422)
unfinished = audit(p["id"], state="failed")
api(root + f"/audits/{after}/comparison?baseline_id={unfinished}", status=409)
different_method = audit(p["id"], cfg=dict(config, render_js=True))
page(different_method, "common")
method = api(root + f"/audits/{different_method}/comparison?baseline_id={before}")
assert next(c for c in method["pages"] if c["url"].endswith("/common"))["unconfirmed_absent_issues"] == ["thin-content"]
assert jobs_before == sql("SELECT count(*) FROM jobs;")
print("PASS: comparison, full-value fingerprints, unknown absences, failed fetches, config changes, pagination, scope/status/auth/member isolation and no jobs", flush=True)

# Schedule writes copy a completed, tenant-owned baseline and enforce revision CAS.
schedule_root = root + "/audit-schedule"
assert api(schedule_root) is None
body = {"expected_revision": 0, "expected_domain": "example.com", "baseline_audit_id": before, "schedule": "weekly", "hour_utc": 6, "weekday": 1}
api(schedule_root, "PUT", body, 403, member)
api(schedule_root, "PUT", body, 404, outsider)
api(schedule_root, "PUT", dict(body, baseline_audit_id=foreign_audit), 404)
api(schedule_root, "PUT", dict(body, baseline_audit_id=unfinished), 409)
api(schedule_root, "PUT", dict(body, expected_domain="stale.example"), 409)
for field, value in [("schedule", "hourly"), ("hour_utc", 24), ("weekday", 0)]:
    api(schedule_root, "PUT", dict(body, **{field: value}), 422)
with concurrent.futures.ThreadPoolExecutor(2) as pool:
    writes = list(pool.map(lambda _: api(schedule_root, "PUT", body, (200, 409), return_status=True), range(2)))
assert sorted(w[0] for w in writes) == [200, 409], writes
saved = api(schedule_root)
assert saved["revision"] == 1 and saved["config"] == config and saved["next_run_at"]
assert api(schedule_root, client=member) == saved
disabled = api(schedule_root, "PUT", dict(body, expected_revision=1, schedule="manual", baseline_audit_id=None))
assert disabled["next_run_at"] is None and disabled["schedule"] == "manual"

cases = {}
for name in ["normal", "active", "archived", "changed", "quota", "renderer"]:
    project = api("/projects", "POST", {"name": "Scheduled " + name, "domain": "audit-fixture.example"}, 201)
    pid = project["id"]
    baseline = audit(pid, "http://audit-fixture.example/", cfg=dict(config, render_js=name == "renderer"))
    api(f"/projects/{pid}/audit-schedule", "PUT", dict(body, expected_domain="audit-fixture.example", baseline_audit_id=baseline, schedule="daily"))
    if name == "active":
        audit(pid, "http://audit-fixture.example/", state="queued")
    elif name == "archived":
        api(f"/projects/{pid}/archive", "POST")
    elif name == "changed":
        api(f"/projects/{pid}", "PATCH", {"domain": "changed.example"})
    elif name == "quota":
        sql(f"INSERT INTO audits(id,project_id,start_url,status) SELECT gen_random_uuid(),'{pid}','http://audit-fixture.example/','completed' FROM generate_series(1,99);")
    cases[name] = pid
guard_project = api("/projects", "POST", {"name": "Queued website guard", "domain": "audit-fixture.example"}, 201)
guard_id = audit(guard_project["id"], "http://audit-fixture.example/", state="queued")
guard_job = str(uuid.uuid4())
payload = json.dumps({"audit_id": guard_id, "scheduled": True, "expected_domain": "audit-fixture.example"})
sql(f"INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts) VALUES ('{guard_job}','site_audit','{me['organization_id']}','{guard_project['id']}','{payload}',3); UPDATE audits SET job_id='{guard_job}' WHERE id='{guard_id}';")
api("/projects/" + guard_project["id"], "PATCH", {"domain": "changed.example"})
due_ids = ",".join("'" + pid + "'" for pid in cases.values())
sql(f"UPDATE audit_schedules SET next_run_at=now()-interval '1 minute' WHERE project_id IN ({due_ids}) AND schedule<>'manual';")
subprocess.run(["docker", "start", *workers], check=True, capture_output=True)
deadline = time.monotonic() + 100
while time.monotonic() < deadline:
    states = {name: api(f"/projects/{pid}/audit-schedule") for name, pid in cases.items()}
    if states["normal"]["last_audit_id"] and all(states[n]["schedule"] == "manual" for n in ["archived", "changed", "quota", "renderer"]) and states["active"]["last_error"]:
        break
    time.sleep(1)
else:
    raise AssertionError(states)
normal_id = states["normal"]["last_audit_id"]
assert sql(f"SELECT count(*) FROM jobs WHERE project_id='{cases['normal']}' AND kind='site_audit';") == "1"
assert states["normal"]["revision"] == 2
for name in ["active", "archived", "changed", "quota", "renderer"]:
    assert states[name]["last_audit_id"] is None, states[name]
assert states["active"]["schedule"] == "daily" and "active" in states["active"]["last_error"]
assert sql(f"SELECT count(*) FROM audits WHERE project_id='{cases['quota']}';") == "100"
# The successful scheduled job runs the real crawler against the local badseo fixture.
while time.monotonic() < deadline:
    run = api(f"/projects/{cases['normal']}/audits/{normal_id}")
    if run["status"] == "completed":
        break
    assert run["status"] != "failed", run
    time.sleep(1)
else:
    raise AssertionError(run)
assert run["config"] == config and 0 < run["pages_crawled"] <= 3
while time.monotonic() < deadline:
    guard_run = api(f"/projects/{guard_project['id']}/audits/{guard_id}")
    if guard_run["status"] == "failed":
        break
    time.sleep(1)
assert guard_run["status"] == "failed" and guard_run["pages_crawled"] == 0 and "website changed" in guard_run["error"], guard_run
key = api("/api-keys", "POST", {"name": "SA13 regression"}, 201)


def mcp(name, arguments):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"], "Content-Type": "application/json", "Accept": "application/json, text/event-stream"},
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.loads(response.read())
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp("compare_site_audits", {"project_id": p["id"], "audit_id": after, "baseline_id": before}) == {"ok": True, "data": result}
assert mcp("get_audit_schedule", {"project_id": p["id"]}) == {"ok": True, "data": disabled}
changed = mcp("set_audit_schedule", dict(body, project_id=p["id"], expected_revision=disabled["revision"], schedule="manual"))
assert changed["ok"] and changed["data"]["revision"] == disabled["revision"] + 1
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: schedule CAS/admin scope, frozen budgets, two-worker atomic due claim, active skip, archive/website/quota/renderer pause, real local crawl and all three MCP tools", flush=True)
