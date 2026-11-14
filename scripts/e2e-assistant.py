#!/usr/bin/env python3
"""M7 assistant/content regression. Use only the disposable compose.m7check stack."""
import concurrent.futures
import http.cookiejar
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:3501"
database = sys.argv[2] if len(sys.argv) > 2 else "seoseo-m7check-db-1"
assert database.startswith("seoseo-m7check"), "Disposable M7 database required"


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, status=200, client=owner):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1"})
    try:
        response = client.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == status, (path, response.status, raw[:1000])
    try:
        return json.loads(raw) if raw else None
    except json.JSONDecodeError:
        assert status >= 400, raw
        return raw.decode()


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo", "-At"],
        input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def until(read, predicate, seconds=80):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = read()
        if predicate(value):
            return value
        time.sleep(.4)
    raise AssertionError(("timeout", value))


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"assistant-{uuid.uuid4().hex}@example.com", "password": "disposable-m7-password", "name": "M7 regression"}, 201, client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": f"M7 assistant {uuid.uuid4().hex}", "domain": "ours.example"}, 201)
root = "/projects/" + p["id"]
base = root + "/assistant/chats"
api(root + "/context", "PUT", {"revision": 0, "sections": [{"key": "business_overview", "title": "Business", "content": "OWNER_MEMORY_MARKER: SEO audit services"}]})
assert api(root + "/assistant/models")["models"][0] == "fixture"
assert api(base) == []
api(base, client=outsider, status=404)
api(base, client=session(), status=401)
api(base, "POST", {"title": " "}, 422)
c = api(base, "POST", {"title": "Tool evidence test"}, 201)
path = base + "/" + c["id"]
api(path, client=member, status=404)
api(path, client=outsider, status=404)
api(path + "/messages", "POST", {"model": "unapproved", "message": "hello"}, 422)
api(path + "/messages", "POST", {"model": "fixture", "message": "x" * 4001}, 422)
api(path + "/messages", "POST", {"model": "fixture", "message": "hello", "project_id": str(uuid.uuid4())}, 422)


def send(message, model="fixture", chat_path=path):
    return api(chat_path + "/messages", "POST", {"model": model, "message": message}, 202)


def finish(chat_path=path):
    return until(lambda: api(chat_path), lambda v: v["turns"][-1]["status"] in ("completed", "failed"))["turns"][-1]


queued = send("SLOW What should I fix first?")
api(path + "/messages", "POST", {"model": "fixture", "message": "concurrent"}, 409)
t = finish()
assert t["status"] == "completed" and "Fixture answer" in t["answer"], t
assert [e["tool"] for e in t["evidence"]] == ["project_memory", "audit_summary"]
assert t["evidence"][0]["data"]["sections"][0]["content"].startswith("OWNER_MEMORY_MARKER")
assert "gateway_id" not in t
send("Follow up about this project")
assert finish()["status"] == "completed"
for message in ("RATE_LIMIT", "MALFORMED", "FORGE_SCOPE", "LOOP"):
    send(message)
    turn = finish()
    assert turn["status"] == "failed" and turn["answer"] is None, turn
    if message == "LOOP":
        assert len(turn["evidence"]) == 5, turn
    assert api("/jobs/" + turn["job_id"])["max_attempts"] == 1
send("UNKNOWN_TOOL")
turn = finish()
assert turn["status"] == "completed" and "error" in turn["evidence"][1]["data"]
assert api(root + "/context")["revision"] == 1, "Assistant must not edit project memory"
missing = f"missing-{uuid.uuid4().hex}"
score = api(root + "/content/score", "POST", {"keyword": missing, "content": "SEO audits help."})
assert score["coverage_score"] is None and score["evidence"]["references"] == []
api(root + "/content/brief", "POST", {"keyword": missing, "model": "fixture"}, 409)
api(root + "/content/score", "POST", {"keyword": "seo audits", "content": "draft"}, 404, outsider)
api(root + "/content/score", "POST", {"keyword": "seo audits", "content": " "}, 422)
sid = str(uuid.uuid4())
sql(f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language) VALUES('{sid}','offline_fixture',false,'seo audits','US','en'); INSERT INTO serp_items(snapshot_id,position,url,domain,title,snippet) VALUES('{sid}',1,'https://ref.example/1','ref.example','SEO audits checklist','Technical crawl advice'),('{sid}',2,'https://ref.example/2','ref.example','SEO audits checklist','Technical website advice');")
score = api(root + "/content/score", "POST", {"keyword": "SEO audits", "content": "SEO AUDITS checklist technical advice"})
assert score["coverage_score"] == 100 and score["evidence"]["snapshot_id"] == sid and score["evidence"]["exact"] is False, score
assert api(root + "/content/score", "POST", {"keyword": "seo audits", "content": "unrelated"})["coverage_score"] == 0
brief = api(root + "/content/brief", "POST", {"keyword": "seo audits", "model": "fixture"}, 202)
bt = finish(base + "/" + brief["chat"]["id"])
assert bt["status"] == "completed" and bt["evidence"][1]["tool"] == "cached_serp", bt
# Member chat ownership remains independent, even in the same workspace.
mc = api(base, "POST", {"title": "Member private"}, 201, member)
api(base + "/" + mc["id"], status=404)
# A terminal queue failure is reconciled without replaying an ambiguous provider request.
crash = api(base, "POST", {"title": "Interrupted turn"}, 201)
crash_path = base + "/" + crash["id"]
interrupted = send("SLOW interruption test", chat_path=crash_path)
sql(f"UPDATE assistant_turns SET status='running',request_started=true WHERE id='{interrupted['id']}'; UPDATE jobs SET status='failed' WHERE id='{interrupted['job_id']}';")
assert api(crash_path)["turns"][-1]["status"] == "failed"
# Changing the market preserves old chat history but disallows continuation.
api(root, "PATCH", {"country_code": "ID"})
api(path + "/messages", "POST", {"model": "fixture", "message": "new market"}, 409)
assert api(root + "/content/score", "POST", {"keyword": "seo audits", "content": "draft"})["coverage_score"] is None
api(root, "PATCH", {"country_code": "US"})
# In-flight jobs still consume capacity after their chats have been deleted.
orphan_jobs = [str(uuid.uuid4()) for _ in range(4)]
for jid in orphan_jobs:
    sql(f"INSERT INTO jobs(id,kind,organization_id,project_id,payload,max_attempts,run_at) VALUES('{jid}','assistant_turn','{me['organization_id']}','{p['id']}','{{}}',1,now()+interval '1 day');")
api(path + "/messages", "POST", {"model": "fixture", "message": "must respect outstanding jobs"}, 409)
sql("UPDATE jobs SET status='failed' WHERE id IN (" + ",".join("'" + jid + "'" for jid in orphan_jobs) + ");")
# Two concurrent creations at the per-user limit must never exceed twenty.
remaining = 19 - len(api(base))
for n in range(remaining):
    api(base, "POST", {"title": f"Bound {n}"}, 201)


def create_race(_):
    try:
        api(base, "POST", {"title": "Race"}, 201)
        return True
    except AssertionError as error:
        assert error.args[0][1] == 409, error
        return False


with concurrent.futures.ThreadPoolExecutor(2) as executor:
    assert sum(executor.map(create_race, range(2))) == 1
assert len(api(base)) == 20
# API-key ownership agrees with session ownership.
key = api("/api-keys", "POST", {"name": "M7 regression"}, 201)


def mcp(name, arguments):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
        "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1,
        "method": "tools/call", "params": {"name": name, "arguments": arguments}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.load(response)
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp("score_page_content", {"project_id": p["id"], "keyword": "seo audits", "content": "SEO audits checklist technical advice"})["data"]["coverage_score"] == 100
assert mcp("get_assistant_chat", {"project_id": p["id"], "chat_id": c["id"]})["data"]["chat"]["id"] == c["id"]
assert not mcp("get_assistant_chat", {"project_id": p["id"], "chat_id": mc["id"]})["ok"]
request = urllib.request.Request(origin + "/api/v1" + path, headers={"Authorization": "Bearer " + key["secret"]})
with urllib.request.urlopen(request) as response:
    assert json.load(response)["chat"]["id"] == c["id"]
api(path, "DELETE", status=204)
api(path, status=404)
assert sql(f"SELECT count(*) FROM assistant_turns WHERE chat_id='{c['id']}'") == "0"
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: assistant tools/history, private ownership and tenant isolation, strict inputs, budget, no writes, provider failure/no retry, interrupted work, cached content briefs/coverage, market isolation, concurrent quotas and deletion")
