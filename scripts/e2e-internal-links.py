#!/usr/bin/env python3
"""Stored internal-link regression. ONLY run against a disposable m6check DB.
python scripts/e2e-internal-links.py http://127.0.0.1:3401 seoseo-m6check-db
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
    result = subprocess.run(["docker", "exec", "-i", database, "psql", "-At", "-v", "ON_ERROR_STOP=1", "-U", "seoseo", "-d", "seoseo"],
        input=statement, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email": f"graph-{uuid.uuid4().hex}@example.com",
        "password": "disposable-internal-links-password", "name": "Internal links regression"}, 201, client=client)
me, mate = api("/me"), api("/me", client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "Internal links regression", "domain": "example.com"}, 201)
root = "/projects/" + p["id"]


def audit(state="completed"):
    aid = str(uuid.uuid4())
    sql(f"INSERT INTO audits(id,project_id,start_url,status,phase,pages_crawled,pages_total,pages_failed,stop_reason) VALUES ('{aid}','{p['id']}','https://example.com/','{state}','done',12,12,1,'page_limit');")
    return aid


aid = audit()
nodes = {}
for name in ["", "a-source", "b-linked", "c-linked", "d-nofollow", "e-redirect", "f-canonical", "g-error", "h-noindex", "i-unrelated", "j-weak", "z-outside-sample"]:
    nid = str(uuid.uuid4())
    nodes[name] = nid
    title = "Site Home" if name == "" else "Technical clothing" if name == "i-unrelated" else "Technical SEO auditing"
    status = 404 if name == "g-error" else 200
    indexable = str(name not in ["g-error", "h-noindex"]).lower()
    redirects = '[{"url":"https://example.com/e-redirect"}]' if name == "e-redirect" else '[]'
    canonical = "'https://other.example/canonical'" if name == "f-canonical" else "NULL"
    sql(f"INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title,is_indexable,redirect_chain,canonical_url) VALUES ('{nid}','{aid}','https://example.com/{name}','{nid}',{status},'html',1,'{title}',{indexable},'{redirects}',{canonical});")


def edge(source, target, nofollow=False, anchor="Audit guide"):
    target_id = nodes.get(target, "not-fetched")
    sql(f"INSERT INTO audit_links(audit_id,from_page_id,to_url_hash,to_url,nofollow,anchor) VALUES ('{aid}','{nodes[source]}','{target_id}','https://example.com/{target}',{str(nofollow).lower()},'{anchor}');")


edge("", "b-linked")
edge("", "c-linked")
edge("z-outside-sample", "b-linked")
edge("a-source", "c-linked", True)
edge("a-source", "c-linked", anchor="Technical audit")
edge("a-source", "c-linked")
edge("a-source", "d-nofollow", True)
edge("a-source", "a-source")
edge("a-source", "unfetched")
path = root + "/audits/" + aid + "/internal-links"
jobs_before = sql("SELECT count(*) FROM jobs;")
result = api(path)
assert result["total_pages"] == 12 and result["sampled_pages"] == 12, result
assert result["total_sample_edges"] == 5 and len(result["edges"]) == 5 and not result["edges_truncated"], result
by_id = {node["id"]: node for node in result["nodes"]}
a = by_id[nodes["a-source"]]
assert (a["incoming_sources"], a["outgoing_targets"], a["unfetched_targets"]) == (0, 3, 1), a
d = by_id[nodes["d-nofollow"]]
assert (d["incoming_sources"], d["followed_incoming_sources"]) == (1, 0), d
combined = next(e for e in result["edges"] if e["source_id"] == nodes["a-source"] and e["target_id"] == nodes["c-linked"])
assert combined["occurrences"] == 3 and combined["followed"], combined
existing = {(e["source_id"], e["target_id"]) for e in result["edges"]}
excluded = {nodes[n] for n in ["e-redirect", "f-canonical", "g-error", "h-noindex"]}
assert result["suggestions"], result
for suggestion in result["suggestions"]:
    source, target = suggestion["source_id"], suggestion["target_id"]
    assert source != target and (source, target) not in existing
    assert source not in excluded and target not in excluded
    assert target not in [nodes[""], nodes["b-linked"], nodes["c-linked"]]
    assert len(suggestion["shared_terms"]) >= 2
assert any("incomplete" in w for w in result["warnings"])
assert api(path, client=member) == result
api(path, client=outsider, status=404)
api(path, client=session(), status=401)
other = api("/projects", "POST", {"name": "Other project"}, 201)
api("/projects/" + other["id"] + "/audits/" + aid + "/internal-links", status=404)
sample = api(path + "?limit=10")
assert sample["sampled_pages"] == 10 and sample["total_pages"] == 12
assert by_id[nodes["b-linked"]]["followed_incoming_sources"] == 2
assert next(n for n in sample["nodes"] if n["id"] == nodes["b-linked"])["followed_incoming_sources"] == 2
assert all(s["target_id"] != nodes["b-linked"] for s in sample["suggestions"])
for limit in [0, 9, 501]:
    api(path + f"?limit={limit}", status=422)
for state in ["queued", "running", "failed", "cancelled"]:
    other_audit = audit(state)
    api(root + "/audits/" + other_audit + "/internal-links", status=409)
empty = audit()
assert api(root + "/audits/" + empty + "/internal-links")["nodes"] == []
# Dense fixture: 72 * 71 = 5112 distinct non-self edges, exceeding the graph cap.
dense = audit()
sql(f"INSERT INTO audit_pages(id,audit_id,url,url_hash,status_code,fetch_class,depth,title) SELECT gen_random_uuid(),'{dense}','https://example.com/dense/'||n,n::text,200,'html',1,'Technical SEO auditing' FROM generate_series(1,72) n;"
    f"INSERT INTO audit_links(audit_id,from_page_id,to_url_hash,to_url) SELECT '{dense}',s.id,t.url_hash,t.url FROM audit_pages s CROSS JOIN audit_pages t WHERE s.audit_id='{dense}' AND t.audit_id='{dense}' AND s.id<>t.id;")
bounded = api(root + "/audits/" + dense + "/internal-links")
assert bounded["total_sample_edges"] == 5112 and len(bounded["edges"]) == 5000 and bounded["edges_truncated"]
assert not bounded["suggestions"] and any("disabled" in w for w in bounded["warnings"])
key = api("/api-keys", "POST", {"name": "Internal links regression"}, 201)
request = urllib.request.Request(origin + "/mcp", method="POST", headers={"Authorization": "Bearer " + key["secret"],
    "Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, data=json.dumps({"jsonrpc": "2.0", "id": 1,
    "method": "tools/call", "params": {"name": "get_internal_link_graph", "arguments": {"project_id": p["id"], "audit_id": aid}}}).encode())
with urllib.request.urlopen(request) as response:
    payload = json.loads(response.read())
assert json.loads(payload["result"]["content"][0]["text"]) == {"ok": True, "data": result}
api(root + "/archive", "POST")
assert api(path) == result
assert jobs_before == sql("SELECT count(*) FROM jobs;"), "Graph reads queued jobs"
api("/api-keys/" + key["key"]["id"], "DELETE", status=204)
print("PASS: full-audit counts, bounded graph, nofollow/duplicate/self/unfetched links, editorial eligibility, sample boundary, auth/tenant/project/member/archive isolation, terminal state gates, dense edge cap, MCP parity, validation and no queued jobs")
