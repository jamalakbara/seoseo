#!/usr/bin/env python3
"""M7 completion acceptance. ONLY the disposable compose.m7check stack; no live recipients."""
import concurrent.futures
import http.cookiejar
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:3501"
database = sys.argv[2] if len(sys.argv) > 2 else "seoseo-m7check-db-1"
assert database.startswith("seoseo-m7check"), "Disposable M7 database required"
password = "disposable-m7-acceptance-password"


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member, doomed = [session() for _ in range(4)]


def api(path, method="GET", body=None, status=200, client=owner):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method, data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1"})
    try:
        response = client.open(request, timeout=55)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == status, (path, response.status, raw[:1000])
    if response.headers.get("Content-Type", "").startswith("application/pdf"):
        return raw
    try:
        return json.loads(raw) if raw else None
    except json.JSONDecodeError:
        assert status >= 400, raw
        return raw.decode()


def sql(statement, db="seoseo"):
    r = subprocess.run(["docker", "exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", db, "-At"], input=statement, capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    return r.stdout.strip()


def read(url):
    with urllib.request.urlopen(url, timeout=20) as r:
        return json.load(r)


def until(fn, accept, seconds=90):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = fn()
        if accept(value):
            return value
        time.sleep(.5)
    raise AssertionError(("timeout", value))


def restart_worker():
    subprocess.run(["docker", "restart", "seoseo-m7check-worker-1"], check=True, capture_output=True)


for client in (owner, outsider, member, doomed):
    api("/auth/sign-up", "POST", {"email": f"m7-{uuid.uuid4().hex}@example.com", "name": "M7 acceptance", "password": password}, 201, client)
me, mate, gone = api("/me"), api("/me", client=member), api("/me", client=doomed)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": f"M7 acceptance {uuid.uuid4().hex[:8]}", "domain": "ours.example"}, 201)
root = "/projects/" + p["id"]
assert api(root + "/notifications/settings") is None
for client, status in ((outsider, 404), (session(), 401)):
    api(root + "/notifications/events", status=status, client=client)
settings = {"revision": 0, "expected_domain": "ours.example", "rank_enabled": True, "page_enabled": True, "min_change": 5, "include_depth_changes": True, "key_urls": ["https://ours.example/page", "https://ours.example/missing"]}
api(root + "/notifications/settings", "PUT", settings, 403, member)
api(root + "/notifications/settings", "PUT", {**settings, "min_change": 0}, 422)
api(root + "/notifications/settings", "PUT", {**settings, "key_urls": ["https://other.example/"]}, 422)
assert api(root + "/notifications/settings", "PUT", settings)["revision"] == 1
api(root + "/notifications/settings", "PUT", settings, 409)
for kind, url in [("webhook", "ftp://receiver/hook"), ("webhook", "http://user:secret@receiver/hook"), ("slack", "https://hooks.slack.com.evil.example/services/a")]:
    api(root + "/notifications/endpoints", "POST", {"name": "Invalid", "kind": kind, "url": url}, 422)
ep = api(root + "/notifications/endpoints", "POST", {"name": "Local receiver", "kind": "webhook", "url": "http://receiver:8080/hook/" + p["id"]})
assert "secret_url" not in ep and "url" not in ep
assert sql(f"SELECT octet_length(secret_url)>20 FROM notification_endpoints WHERE id='{ep['id']}'") == "t"
t = api(root + "/rank-trackers", "POST", {"name": "Alert source", "keywords": ["gain", "enter", "outside", "method", "missing"]}, 201)
ids = json.loads(sql(f"SELECT json_object_agg(keyword,id) FROM rank_tracker_keywords WHERE tracker_id='{t['id']}'"))
before, after, ab, aa = [str(uuid.uuid4()) for _ in range(4)]
frozen = "ARRAY[" + ",".join("'" + v + "'::uuid" for v in ids.values()) + "]"
statements = ["BEGIN;", f"INSERT INTO rank_runs(id,tracker_id,status,trigger,keyword_ids,keywords_total,created_at,finished_at) VALUES('{before}','{t['id']}','completed','manual',{frozen},5,now()-interval '2 hours',now()-interval '1 hour'),('{after}','{t['id']}','completed','manual',{frozen},5,now(),clock_timestamp());"]
for keyword, pos0, pos1 in [("gain", 9, 2), ("enter", None, 5), ("outside", None, None), ("method", 9, 2), ("missing", 9, 1)]:
    for run, pos, provider in [(before, pos0, "other" if keyword == "method" else "fixture"), (after, pos1, "fixture")]:
        if run == before and keyword == "missing":
            continue
        statements.append(f"INSERT INTO rank_results(run_id,keyword_id,position,depth,provider,exact) VALUES('{run}','{ids[keyword]}',{pos if pos is not None else 'NULL'},10,'{provider}',false);")
statements.append(f"INSERT INTO audits(id,project_id,start_url,status,config,created_at,finished_at,pages_crawled,health_score) VALUES('{ab}','{p['id']}','https://ours.example/','completed','{{}}',now()-interval '2 hours',now()-interval '1 hour',2,90),('{aa}','{p['id']}','https://ours.example/','completed','{{}}',now(),clock_timestamp(),1,85);")
for audit, title in [(ab, "Before"), (aa, "After")]:
    statements.append(f"INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) VALUES('{uuid.uuid4()}','{audit}','https://ours.example/page','page',200,'html',0,'{title}');")
statements += [f"INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) VALUES('{uuid.uuid4()}','{ab}','https://ours.example/missing','missing',200,'html',0,'Missing later');", "COMMIT;"]
sql("\n".join(statements))
with concurrent.futures.ThreadPoolExecutor(2) as executor:
    list(executor.map(lambda _: api(root + "/notifications/evaluate", "POST"), range(2)))
events = api(root + "/notifications/events")
assert len(events) == 2, events
rank = next(e for e in events if e["kind"] == "rank_movement")
page = next(e for e in events if e["kind"] == "key_page_change")
assert rank["data"]["matched"] == 2 and rank["data"]["comparable"] == 3, rank
assert next(i for i in rank["data"]["items"] if i["kind"] == "entered_depth")["change"] is None
assert page["data"]["matched"] == 1 and page["data"]["unavailable_or_incomparable"] == 1
assert api(root + "/notifications/evaluate", "POST") == 0
api(root + "/notifications/events/" + rank["id"] + "/acknowledge", "POST", status=204, client=member)
calls = until(lambda: read("http://127.0.0.1:3504/calls"), lambda values: len([v for v in values if v.get("data", {}).get("project_id") == p["id"]]) >= 2)
assert all(v["delivery_id"] for v in calls if v.get("data", {}).get("project_id") == p["id"])
for path in ("/fail", "/redirect", "/drop"):
    bad = api(root + "/notifications/endpoints", "POST", {"name": "Failure fixture", "kind": "webhook", "url": "http://receiver:8080" + path})
    did = api(root + "/notifications/endpoints/" + bad["id"] + "/test", "POST", status=202)
    deliveries = until(lambda: api(root + "/notifications/deliveries"), lambda rows: any(d["id"] == did and d["status"] == "failed" for d in rows))
    d = next(d for d in deliveries if d["id"] == did)
    assert api("/jobs/" + d["job_id"])["attempts"] == 1
    api(root + "/notifications/endpoints/" + bad["id"], "DELETE", status=204)
assert not any(c["path"] == "/target" for c in read("http://127.0.0.1:3504/calls"))
# Queued destinations honor revision changes; ambiguous claimed attempts are not replayed.
subprocess.run(["docker", "stop", "seoseo-m7check-worker-1"], check=True, capture_output=True)
cancel_path = "/cancel/" + p["id"]
cancel_ep = api(root + "/notifications/endpoints", "POST", {"name": "Cancellation fixture", "kind": "webhook", "url": "http://receiver:8080" + cancel_path})
queued = [api(root + "/notifications/endpoints/" + cancel_ep["id"] + "/test", "POST", status=202) for _ in range(3)]
api(root + "/notifications/endpoints/" + cancel_ep["id"] + "/test", "POST", status=409)
sql(f"UPDATE notification_deliveries SET status='sending',attempted_at=now() WHERE id='{queued[2]}'; UPDATE jobs SET status='failed',attempts=1 WHERE id=(SELECT job_id FROM notification_deliveries WHERE id='{queued[2]}');")
api(root + "/notifications/endpoints/" + cancel_ep["id"], "PATCH", {"revision": cancel_ep["revision"], "enabled": False}, 204)
restart_worker()
rows = until(lambda: api(root + "/notifications/deliveries"), lambda rows: all(any(d["id"] == did and d["status"] == "cancelled" for d in rows) for did in queued[:2]))
assert next(d for d in rows if d["id"] == queued[2])["status"] == "failed"
assert not any(c["path"] == cancel_path for c in read("http://127.0.0.1:3504/calls"))
api(root + "/notifications/endpoints/" + cancel_ep["id"], "DELETE", status=204)
# Profile, source-labelled records and transient Places sample.
profile = {"revision": 0, "name": "M7 Business", "address": "Fixture Street", "phone": None, "website": "https://ours.example/", "hours": "Mon–Fri", "description": "User supplied profile", "source_label": "Operator record"}
assert api(root + "/business/profile", "PUT", profile)["revision"] == 1
api(root + "/business/profile", "PUT", profile, 409)
api(root + "/business/profile", "PUT", profile, 403, member)
for kind in ("review", "question", "update"):
    record = api(root + "/business/entries", "PUT", {"id": None, "revision": 0, "kind": kind, "text": "Synthetic record", "response": "Unpublished draft", "rating": 4 if kind == "review" else None, "source_label": "Synthetic fixture", "source_url": None})
    edited = {**record["data"], "id": record["id"], "revision": record["revision"], "text": "Edited synthetic record"}
    assert api(root + "/business/entries", "PUT", edited)["revision"] == 2
    api(root + "/business/entries", "PUT", edited, 409)
assert len(api(root + "/business/entries")) == 3
assert api(root + "/business/places", "POST", {"place_id": "fixture-place", "include_reviews": True})["data"]["displayName"]["text"] == "Fixture Business"
assert len(api(root + "/business/entries")) == 3
api(root + "/business/places", "POST", {"place_id": "../bad", "include_reviews": False}, 422)
# All built-ins save valid escaped HTML, then Chromium exports a network-isolated PDF.
reports = []
for kind in ("audit", "rank", "keywords", "links", "ai"):
    report = api(root + "/reports/generate", "POST", {"kind": kind, "title": "<M7 report>", "byline": "<script>alert(1)</script>", "accent": "#123456"})
    assert "&lt;M7 report&gt;" in report["html"] and "<script>alert" not in report["html"]
    reports.append(report)
api(root + "/reports/generate", "POST", {"kind": "invalid"}, 422)
api(root + "/reports/generate", "POST", {"kind": "audit", "accent": "red;bad"}, 422)
pdf = api(root + "/reports/" + reports[0]["id"] + "/pdf")
assert pdf.startswith(b"%PDF-") and len(pdf) > 1000
tmp = Path(tempfile.mkdtemp(prefix="seoseo-m7-acceptance-"))
(tmp / "report.pdf").write_bytes(pdf)
malicious = api(root + "/reports", "POST", {"title": "PDF isolation", "summary": None, "html": '<html><body><h1>Isolated PDF</h1><img src="http://receiver:8080/beacon"><script>fetch("http://receiver:8080/beacon-script")</script></body></html>', "skill": None, "template_id": None}, 201)
assert api(root + "/reports/" + malicious["id"] + "/pdf").startswith(b"%PDF-")
assert not any(c["path"].startswith("/beacon") for c in read("http://127.0.0.1:3504/calls"))
# MCP uses the same project scope and report domain service.
key = api("/api-keys", "POST", {"name": "M7 completion fixture"}, 201)


def mcp(name, arguments):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
        "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1,
        "method": "tools/call", "params": {"name": name, "arguments": arguments}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.load(response)
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp("get_alert_settings", {"project_id": p["id"]})["data"] == api(root + "/notifications/settings")
assert len(mcp("list_alerts", {"project_id": p["id"]})["data"]) == 2
assert mcp("generate_report", {"project_id": p["id"], "kind": "rank", "title": "MCP fixture"})["data"]["title"] == "MCP fixture"
assert not mcp("list_alerts", {"project_id": str(uuid.uuid4())})["ok"]
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
# Weekly schedule -> real SMTP receiver, once per claimed period, no arbitrary recipient.
digest = api(root + "/notifications/digest")
assert digest["smtp_ready"]
api(root + "/notifications/digest", "PUT", {"revision": 0, "enabled": True, "weekday": 1, "hour_utc": 6, "email": "other@example.com"}, 422)
digest = api(root + "/notifications/digest", "PUT", {"revision": 0, "enabled": True, "weekday": 1, "hour_utc": 6})
sql(f"UPDATE digest_subscriptions SET next_run_at=now()-interval '1 minute' WHERE project_id='{p['id']}' AND user_id='{me['user']['id']}';")
restart_worker()
mail = until(lambda: read("http://127.0.0.1:3505/api/v1/messages"), lambda data: any(p["name"] in m["Subject"] for m in data.get("messages", [])))
matching = [m for m in mail["messages"] if p["name"] in m["Subject"]]
assert len(matching) == 1
until(lambda: api(root + "/notifications/deliveries"), lambda rows: any(d["kind"] == "email" and d["status"] == "delivered" for d in rows))
restart_worker()
assert len([m for m in read("http://127.0.0.1:3505/api/v1/messages")["messages"] if p["name"] in m["Subject"]]) == 1
api(root + "/notifications/digest", "PUT", {"revision": 1, "enabled": False, "weekday": 1, "hour_utc": 6})
# Account request/cancel/grace purge, including grants in a surviving shared project.
delete_body = {"password": password, "confirmation": "DELETE MY ACCOUNT"}
api("/account/deletion", "POST", delete_body, 409)
api("/account/deletion", "POST", {**delete_body, "password": "wrong"}, 403, doomed)
pending = api("/account/deletion", "POST", delete_body, client=doomed)
assert pending["delete_after"] > pending["requested_at"]
api("/account/deletion", "DELETE", status=204, client=doomed)
assert api("/account/deletion", client=doomed) is None
doomed_project = api("/projects", "POST", {"name": "Delete fixture only", "domain": "delete.example"}, 201, doomed)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES('{me['organization_id']}','{gone['user']['id']}','admin'); INSERT INTO gsc_connections(project_id,connected_by,refresh_token_enc) VALUES('{p['id']}','{gone['user']['id']}','\\x01');")
api("/account/deletion", "POST", delete_body, client=doomed)
sql(f"UPDATE account_deletions SET delete_after=now()-interval '1 second' WHERE user_id='{gone['user']['id']}';")
restart_worker()
until(lambda: sql(f"SELECT count(*) FROM users WHERE id='{gone['user']['id']}'"), lambda n: n == "0")
api("/me", status=401, client=doomed)
assert sql(f"SELECT count(*) FROM projects WHERE id='{doomed_project['id']}'") == "0"
assert sql(f"SELECT count(*) FROM gsc_connections WHERE project_id='{p['id']}'") == "0"
assert api(root)["id"] == p["id"]
# Full database backup and empty-target restore, including encrypted destination bytes.
env = tmp / "fixture.env"
env.write_text("POSTGRES_PASSWORD=disposable-m7-password\nAPP_ENCRYPTION_KEY=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n", encoding="utf-8")
common = [sys.executable, "scripts/maintenance.py"]
subprocess.run(common + ["backup", "--container", database, "--directory", str(tmp / "backup"), "--env-file", str(env)], check=True)
restored_db = "m7_restore_" + uuid.uuid4().hex[:8]
sql("CREATE DATABASE " + restored_db)
subprocess.run(common + ["restore", "--container", database, "--db-name", restored_db, "--directory", str(tmp / "backup"), "--env-output", str(tmp / "restored.env"), "--confirm", "RESTORE EMPTY DATABASE"], check=True)
for table in ("projects", "reports", "notification_events", "notification_endpoints", "business_entries"):
    assert sql("SELECT count(*) FROM " + table) == sql("SELECT count(*) FROM " + table, restored_db)
assert (tmp / "restored.env").read_bytes() == env.read_bytes()
rejected = subprocess.run(common + ["restore", "--container", database, "--db-name", restored_db, "--directory", str(tmp / "backup"), "--env-output", str(tmp / "not-created.env"), "--confirm", "RESTORE EMPTY DATABASE"], capture_output=True)
assert rejected.returncode == 1 and not (tmp / "not-created.env").exists()
print("PASS: M7 alerts/comparability/deduplication, scoped delivery and no redirects/replay, business workspace/Places fixture, all five reports, isolated PDF, real weekly SMTP digest, account grace/cancel/purge, backup/empty restore and refusal to overwrite", tmp)
