#!/usr/bin/env python3
"""Offline GSC regression: use compose.test.yaml + compose.gsc-test.yaml.
Run after smoke: python scripts/e2e-gsc.py EMAIL PASSWORD [http://127.0.0.1:3100]
Only touches disposable projects/users and the local OAuth fixture.
"""
import datetime
import http.cookiejar
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid

email, password = sys.argv[1:3]
origin = sys.argv[3] if len(sys.argv) > 3 else "http://127.0.0.1:3100"
base = origin + "/api/v1"


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()), NoRedirect())


owner = session()


def request(path, method="GET", body=None, expected=200, opener=owner, key=None, raw=False):
    headers = {"Content-Type": "application/json", "X-Seoseo-Csrf": "1"}
    if key:
        headers["Authorization"] = "Bearer " + key
    url = path if path.startswith("http") else base + path
    req = urllib.request.Request(url, method=method, headers=headers, data=json.dumps(body).encode() if body is not None else None)
    try:
        res = opener.open(req, timeout=60)
    except urllib.error.HTTPError as e:
        res = e
    data = res.read().decode()
    assert res.status == expected, (path, res.status, data[:300])
    if expected in (302, 303):
        return res.headers["Location"]
    return data if raw else json.loads(data) if data else None


def control(mode):
    request("http://127.0.0.1:3101/control", "POST", {"mode": mode})


def oauth_callback(root):
    url = request(root + "/connect", "POST")["url"]
    q = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)
    assert q["scope"] == ["https://www.googleapis.com/auth/webmasters.readonly"]
    assert q["code_challenge_method"] == ["S256"]
    callback = request(url, expected=302)
    # Keep the session cookie on the test origin even when WEB_BASE_URL uses localhost.
    parsed = urllib.parse.urlsplit(callback)
    return origin + parsed.path + "?" + parsed.query


request("/auth/sign-in", "POST", {"email": email, "password": password})
projects = [request("/projects", "POST", {"name": "GSC fixture " + str(i), "domain": "example.org"}, 201) for i in range(2)]
root = "/projects/" + projects[0]["id"] + "/gsc"
other = "/projects/" + projects[1]["id"] + "/gsc"
key_row = request("/api-keys", "POST", {"name": "GSC fixture"}, 201)
member_id = None
today = datetime.datetime.now(datetime.timezone.utc).date()
query = {"start_date": str(today - datetime.timedelta(days=31)), "end_date": str(today - datetime.timedelta(days=3)), "dimensions": ["query"], "row_limit": 1}
try:
    control("normal")
    assert request(root)["configured"] and not request(root)["connected"]
    request(root + "/performance", "POST", query, 409)
    request(root + "/connect", "POST", key=key_row["secret"], expected=403)
    request("/projects/" + str(uuid.uuid4()) + "/gsc", expected=404)

    # A different signed-in member must not consume the owner's OAuth state.
    member_email = "gsc-" + uuid.uuid4().hex + "@example.com"
    invite = request("/team/invitations", "POST", {"email": member_email, "role": "member"}, 201)
    member = session()
    request("/auth/sign-up", "POST", {"name": "GSC test member", "email": member_email, "password": "fixture-password-long", "invite_token": invite["token"]}, 201, opener=member)
    member_id = request("/me", opener=member)["user"]["id"]
    callback = oauth_callback(root)
    request(callback, expected=422, opener=member)
    request(callback, expected=303)
    request(callback, expected=422)  # single-use even after success
    declined = oauth_callback(other)
    parts = urllib.parse.urlsplit(declined)
    state = urllib.parse.parse_qs(parts.query)["state"][0]
    request(origin + parts.path + "?" + urllib.parse.urlencode({"state": state, "error": "access_denied"}), expected=422)
    request(declined, expected=422)
    invalid = oauth_callback(other)
    parts = urllib.parse.urlsplit(invalid)
    state = urllib.parse.parse_qs(parts.query)["state"][0]
    request(origin + parts.path + "?" + urllib.parse.urlencode({"state": state, "code": "invalid-code"}), expected=409)
    request(invalid, expected=422)
    assert request(root)["connected"] and request(root)["site_url"] is None
    assert not request(other)["connected"]
    request(root + "/performance", "POST", query, 409)
    sites = request(root + "/sites")
    assert len(sites) == 2 and sites[0]["siteUrl"] == "sc-domain:example.org"
    request(root + "/site", "PUT", {"site_url": "sc-domain:unverified.org"}, 422)
    request(root + "/site", "PUT", {"site_url": "sc-domain:example.org"})
    request(root + "/connect", "POST", expected=403, opener=member)
    request(root + "/site", "PUT", {"site_url": "sc-domain:example.org"}, 403, opener=member)
    assert request(root + "/performance", "POST", query, opener=member)["rows"][0]["clicks"] == 12
    print("PASS GSC OAuth: PKCE, single-use/user-bound state, member permissions, verified properties, project isolation", flush=True)

    data = request(root + "/performance", "POST", query)
    assert data["source"] == "google_search_console" and data["next_start_row"] == 1
    assert data["rows"][0] == {"keys": ["=test formula"], "clicks": 12.0, "impressions": 100.0, "ctr": .12, "position": 3.5}
    assert request(root + "/performance", "POST", {**query, "start_row": 1})["rows"] == []
    request(root + "/performance", "POST", {**query, "row_limit": 1001}, 422)
    request(root + "/performance", "POST", {**query, "end_date": str(today)}, 422)
    request(root + "/performance", "POST", {**query, "dimensions": ["invalid"]}, 422)
    csv = request(root + "/performance.csv", "POST", query, raw=True)
    assert '"\'=test formula"' in csv and '"clicks"' in csv
    request(root + "/inspect", "POST", {"urls": []}, 422)
    request(root + "/inspect", "POST", {"urls": ["file:///etc/passwd"]}, 422)
    results = request(root + "/inspect", "POST", {"urls": ["https://example.org/", "https://example.org/invalid"]})
    assert results[0]["result"]["inspectionResult"]["indexStatusResult"]["verdict"] == "PASS"
    assert results[1]["error"] and results[1]["result"] is None
    control("empty")
    assert request(root + "/performance", "POST", query)["rows"] == []
    control("quota")
    request(root + "/performance", "POST", query, 503)
    control("forbidden")
    request(root + "/performance", "POST", query, 403)
    control("revoked")
    request(root + "/performance", "POST", query, 409)
    control("normal")
    print("PASS GSC data: measured metrics, pagination, bounds, CSV injection protection, partial inspection, quota and revoked access", flush=True)

    def tool(name, args):
        req = urllib.request.Request(origin + "/mcp", method="POST", headers={"Content-Type": "application/json", "Accept": "application/json, text/event-stream", "Authorization": "Bearer " + key_row["secret"], "MCP-Protocol-Version": "2025-06-18"}, data=json.dumps({"jsonrpc": "2.0", "id": str(uuid.uuid4()), "method": "tools/call", "params": {"name": name, "arguments": args}}).encode())
        data = json.load(urllib.request.urlopen(req, timeout=60))
        payload = json.loads(data["result"]["content"][0]["text"])
        assert payload["ok"], payload
        return payload["data"]

    ref = {"project_id": projects[0]["id"]}
    assert len(tool("list_search_console_sites", ref)) == 2
    assert tool("set_search_console_site", {**ref, "site_url": "https://example.org/"})["site_url"] == "https://example.org/"
    assert tool("get_search_console_performance", {**ref, **query})["rows"][0]["clicks"] == 12
    assert tool("inspect_urls", {**ref, "urls": ["https://example.org/"]})[0]["result"]
    print("PASS GSC MCP: all four tools over authenticated HTTP", flush=True)

    # A second project owns its own grant. Disconnect removes pending state too.
    request(oauth_callback(other), expected=303)
    pending = oauth_callback(root)
    request(root, "DELETE", expected=204)
    request(pending, expected=422)
    assert not request(root)["connected"] and request(other)["connected"]
    request(root + "/performance", "POST", query, 409)
    print("PASS GSC disconnect: grant and pending state removed; other project unaffected", flush=True)
finally:
    control("normal")
    request("/api-keys/" + key_row["key"]["id"], "DELETE", expected=204)
    if member_id:
        request("/team/members/" + member_id, "DELETE", expected=204)
    for p in projects:
        request("/projects/" + p["id"] + "/archive", "POST")
        request("/projects/" + p["id"], "DELETE", expected=204)
