#!/usr/bin/env python3
"""Offline backlink snapshot regression. Disposable password-auth API/DB only.

python scripts/e2e-backlinks.py http://127.0.0.1:3401 seoseo-m5links-db
Requires ALLOW_SIGNUP=true and docker exec psql as seoseo in the named test DB.
"""
import concurrent.futures
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin, database = sys.argv[1:3]


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


owner, outsider, member = session(), session(), session()


def api(path, method="GET", body=None, expected=200, client=owner, key=None, status_only=False):
    headers = {"Content-Type": "application/json", "x-seoseo-csrf": "1"}
    if key:
        headers["Authorization"] = "Bearer " + key
    request = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None, headers=headers)
    try:
        response = (urllib.request.urlopen if key else client.open)(request, timeout=45)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    if status_only:
        return response.status, json.loads(raw) if raw else None
    assert response.status == expected, (path, response.status, raw[:500])
    return json.loads(raw) if raw else None


def sql(statement):
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"],
        input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"backlinks-{uuid.uuid4().hex}@example.com", "password": "disposable-backlinks-password", "name": "Backlink regression"}, 201, client=client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "Backlink samples", "domain": "example.com"}, 201)
root = "/projects/" + p["id"] + "/backlink-snapshots"


def link(name, **overrides):
    return dict({"source_url": f"https://{name}.org/page#ignored", "target_url": "https://example.com/target#ignored", "anchor": name, "nofollow": None}, **overrides)


def payload(links, dataset="export", domain="example.com"):
    return {"expected_domain": domain, "dataset": dataset, "complete_sample": True, "links": links}


assert api(root) == []
a = api(root, "POST", payload([link("one"), link("two", nofollow=True)]), 201)
assert (a["live_count"], a["new_count"], a["lost_count"], a["comparison_id"]) == (2, 0, 0, None)
b = api(root, "POST", payload([link("one", anchor="updated", nofollow=False), link("three")]), 201)
assert (b["live_count"], b["new_count"], b["lost_count"], b["comparison_id"]) == (2, 1, 1, a["id"])
view = api(root + "/" + b["id"])
assert view["source"] == "user_import" and view["spam_score"] is None and view["referring_domain_count"] == 2
assert view["top_pages"] == [{"value": "https://example.com/target", "links": 2}]
assert sorted(x["state"] for x in view["items"]) == ["live", "lost", "new"]
assert all("#" not in x["source_url"] and "#" not in x["target_url"] for x in view["items"])
assert api(root + "/" + b["id"] + "?state=live")["total"] == 2
assert api(root + "/" + b["id"] + "?state=lost")["items"][0]["source_domain"] == "two.org"
assert len(api(root + "/" + b["id"] + "?limit=1&offset=1")["items"]) == 1
api(root + "/" + b["id"] + "?state=unknown", expected=422)
assert api(root + "/" + b["id"], client=member)["total"] == 3
api(root, "POST", payload([]), expected=403, client=member)
api(root + "/" + b["id"], "DELETE", expected=403, client=member)
for client, status in ((outsider, 404), (session(), 401)):
    api(root, expected=status, client=client)
    api(root + "/" + b["id"], expected=status, client=client)
    api(root, "POST", payload([]), expected=status, client=client)
    api(root + "/" + b["id"], "DELETE", expected=status, client=client)

# Datasets stay independent, including their first empty baseline.
separate = api(root, "POST", payload([], "separate"), 201)
assert separate["comparison_id"] is None and separate["lost_count"] == 0
empty = api(root, "POST", payload([]), 201)
assert empty["comparison_id"] == b["id"] and empty["live_count"] == 0 and empty["lost_count"] == 2
again = api(root, "POST", payload([]), 201)
assert again["comparison_id"] == empty["id"] and again["lost_count"] == 0
returned = api(root, "POST", payload([link("one")]), 201)
assert returned["new_count"] == 1 and returned["lost_count"] == 0

for bad in [dict(payload([]), complete_sample=False), dict(payload([]), dataset="bad key"),
            payload([link("one"), link("one")]), payload([link("one", target_url="https://notexample.com/")]),
            payload([link("one", source_url="https://sub.example.com/page")]),
            payload([link("one", source_url="http://127.0.0.1/")]),
            payload([link("one", source_url="https://user:pass@one.org/")]),
            payload([link("one", anchor="x" * 501)]), payload([link("one")] * 1001)]:
    api(root, "POST", bad, expected=422)
api(root, "POST", payload([], domain="changed.org"), expected=409)

# Deleting a prior snapshot does not rewrite an already recorded diff.
api(root + "/" + a["id"], "DELETE", expected=204)
assert api(root + "/" + b["id"])["snapshot"] == b
other = api("/projects", "POST", {"name": "Other project", "domain": "example.com"}, 201)
api("/projects/" + other["id"] + "/backlink-snapshots/" + b["id"], expected=404)

key = api("/api-keys", "POST", {"name": "Backlink regression"}, 201)


def tool(name, args):
    request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
        "Content-Type": "application/json", "Accept": "application/json, text/event-stream"},
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}}).encode())
    with urllib.request.urlopen(request, timeout=45) as response:
        result = json.load(response)
    return json.loads(result["result"]["content"][0]["text"])


assert tool("get_backlink_snapshot", {"project_id": p["id"], "snapshot_id": b["id"]})["data"] == view
assert tool("list_backlink_snapshots", {"project_id": p["id"]})["data"] == api(root)
mcp_import = tool("import_backlink_snapshot", dict(payload([], "mcp"), project_id=p["id"]))
assert mcp_import["ok"]
assert tool("delete_backlink_snapshot", {"project_id": p["id"], "snapshot_id": mcp_import["data"]["id"]})["ok"]

# Concurrent same-dataset imports form a chain rather than comparing both to an old baseline.
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    results = list(pool.map(lambda name: api(root, "POST", payload([link(name)], "concurrent"), 201, key=key["secret"]), ["four", "five"]))
ids = {x["id"] for x in results}
assert sum(x["comparison_id"] in ids for x in results) == 1, results
latest = next(x for x in api(root) if x["dataset"] == "concurrent")
assert latest["comparison_id"] in ids and latest["new_count"] == 1 and latest["lost_count"] == 1, latest

# Quota check and write are atomic under competing imports.
quota = "/projects/" + other["id"] + "/backlink-snapshots"
sql(f"INSERT INTO backlink_snapshots(id,project_id,domain,dataset,live_count,new_count,lost_count) SELECT gen_random_uuid(),'{other['id']}','example.com','quota',0,0,0 FROM generate_series(1,99);")
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    codes = list(pool.map(lambda name: api(quota, "POST", payload([], name), key=key["secret"], status_only=True)[0], ["quota-a", "quota-b"]))
assert sorted(codes) == [201, 409], codes
assert len(api(quota)) == 100

# Detail reads hide old website evidence; metadata/deletion still support cleanup.
api("/projects/" + p["id"], "PATCH", {"domain": "newsite.org"})
assert any(s["id"] == b["id"] for s in api(root))
api(root + "/" + b["id"], expected=404)
api(root, "POST", payload([]), expected=409)
fresh = api(root, "POST", payload([], domain="newsite.org"), 201)
assert fresh["comparison_id"] is None
api(root + "/" + b["id"], "DELETE", expected=204)
api("/projects/" + p["id"] + "/archive", "POST")
api(root, "POST", payload([], domain="newsite.org"), expected=409)
api(root + "/" + fresh["id"], "DELETE", expected=404)
assert api(root + "/" + fresh["id"])["total"] == 0
api("/api-keys/" + key["key"]["id"], "DELETE", expected=204)
print("PASS: backlink snapshots, diff/empty/reappearance, aggregates, isolation/roles, validation, deletion, concurrent baselines/quotas, website identity, archived reads and MCP parity")
