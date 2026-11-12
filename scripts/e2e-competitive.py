#!/usr/bin/env python3
"""Offline competitive research regression. Use a disposable password-auth API/DB.

python scripts/e2e-competitive.py http://127.0.0.1:3401 seoseo-m5-db
The API needs ALLOW_SIGNUP=true; the DB container must allow local psql as seoseo.
Creates synthetic users/projects and SERP fixtures. Never point at production.
"""
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid

origin, database = sys.argv[1:3]


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider = session(), session()


def req(path, method="GET", body=None, status=200, client=owner):
    request = urllib.request.Request(origin + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1", "Accept": "application/json, text/event-stream"})
    try:
        response = client.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == status, (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def api(path, *args, **kwargs):
    return req("/api/v1" + path, *args, **kwargs)


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"],
        input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr


for client in (owner, outsider):
    api("/auth/sign-up", "POST", {"email": f"competitive-{uuid.uuid4().hex}@example.com",
        "password": "disposable-competitive-password", "name": "Competitive regression"}, 201, client=client)
p = api("/projects", "POST", {"name": "Competitive regression", "domain": "www.ours.com"}, 201)
root = "/projects/" + p["id"]
api(root + "/saved-keywords", "POST", {"keywords": ["shared", "gap", "empty", "stale"]})
other = api("/projects", "POST", {"name": "Other scope"}, 201)
api("/projects/" + other["id"] + "/saved-keywords", "POST", {"keywords": ["private-other-project"]})


def snapshot(keyword, items, age="0 days", exact=True, country="US"):
    sid = str(uuid.uuid4())
    # Fixture strings are constants, never user input.
    statement = f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES ('{sid}','fixture',{str(exact).lower()},'{keyword}','{country}','en',now()-interval '{age}');"
    for position, domain in items:
        statement += f"INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('{sid}',{position},'https://{domain}/page','{domain}','Fixture');"
    sql(statement)


snapshot("shared", [(1, "rival.com")], "2 days")
snapshot("shared", [(2, "rival.com"), (3, "ours.com"), (5, "rival.com"), (8, "third.com"), (31, "outside.com")], "1 day", False)
snapshot("gap", [(12, "rival.com"), (7, "third.com")])
snapshot("gap", [(1, "wrong-market.com")], country="GB")
snapshot("empty", [(1, "rival.com")], "1 day")
snapshot("empty", [])  # latest empty page replaces old evidence
snapshot("stale", [(1, "rival.com")], "31 days")
snapshot("private-other-project", [(1, "rival.com")])
# Research-only keywords also enter the project scope; repeated ideas/saved words dedupe.
research = str(uuid.uuid4())
sql(f"INSERT INTO keyword_researches(id,project_id,seed,country,language,status) VALUES ('{research}','{p['id']}','fixture','US','en','completed');"
    f"INSERT INTO keyword_ideas(id,research_id,keyword,word_count,popularity,intent) VALUES ('{uuid.uuid4()}','{research}','researched',1,50,'informational'),('{uuid.uuid4()}','{research}','shared',1,50,'informational');")
snapshot("researched", [(21, "rival.com")])
path = root + "/domain-overview?domain=" + urllib.parse.quote("https://news.rival.com/path")
r = api(path)
assert r["domain"] == "rival.com" and r["project_domain"] == "ours.com", r
assert (r["scope_keywords"], r["sampled_keywords"], r["checked_keywords"], r["exact_keywords"], r["matched_keywords"]) == (5, 5, 4, 3, 3), r
assert r["estimated_monthly_traffic"] is None
assert r["distribution"] == {"top_3": 1, "positions_4_10": 0, "positions_11_20": 1, "positions_21_30": 1}
assert r["keyword_gaps"] == ["gap", "researched"]
assert r["top_pages"][0]["keywords"] == 3
assert r["competitors"] == [{"domain": "third.com", "shared_keywords": 2}, {"domain": "ours.com", "shared_keywords": 1}]
assert r["keywords"][0]["position"] == 2 and r["keywords"][0]["exact"] is False
assert r["keywords"][0]["project_position"] == 3
assert api(root + "/domain-overview")["domain"] == "ours.com"
assert api(root + "/domain-overview?domain=unknown.com")["matched_keywords"] == 0
assert api(root + "/domain-overview?domain=rival.com&country=ID")["scope_keywords"] == 0
api(path, status=404, client=outsider)
api(path, status=401, client=session())
api(root + "/domain-overview?domain=co.uk", status=422)
api(root + "/domain-overview?country=USA", status=422)
api(root + "/domain-overview?language=EN", status=422)
key = api("/api-keys", "POST", {"name": "Competitive regression"}, 201)
request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
    "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1,
    "method": "tools/call", "params": {"name": "get_domain_overview", "arguments": {"project_id": p["id"], "domain": "rival.com"}}}).encode())
with urllib.request.urlopen(request) as response:
    payload = json.loads(response.read())
mcp = json.loads(payload["result"]["content"][0]["text"])
assert mcp["ok"] and mcp["data"] == r, mcp
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
# Bound the project scope while retaining its real size in the response.
sql(f"INSERT INTO saved_keywords(id,project_id,keyword,country,language) SELECT gen_random_uuid(),'{p['id']}', 'bounded ' || lpad(n::text,4,'0'),'US','en' FROM generate_series(1,501) n;")
bounded = api(path)
assert bounded["scope_keywords"] == 506 and bounded["sampled_keywords"] == 500
assert any("500" in w for w in bounded["warnings"])
print("PASS: competitive research, latest/empty/stale snapshots, market/project/tenant isolation, gaps, scope cap, validation, MCP parity")
