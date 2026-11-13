#!/usr/bin/env python3
"""Cached SERP clustering regression; ONLY use a disposable seoseo-m6check DB.

python scripts/e2e-clustering.py http://127.0.0.1:3401 seoseo-m6check-db
Requires password authentication, ALLOW_SIGNUP=true and local psql as seoseo.
"""
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin, database = sys.argv[1:3]
assert database.startswith("seoseo-m6check"), "Use a dedicated disposable m6check database"


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


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


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"cluster-{uuid.uuid4().hex}@example.com",
        "password": "disposable-clustering-password", "name": "Clustering regression"}, 201, client=client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "Clustering regression", "domain": "ours.example"}, 201)
root = "/projects/" + p["id"]
path = root + "/keyword-clusters?min_shared=2"
assert api(path)["scope_keywords"] == 0
names = ["a", "b", "c", "different provider", "aggregated", "missing", "stale", "empty", "duplicates", "deep", "wrong market", "research only", "tracked only", "imported only"]
api(root + "/saved-keywords", "POST", {"keywords": names[:-3]})
other = api("/projects", "POST", {"name": "Private scope"}, 201)
api("/projects/" + other["id"] + "/saved-keywords", "POST", {"keywords": ["private other project"]})


def snapshot(keyword, urls, provider="fixture", age="0 days", country="US", exact=True, first=1):
    sid = str(uuid.uuid4())
    statement = f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES ('{sid}','{provider}',{str(exact).lower()},'{keyword}','{country}','en',now()-interval '{age}');"
    for position, url in enumerate(urls, first):
        statement += f"INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('{sid}',{position},'{url}','fixture.example','Fixture');"
    sql(statement)


urls = ["https://fixture.example/1", "https://fixture.example/2"]
snapshot("a", urls)
snapshot("b", urls + ["https://fixture.example/3", "https://fixture.example/4"])
snapshot("c", ["https://fixture.example/3", "https://fixture.example/4"])
snapshot("different provider", urls, provider="other")
snapshot("aggregated", urls, exact=False)
snapshot("stale", urls, age="8 days")
snapshot("empty", urls, age="1 day")
snapshot("empty", [])
snapshot("duplicates", ["https://fixture.example/1#one", "https://fixture.example/1#two"])
snapshot("deep", urls, first=11)
snapshot("wrong market", urls, country="GB")
snapshot("private other project", urls)
research = str(uuid.uuid4())
sql(f"INSERT INTO keyword_researches(id,project_id,seed,country,language,status) VALUES ('{research}','{p['id']}','fixture','US','en','completed');"
    f"INSERT INTO keyword_ideas(id,research_id,keyword,word_count,popularity,intent) VALUES ('{uuid.uuid4()}','{research}','research only',2,50,'informational'),('{uuid.uuid4()}','{research}','a',1,50,'informational');")
tracker = api(root + "/rank-trackers", "POST", {"name": "Cluster scope", "domain": "ours.example", "country": "US", "language": "en"}, 201)
api(root + "/rank-trackers/" + tracker["id"] + "/keywords", "POST", {"keywords": ["tracked only"]})
api(root + "/competitive-volumes", "POST", {"source_label": "Synthetic fixture", "rows": [{"keyword": "imported only", "monthly_search_volume": 10}]})
snapshot("research only", urls)
snapshot("tracked only", urls)
snapshot("imported only", urls)
result = api(path)
assert result["scope_keywords"] == len(names), result
assert result["sampled_keywords"] == len(names) and result["eligible_keywords"] == 8, result
assert len(result["clusters"]) == 1 and result["clusters"][0]["minimum_shared_urls"] == 2, result
assert [k["keyword"] for k in result["clusters"][0]["keywords"]] == ["a", "b", "imported only", "research only", "tracked only"], result
assert [k["keyword"] for k in result["ungrouped"]] == ["aggregated", "c", "different provider"], result
assert len(result["unavailable"]) == 6, result
assert api(path, client=member) == result
api(path, client=outsider, status=404)
api(path, client=session(), status=401)
assert api(root + "/keyword-clusters?min_shared=3")["clusters"] == []
assert api(root + "/keyword-clusters?country=ID")["scope_keywords"] == 0
for query in ["min_shared=0", "min_shared=11", "country=USA", "language=EN"]:
    api(root + "/keyword-clusters?" + query, status=422)
key = api("/api-keys", "POST", {"name": "Clustering regression"}, 201)


def mcp(project_id):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
        "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1,
        "method": "tools/call", "params": {"name": "cluster_keywords", "arguments": {"project_id": project_id, "min_shared": 2}}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.loads(response.read())
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp(p["id"]) == {"ok": True, "data": result}
assert not mcp(str(uuid.uuid4()))["ok"]
# The endpoint remains readable for archived projects, like other evidence views.
api(root + "/archive", "POST")
assert api(path) == result
api(root + "/restore", "POST")
sql(f"INSERT INTO saved_keywords(id,project_id,keyword,country,language) SELECT gen_random_uuid(),'{p['id']}', 'bounded ' || lpad(n::text,4,'0'),'US','en' FROM generate_series(1,501) n;")
bounded = api(path)
assert bounded["scope_keywords"] == len(names) + 501 and bounded["sampled_keywords"] == 500
assert any("500" in warning for warning in bounded["warnings"])
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: clustering, complete-link chain prevention, provider/exact/market isolation, missing/stale/empty/duplicate/deep results, project scopes, member access, tenant/auth isolation, MCP parity, validation, archive reads and 500-keyword bound")
