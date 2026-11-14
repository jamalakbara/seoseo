#!/usr/bin/env python3
"""M8 acceptance against ONLY the disposable m8check stack. No live paid API calls."""
import datetime
import http.cookiejar
import json
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:3601"
database = "seoseo-m8check-db-1"
def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
owner, outsider, member, anonymous = [session() for _ in range(4)]
def api(path, method="GET", body=None, expected=200, client=owner, csrf=True):
    headers = {"Content-Type": "application/json"}
    if csrf: headers["x-seoseo-csrf"] = "1"
    req = urllib.request.Request(origin + "/api/v1" + path, data=json.dumps(body).encode() if body is not None else None, method=method, headers=headers)
    try: r = client.open(req, timeout=55)
    except urllib.error.HTTPError as e: r = e
    raw = r.read()
    assert r.status == expected, (path, r.status, raw[:1000])
    return json.loads(raw) if raw and r.headers.get("content-type", "").startswith("application/") else raw
def sql(query):
    r = subprocess.run(["docker", "exec", "-i", database, "psql", "-U", "seoseo", "-d", "seoseo", "-v", "ON_ERROR_STOP=1", "-At"], input=query, text=True, capture_output=True)
    assert r.returncode == 0, r.stderr
    return r.stdout.strip()
password = "disposable-m8-test-password"
identities = []
for client in [owner, outsider, member]:
    email = f"m8-{uuid.uuid4().hex}@example.invalid"
    api("/auth/sign-up", "POST", {"email": email, "password": password, "name": "M8 acceptance"}, 201, client)
    identities.append(email)
me, mate = api("/me"), api("/me", client=member)
sql(f"UPDATE users SET is_instance_admin=true WHERE id='{me['user']['id']}'; INSERT INTO members(organization_id,user_id,role) VALUES('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects", "POST", {"name": "M8 acceptance", "domain": "store.example.com"}, 201)
pid = p["id"]; root = "/projects/" + pid

# Public tools require neither a session nor CSRF and never save the supplied HTML.
samples = {
    "keywords": "coffee", "serp-preview": "<title>A &amp; B</title>",
    "backlinks": '<a href="https://example.com/guide" rel="nofollow">Guide</a>',
    "authority": "", "robots": "User-agent: *\nDisallow: /private",
    "sitemap": "<urlset><url><loc>https://example.com/</loc></url></urlset>",
    "meta": '<meta name="description" content="Useful">', "schema": '{"@context":"https://schema.org","@type":"Organization"}',
    "hreflang": '<link rel="alternate" hreflang="en" href="/en">',
}
for tool, content in samples.items():
    v = api("/public/tools", "POST", {"tool": tool, "content": content, "url": "https://example.com/private"}, client=anonymous, csrf=False)
    assert v is not None, tool
    if tool == "robots": assert v["allowed"] is False
    if tool == "backlinks": assert v["matching_links"][0]["nofollow"] is True
    if tool == "authority": assert v["sr"] is None
for tool in ["domain-age", "redirects"]:
    api("/public/tools", "POST", {"tool": tool, "url": "http://127.0.0.1/"}, 422, anonymous)
api("/public/tools", "POST", {"tool": "sitemap", "content": "<urlset><url>"}, 422, anonymous)
api("/public/tools", "POST", {"tool": "schema", "content": "not JSON"}, 422, anonymous)
api("/public/tools", "POST", {"tool": "meta", "content": "x" * 262145}, 422, anonymous)
print("PASS public tools, parsers, size limits, private-network denial")

raw = '203.0.113.10 - private-user [10/Oct/2000:13:55:36 -0700] "GET /account?token=private-token HTTP/1.1" 404 12 "private-referrer" "Googlebot"\ninvalid'
summary = api(root + "/log-analyses", "POST", {"content": raw})
assert summary["summary"]["accepted"] == 1 and summary["summary"]["rejected"] == 1
stored = sql(f"SELECT summary::text FROM log_analyses WHERE project_id='{pid}';")
for private in ["203.0.113.10", "private-user", "/account", "private-token", "private-referrer"]: assert private not in stored
for _ in range(21): api(root + "/log-analyses", "POST", {"content": raw})
assert len(api(root + "/log-analyses")) == 20
api(root + "/log-analyses", "POST", {"content": raw}, 403, member)
api(root + "/log-analyses", client=outsider, expected=404)
api(root + "/log-analyses", "POST", {"content": raw}, 403, csrf=False)
api(root + "/log-analyses", "POST", {"content": "x" * 1048577}, 422)
row = api(root + "/log-analyses")[0]
api(root + "/log-analyses/" + row["id"], "DELETE", expected=204)
print("PASS tenant scope, roles, CSRF, private log aggregation, retention and deletion")

assert api(root + "/volatility")["index"] is None
keyword = "m8 sample " + uuid.uuid4().hex[:8]
country, language = p["country_code"], p["language_code"]
sql(f"INSERT INTO saved_keywords(id,project_id,keyword,country,language) VALUES('{uuid.uuid4()}','{pid}','{keyword}','{country}','{language}');")
missing = api(root + "/volatility")
assert missing["excluded_keywords"] == 1 and missing["serp_pairs"][0]["current_id"] is None
snaps = [str(uuid.uuid4()) for _ in range(2)]
for n, snap in enumerate(snaps):
    sql(f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES('{snap}','fixture',true,'{keyword}','{country}','{language}',now()-interval '{2-n} hours');")
    for i in range(1, 11):
        url = f"https://example.com/{i if n == 0 or i > 2 else i+10}"
        sql(f"INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES('{snap}',{i},'{url}','example.com','Fixture result {i}');")
v = api(root + "/volatility")
assert v["index"] == 20 and v["comparable_keywords"] == 1, v
assert v["serp_pairs"][0]["previous_id"] == snaps[0]
sql(f"DELETE FROM serp_items WHERE snapshot_id='{snaps[1]}' AND position=10;")
assert api(root + "/volatility")["index"] is None
sql(f"INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES('{snaps[1]}',10,'https://example.com/10','example.com','Fixture result 10');")
api(root + "/volatility", client=outsider, expected=404)
print("PASS exact-engine SERP churn, frozen evidence pairs, incomplete lists remain unknown")

expires = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=1)).isoformat()
credential = {"host": "store.example.com", "signature": "sig1=:disposable-secret:", "signature_input": 'sig1=("@authority" "signature-agent");tag="web-bot-auth"', "signature_agent": '"https://shopify.com"', "expires_at": expires}
api(root + "/crawler-credential", "PUT", credential, 403, member)
api(root + "/crawler-credential", "PUT", {**credential, "host": "other.example.com"}, 422)
api(root + "/crawler-credential", "PUT", {**credential, "signature": "line\r\ninjection"}, 422)
view = api(root + "/crawler-credential", "PUT", credential)
assert view["configured"] and "secret" not in json.dumps(view)
assert sql(f"SELECT octet_length(secret)>0 AND position('disposable-secret' in encode(secret,'escape'))=0 FROM crawler_credentials WHERE project_id='{pid}'") == "t"
api(root + "/crawler-credential", client=outsider, expected=404)
api(root + "/crawler-credential", "DELETE", expected=204)
assert not api(root + "/crawler-credential")["configured"]
print("PASS signed crawler credentials: exact host, encrypted storage, validation, redaction, revocation")

body = {"keywords": ["coffee"], "geo_target_id": "2840", "language_id": "1000", "currency": "USD"}
cpc = api(root + "/keyword-cpc", "POST", body)
assert cpc["available"] is False and cpc["rows"][0]["average_cpc_micros"] is None
api(root + "/keyword-cpc", "POST", body, 403, member)
api(root + "/keyword-cpc", "POST", {**body, "geo_target_id": "../x"}, 422)
telemetry = api("/settings/telemetry")
assert telemetry["enabled"] is False and set(telemetry["payload"]) == {"product", "schema", "version"}
api("/settings/telemetry/send", "POST", expected=409)
api("/settings/telemetry", "PUT", {"enabled": True}, client=outsider, expected=403)
api("/settings/telemetry", "PUT", {"enabled": True})
api("/settings/telemetry/send", "POST", expected=409)  # no receiver: still no network
api("/settings/telemetry", "PUT", {"enabled": False})
print("PASS CPC missing-key unknowns, explicit scope, telemetry defaults and administrator controls")

key = api("/api-keys", "POST", {"name": "M8 widget acceptance"}, 201)
def mcp(method, params=None, auth=True):
    headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream", "MCP-Protocol-Version": "2025-06-18"}
    if auth: headers["Authorization"] = "Bearer " + key["secret"]
    req = urllib.request.Request(origin + "/mcp", data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode(), headers=headers)
    try: r = urllib.request.urlopen(req, timeout=40)
    except urllib.error.HTTPError as e: r = e
    if not auth: assert r.status == 401; return
    assert r.status == 200
    result = json.load(r)
    assert "error" not in result, result
    return result["result"]
mcp("resources/list", auth=False)
info = mcp("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "M8 test", "version": "1"}})
assert "resources" in info["capabilities"]
tools = mcp("tools/list")["tools"]
widget = next(t for t in tools if t["name"] == "show_serp_widget")
uri = widget["_meta"]["ui"]["resourceUri"]
resource = mcp("resources/read", {"uri": uri})["contents"][0]
assert resource["mimeType"] == "text/html;profile=mcp-app" and "textContent" in resource["text"]
view = mcp("tools/call", {"name": "show_serp_widget", "arguments": {"project_id": pid, "keyword": keyword}})
assert len(view["structuredContent"]["items"]) == 10
assert json.loads(view["content"][0]["text"])["ok"]
fallback = mcp("tools/call", {"name": "show_gsc_widget", "arguments": {"project_id": pid, "start_date": "2026-10-01", "end_date": "2026-10-07"}})
assert fallback["isError"]  # no configured GSC grant, no invented measurements
api("/api-keys/" + key["key"]["id"], "DELETE", expected=204)
print(f"PASS MCP authentication, {len(tools)} tools, resource metadata, HTML widget and structured/JSON fallback")

sql("UPDATE public_tool_limits SET requests=119,window_at=now() WHERE bucket='analysis';")
api("/public/tools", "POST", {"tool": "meta", "content": "<title>Budget</title>"}, client=anonymous)
api("/public/tools", "POST", {"tool": "meta", "content": "<title>Budget</title>"}, 503, anonymous)
sql("UPDATE public_tool_limits SET window_at=now()-interval '61 seconds' WHERE bucket='analysis';")
api("/public/tools", "POST", {"tool": "meta", "content": "<title>Budget</title>"}, client=anonymous)
assert sql("SELECT requests FROM public_tool_limits WHERE bucket='analysis'") == "1"
print("PASS shared anonymous request budget boundary and minute-window recovery")

# Leave only disposable test identity coordinates for the local browser test, never credentials.
print(json.dumps({"project_id": pid, "owner_email": identities[0]}))
print("PASS M8 API acceptance")
