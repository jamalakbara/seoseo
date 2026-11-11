"""Offline OAuth + Search Console fixture. Never use in production."""
import base64
import hashlib
import json
import secrets
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote, urlencode, urlsplit

codes = {}
refresh_tokens = set()
mode = "normal"
scope = "https://www.googleapis.com/auth/webmasters.readonly"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass  # OAuth codes/tokens must not enter logs.

    def reply(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urlsplit(self.path)
        if url.path == "/authorize":
            q = {k: v[0] for k, v in parse_qs(url.query).items()}
            if q.get("scope") != scope or q.get("code_challenge_method") != "S256" or q.get("access_type") != "offline":
                return self.reply({"error": "invalid_request"}, 400)
            code = secrets.token_urlsafe(24)
            codes[code] = q
            self.send_response(302)
            self.send_header("Location", q["redirect_uri"] + "?" + urlencode({"state": q["state"], "code": code}))
            self.end_headers()
            return
        if url.path == "/webmasters/v3/sites":
            return self.reply({"siteEntry": [
                {"siteUrl": "sc-domain:example.org", "permissionLevel": "siteOwner"},
                {"siteUrl": "https://example.org/", "permissionLevel": "siteFullUser"},
                {"siteUrl": "sc-domain:unverified.org", "permissionLevel": "siteUnverifiedUser"},
            ]})
        self.reply({"error": "not_found"}, 404)

    def do_POST(self):
        global mode
        raw = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if self.path == "/control":
            mode = json.loads(raw)["mode"]
            return self.reply({"ok": True})
        if self.path == "/token":
            q = {k: v[0] for k, v in parse_qs(raw.decode()).items()}
            if q.get("client_id") != "fixture-client" or q.get("client_secret") != "fixture-secret":
                return self.reply({"error": "invalid_client"}, 400)
            if q.get("grant_type") == "authorization_code":
                auth = codes.pop(q.get("code"), None)
                challenge = base64.urlsafe_b64encode(hashlib.sha256(q.get("code_verifier", "").encode()).digest()).decode().rstrip("=")
                if not auth or challenge != auth["code_challenge"] or q.get("redirect_uri") != auth["redirect_uri"]:
                    return self.reply({"error": "invalid_grant"}, 400)
                refresh = secrets.token_urlsafe(32)
                refresh_tokens.add(refresh)
                return self.reply({"access_token": "fixture-access", "refresh_token": refresh, "scope": scope, "expires_in": 3600})
            if q.get("refresh_token") not in refresh_tokens or mode == "revoked":
                return self.reply({"error": "invalid_grant"}, 400)
            return self.reply({"access_token": "fixture-access", "expires_in": 3600})
        if self.headers.get("Authorization") != "Bearer fixture-access":
            return self.reply({"error": "unauthorized"}, 401)
        if mode == "quota":
            return self.reply({"error": "quota"}, 429)
        if mode == "forbidden":
            return self.reply({"error": "forbidden"}, 403)
        q = json.loads(raw)
        if self.path.endswith("/searchAnalytics/query"):
            site = unquote(self.path.removeprefix("/webmasters/v3/sites/").removesuffix("/searchAnalytics/query"))
            if site not in ("sc-domain:example.org", "https://example.org/") or q.get("dataState") != "final":
                return self.reply({"error": "invalid_query"}, 400)
            if mode == "empty" or q.get("startRow", 0) > 0:
                return self.reply({})
            values = {"query": "=test formula", "page": "https://example.org/", "device": "DESKTOP", "country": "usa", "date": q["startDate"], "searchAppearance": "RICH_RESULT"}
            return self.reply({"rows": [{"keys": [values[d] for d in q.get("dimensions", [])], "clicks": 12, "impressions": 100, "ctr": .12, "position": 3.5}]})
        if self.path == "/inspect":
            if "invalid" in q["inspectionUrl"]:
                return self.reply({"error": "invalid_url"}, 400)
            return self.reply({"inspectionResult": {"indexStatusResult": {"verdict": "PASS", "coverageState": "Submitted and indexed", "googleCanonical": q["inspectionUrl"]}}})
        self.reply({"error": "not_found"}, 404)


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
