#!/usr/bin/env python3
"""Offline manual AI visibility regression. Disposable password-auth stack only.
python scripts/e2e-ai-visibility.py http://127.0.0.1:3401 seoseo-m5ai-db seoseo-m5ai-worker http://127.0.0.1:3402
API/worker: AI_VISIBILITY_MODELS=fixture-a,fixture-b,fixture-c and gateway pointing at ai-model-fixture.py.
"""
import http.cookiejar
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

origin, database, worker, fixture = sys.argv[1:5]


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, expected=200, client=owner):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1"})
    try:
        response = client.open(request, timeout=45)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == expected, (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr


def control(reset=False):
    request = urllib.request.Request(fixture + ("/control" if reset else "/"), method="POST" if reset else "GET",
        data=b"{}" if reset else None, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request) as response:
        return json.load(response)


def wait(tid, rid):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        data = api(root + "/" + tid + "/runs/" + rid)
        if data["run"]["status"] in ("completed", "failed"):
            return data
        time.sleep(.5)
    raise AssertionError("run timed out")


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"ai-regression-{uuid.uuid4().hex}@example.com", "name": "AI regression", "password": "disposable-ai-regression-password"}, 201, client=client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
project = api("/projects", "POST", {"name": "AI answer comparisons", "domain": "example.com"}, 201)
pid = project["id"]
root = "/projects/" + pid + "/ai-trackers"
models = api("/ai-visibility/models")
assert models["ready"] and models["models"] == ["fixture-a", "fixture-b", "fixture-c"], models


def payload(name="Comparison", prompts=None, models=None):
    return {"expected_domain": "example.com", "name": name, "brands": ["Acme", "Rival"],
        "prompts": prompts or ["Which tools are useful?", "Which products are suitable?"],
        "models": models or ["fixture-a", "fixture-b", "fixture-c"], "country": "ID", "language": "en"}


for bad in [dict(payload(), brands=["Acme", "acme"]), dict(payload(), prompts=[]), dict(payload(), models=["not-approved"]), dict(payload(), brands=["x" * 81]), dict(payload(), country="USA")]:
    api(root, "POST", bad, 422)
api(root, "POST", payload(), 403, client=member)
api(root, "POST", payload(), 404, client=outsider)
api(root, expected=401, client=session())
control(True)
t = api(root, "POST", payload(), 201)
assert control()["calls"] == []  # creation makes no provider requests
tid = t["id"]
subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
try:
    run = api(root + "/" + tid + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    rid = run["id"]
    pending = api(root + "/" + tid + "/runs/" + rid)
    assert pending["successful_answers"] == 0 and all(b["mention_rate_pct"] is None for b in pending["brand_rates"])
    api(root + "/" + tid + "/runs", "POST", {"expected_domain": "example.com"}, 409)
    api(root + "/" + tid, "DELETE", expected=409)
    api(root + "/" + tid + "/runs/" + rid, "DELETE", expected=409)
    # Snapshot inputs remain frozen even if configuration is changed directly in the test DB.
    sql(f"UPDATE ai_visibility_trackers SET config=jsonb_set(config,'{{prompts}}','[\"Changed after queueing\"]'::jsonb) WHERE id='{tid}';")
finally:
    subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)
result = wait(tid, rid)
assert result["run"]["status"] == "completed" and result["successful_answers"] == 6, result
assert result["source"] == "model_api" and result["confidence"] == "relative"
assert len(control()["calls"]) == 6 and all(c["prompt"] != "Changed after queueing" for c in control()["calls"])
assert all("country ID" in c["system"] for c in control()["calls"])
rates = {b["brand"]: b["mention_rate_pct"] for b in result["brand_rates"]}
assert abs(rates["Acme"] - 100 / 3) < .01 and abs(rates["Rival"] - 200 / 3) < .01, rates
assert result["answers"][0]["sources"][0]["url"] == "https://sources.example/guide"
api(root + "/" + tid + "/runs", "POST", {"expected_domain": "example.com"}, 409)  # cooldown
assert api(root + "/" + tid + "/runs/" + rid, client=member)["successful_answers"] == 6
api(root + "/" + tid + "/runs/" + rid, expected=404, client=outsider)
api(root + "/" + tid + "/runs", "POST", {"expected_domain": "example.com"}, 403, client=member)

# One success, one failure, and one unattempted cell: failures never enter the denominator.
partial = api(root, "POST", payload("Partial", ["Good", "FAIL after partial", "Never attempted"], ["fixture-a"]), 201)
control(True)
pr = api(root + "/" + partial["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202)
pv = wait(partial["id"], pr["id"])
assert (pv["run"]["status"], pv["successful_answers"], pv["failed_answers"], pv["expected_answers"]) == ("failed", 1, 1, 3), pv
assert len(control()["calls"]) == 2 and pv["brand_rates"][0]["mention_rate_pct"] == 100
for word in ["FAIL", "TRUNCATE"]:
    bad = api(root, "POST", payload(word, [word], ["fixture-a"]), 201)
    br = api(root + "/" + bad["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    bv = wait(bad["id"], br["id"])
    assert bv["successful_answers"] == 0 and bv["failed_answers"] == 1 and bv["brand_rates"][0]["mention_rate_pct"] is None, bv

# Resume skips a persisted successful cell and uses the run's frozen config.
resume = api(root, "POST", payload("Resume", ["Already saved", "Remaining prompt"], ["fixture-a"]), 201)
control(True)
subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
try:
    rr = api(root + "/" + resume["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    sql(f"INSERT INTO ai_visibility_answers(run_id,prompt_index,model,answer,mentions,sources) VALUES('{rr['id']}',0,'fixture-a','Acme','[\"Acme\"]','[]');")
finally:
    subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)
rv = wait(resume["id"], rr["id"])
assert rv["successful_answers"] == 2 and [c["prompt"] for c in control()["calls"]] == ["Remaining prompt"], rv

# Terminal queue failures reconcile the run and release active-run quota.
stuck = api(root, "POST", payload("Queue failure", ["Good"], ["fixture-a"]), 201)
subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
try:
    sr = api(root + "/" + stuck["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    sql(f"UPDATE jobs SET status='failed' WHERE id='{sr['job_id']}';")
    assert api(root + "/" + stuck["id"] + "/runs/" + sr["id"])["run"]["status"] == "failed"
finally:
    subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)

# MCP reads/CRUD call the same services.
key = api("/api-keys", "POST", {"name": "AI regression"}, 201)


def tool(name, args):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"], "Content-Type": "application/json", "Accept": "application/json, text/event-stream"},
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}}).encode())
    with urllib.request.urlopen(request, timeout=45) as response:
        data = json.load(response)
    return json.loads(data["result"]["content"][0]["text"])


assert tool("get_ai_visibility_run", {"project_id": pid, "tracker_id": tid, "run_id": rid})["data"] == result
assert tool("list_ai_visibility_models", {})["data"] == models

# The worker rejects a changed gateway identity before making a provider request.
changed = api(root, "POST", payload("Gateway changed", ["Good"], ["fixture-a"]), 201)
control(True)
subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
try:
    cr = api(root + "/" + changed["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    sql(f"UPDATE ai_visibility_runs SET gateway_id='different-gateway' WHERE id='{cr['id']}';")
finally:
    subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)
cv = wait(changed["id"], cr["id"])
assert cv["run"]["status"] == "failed" and cv["successful_answers"] == 0 and control()["calls"] == []

other = api("/projects", "POST", {"name": "Other", "domain": "example.com"}, 201)
api("/projects/" + other["id"] + "/ai-trackers/" + tid + "/runs/" + rid, expected=404)
api("/projects/" + other["id"] + "/ai-trackers/" + tid, "DELETE", expected=404)

# Project concurrency caps and retained-run quotas are enforced before queue writes.
other_root = "/projects/" + other["id"] + "/ai-trackers"
limits = [api(other_root, "POST", payload(f"Limit {i}", ["Good"], ["fixture-a"]), 201) for i in range(3)]
subprocess.run(["docker", "pause", worker], check=True, capture_output=True)
try:
    queued = [api(other_root + "/" + t["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 202) for t in limits[:2]]
    api(other_root + "/" + limits[2]["id"] + "/runs", "POST", {"expected_domain": "example.com"}, 409)
    for qr in queued:
        sql(f"UPDATE jobs SET status='failed' WHERE id='{qr['job_id']}';")
    api(other_root)  # reconcile terminal jobs before checking retention
    qt = limits[2]["id"]
    sql(f"INSERT INTO ai_visibility_runs(id,tracker_id,config,gateway_id,status,created_at) SELECT gen_random_uuid(),id,config,'retention-fixture','completed',clock_timestamp()-interval '2 minutes' FROM ai_visibility_trackers CROSS JOIN generate_series(1,20) WHERE id='{qt}';")
    api(other_root + "/" + qt + "/runs", "POST", {"expected_domain": "example.com"}, 409)
    old = api(other_root + "/" + qt)["runs"][0]
    api(other_root + "/" + qt + "/runs/" + old["id"], "DELETE", expected=204)
    recovered = api(other_root + "/" + qt + "/runs", "POST", {"expected_domain": "example.com"}, 202)
    sql(f"UPDATE jobs SET status='failed' WHERE id='{recovered['job_id']}';")
finally:
    subprocess.run(["docker", "unpause", worker], check=True, capture_output=True)
api(root + "/" + partial["id"] + "/runs/" + pr["id"], "DELETE", expected=204)
api(root + "/" + partial["id"] + "/runs/" + pr["id"], expected=404)
api("/projects/" + pid, "PATCH", {"domain": "changed.org"})
assert len(api(root)) > 0
api(root + "/" + tid, expected=404)
api(root + "/" + tid + "/runs", "POST", {"expected_domain": "example.com"}, 404)
api(root + "/" + tid, "DELETE", expected=204)  # prior-site cleanup
api("/projects/" + pid + "/archive", "POST")
api(root, "POST", dict(payload(), expected_domain="changed.org"), 409)
api("/api-keys/" + key["key"]["id"], "DELETE", expected=204)
print("PASS: AI model comparisons, frozen config, literal rates/unknowns, partial failures/truncation, resume, terminal-job reconciliation, roles/tenant/project/domain isolation, deletion and MCP parity")
