#!/usr/bin/env python3
"""Disposable RT-07/AI-05 regression: API_ORIGIN M6CHECK_DB. No provider/model calls."""
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid

origin, database = sys.argv[1:3]
assert database.startswith("seoseo-m6check")


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, status=200, client=owner):
    request = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "x-seoseo-csrf": "1", "Accept": "application/json, text/event-stream"})
    try:
        response = client.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == status, (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"], input=statement, text=True, capture_output=True, encoding="utf-8")
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def literal(value):
    return "'" + value.replace("'", "''") + "'"


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"evidence-{uuid.uuid4().hex}@example.com", "password": "disposable-evidence-password", "name": "Evidence research regression"}, 201, client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "Evidence research", "domain": "example.com"}, 201)
root = "/projects/" + p["id"]
jobs_before = sql("SELECT count(*) FROM jobs;")
prefix = "ev " + uuid.uuid4().hex[:8] + " "
names = [prefix + k for k in ["multi", "single", "duplicates", "stale", "empty", "wrongmarket", "unchecked"]]
api(root + "/saved-keywords", "POST", {"keywords": names})


def snapshot(keyword, urls, age="0 days", country="US", exact=True):
    sid = str(uuid.uuid4())
    statement = f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES ('{sid}','fixture',{str(exact).lower()},'{keyword}','{country}','en',now()-interval '{age}');"
    for position, url in urls:
        statement += f"INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('{sid}',{position},'{url}','example.com','Synthetic fixture');"
    sql(statement)


snapshot(names[0], [(1, "https://example.com/a"), (3, "https://example.com/b"), (4, "https://example.com/a#fragment"), (5, "https://blog.example.com/c"), (2, "https://foreign.example/z"), (31, "https://example.com/outside")], exact=False)
snapshot(names[1], [(7, "https://example.com/a")])
snapshot(names[2], [(1, "https://example.com/a#one"), (2, "https://example.com/a#two")])
snapshot(names[3], [(1, "https://example.com/a"), (2, "https://example.com/b")], age="31 days")
snapshot(names[4], [(1, "https://example.com/a"), (2, "https://example.com/b")], age="1 day")
snapshot(names[4], [])
snapshot(names[5], [(1, "https://example.com/a"), (2, "https://example.com/b")], country="GB")
flags = api(root + "/cannibalization")
assert (flags["scope_keywords"], flags["sampled_keywords"], flags["checked_keywords"], flags["matched_keywords"], flags["flagged_keywords"]) == (7, 7, 4, 3, 1), flags
assert flags["items"][0]["keyword"] == names[0] and len(flags["items"][0]["pages"]) == 3
assert not flags["items"][0]["exact"] and flags["items"][0]["pages"][0]["position"] == 1
assert api(root + "/cannibalization?offset=1")["items"] == []
api(root + "/cannibalization", client=outsider, status=404)
api(root + "/cannibalization", client=session(), status=401)
assert api(root + "/cannibalization", client=member) == flags
for query in ["limit=0", "limit=101", "country=USA"]:
    api(root + "/cannibalization?" + query, status=422)


def index_fixture(pid, domain="example.com", country="US", prompts=None, models=None):
    tid, rid = str(uuid.uuid4()), str(uuid.uuid4())
    cfg = {"brands": ["Acme"], "prompts": prompts or ["Best technical SEO tools?", "Which SEO crawler?", "C++ reference?", "Unread prompt?"], "models": models or ["fixture-a", "fixture-b"], "country": country, "language": "en"}
    data = literal(json.dumps(cfg))
    sql(f"INSERT INTO ai_visibility_trackers(id,project_id,domain,name,config) VALUES ('{tid}','{pid}','{domain}','Index fixture',{data}); INSERT INTO ai_visibility_runs(id,tracker_id,config,gateway_id,status) VALUES ('{rid}','{tid}',{data},'fixture-gateway','failed');")
    return tid, rid


def answer(rid, prompt, model, text=None, error=None):
    if error:
        sql(f"INSERT INTO ai_visibility_answers(run_id,prompt_index,model,error) VALUES ('{rid}',{prompt},'{model}',{literal(error)});")
    else:
        sql(f"INSERT INTO ai_visibility_answers(run_id,prompt_index,model,answer,mentions,sources,engine,grounding) VALUES ('{rid}',{prompt},'{model}',{literal(text)},'[]','[]','compatible','none');")


tid, rid = index_fixture(p["id"])
answer(rid, 0, "fixture-a", "Acme helps with technical SEO.")
answer(rid, 0, "fixture-b", "AcmeCorp is a different word.")
answer(rid, 1, "fixture-a", "Use acme for a saved comparison.")
answer(rid, 1, "fixture-b", error="Synthetic blocked response")
answer(rid, 2, "fixture-a", "C++ [Acme]. and Éclair are literal terms.")
# Old website, other project and other market evidence must remain separate.
_, old = index_fixture(p["id"], domain="old.example", prompts=["Secret old prompt"], models=["fixture-a"])
answer(old, 0, "fixture-a", "Acme old website")
other = api("/projects", "POST", {"name": "Other index", "domain": "example.com"}, 201)
_, private = index_fixture(other["id"], prompts=["Other project secret"], models=["fixture-a"])
answer(private, 0, "fixture-a", "Acme other project")
_, gb = index_fixture(p["id"], country="GB", prompts=["British saved prompt"], models=["fixture-a"])
answer(gb, 0, "fixture-a", "Acme British sample")
path = root + "/ai-index?brand=Acme"
result = api(path)
assert result["summary"] == {"retained_runs": 1, "unique_prompts": 4, "expected_answers": 8, "successful_answers": 4, "failed_answers": 1, "unchecked_answers": 3, "brand_mentions": 3}, result
assert result["mention_rate_percent"] == 75 and result["total_matching"] == 3
assert len(result["prompts"]) == 3 and result["answers"] == []
assert api(path + "&limit=1&offset=1")["prompts"] == result["prompts"][1:2]
answers = api(path + "&view=answers")
assert len(answers["answers"]) == 3 and all("acme" in a["excerpt"].lower() for a in answers["answers"])
assert all(a["engine"] == "compatible" and a["grounding"] == "none" for a in answers["answers"])
topic = api(path + "&topic=SEO")
assert topic["summary"]["expected_answers"] == 4 and topic["summary"]["brand_mentions"] == 2
assert abs(topic["mention_rate_percent"] - 200 / 3) < 0.001
for brand in ["C++ [Acme].", "éclair"]:
    assert api(root + "/ai-index?brand=" + urllib.parse.quote(brand))["summary"]["brand_mentions"] == 1
assert api(root + "/ai-index?brand=" + urllib.parse.quote(".*"))["summary"]["brand_mentions"] == 0
assert api(root + "/ai-index?topic=%25")["summary"]["expected_answers"] == 0
all_prompts = api(root + "/ai-index")
assert all_prompts["total_matching"] == 4 and all_prompts["summary"]["brand_mentions"] is None and all_prompts["mention_rate_percent"] is None
assert api(root + "/ai-index?country=GB&brand=Acme")["summary"]["successful_answers"] == 1
assert api(path, client=member) == result
api(path, client=outsider, status=404)
api(path, client=session(), status=401)
for query in ["brand=", "brand=%0A", "limit=0", "limit=101", "offset=-1", "view=global", "country=USA"]:
    api(root + "/ai-index?" + query, status=422)
empty = api("/projects", "POST", {"name": "No website index"}, 201)
assert api("/projects/" + empty["id"] + "/ai-index?brand=Acme")["mention_rate_percent"] is None
api(root, "PATCH", {"domain": "new.example"})
assert api(path)["summary"]["expected_answers"] == 0
api(root, "PATCH", {"domain": "example.com"})
api(root + "/archive", "POST")
assert api(path) == result and api(root + "/cannibalization") == flags
assert jobs_before == sql("SELECT count(*) FROM jobs;")
key = api("/api-keys", "POST", {"name": "Evidence research regression"}, 201)


def mcp(name, arguments):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"], "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).encode())
    with urllib.request.urlopen(request) as response:
        payload = json.loads(response.read())
    return json.loads(payload["result"]["content"][0]["text"])


assert mcp("get_cannibalization_flags", {"project_id": p["id"]}) == {"ok": True, "data": flags}
assert mcp("explore_ai_prompts", {"project_id": p["id"], "brand": "Acme"}) == {"ok": True, "data": result}
assert mcp("lookup_ai_brand", {"project_id": p["id"], "brand": "Acme"}) == {"ok": True, "data": answers}
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: multi-URL candidates, fragment/foreign/stale/empty/depth handling, saved prompt/answer index, lexical and Unicode boundaries, regex/LIKE literals, failure/unchecked denominators, frozen markets, website/project/tenant/member/archive isolation, paging, MCP parity and no jobs")
