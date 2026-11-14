#!/usr/bin/env python3
"""Offline Maps/AIO/Nominatim/CrUX fixtures; no outbound requests."""
import importlib.util
import json
import time
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Lock
from urllib.parse import parse_qs, urlsplit

counts = Counter()
lock = Lock()
spec = importlib.util.spec_from_file_location("bing_fixture", Path(__file__).with_name("bing-webmaster-fixture.py"))
bing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bing)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def send_json(self, status, data):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urlsplit(self.path)
        q = parse_qs(parsed.query)
        if parsed.path == "/counts":
            return self.send_json(200, dict(counts))
        if parsed.path.startswith("/bing/"):
            return bing.Handler.do_GET(self)
        if parsed.path == "/nominatim":
            return self.send_json(200, [{"osm_type": "node", "osm_id": 123, "name": "Fixture Cafe",
                "display_name": "Fixture Cafe, Jakarta", "lat": "-6.2", "lon": "106.8", "type": "cafe"}])
        if q.get("api_key") != ["fixture-serpapi-key"]:
            return self.send_json(403, {"error": "do-not-leak-provider-body"})
        engine = q.get("engine", [""])[0]
        keyword = q.get("q", [""])[0]
        with lock:
            counts[engine] += 1
            counts["q:" + keyword] += 1
        if keyword == "blocked":
            return self.send_json(429, {"error": "do-not-leak-provider-body"})
        if keyword == "interrupted":
            time.sleep(3)
        if engine == "google_maps":
            assert q["ll"][0].startswith("@") and q["hl"] and q["gl"]
            return self.send_json(200, {"local_results": [
                {"position": 1, "place_id": "competitor", "title": "Nearby Coffee", "address": "Fixture Street"},
                {"position": 2, "place_id": "target", "title": "Fixture Cafe", "website": "https://example.com/"}]})
        if engine == "google_ai_overview":
            assert q["page_token"] == ["fixture-token"]
            return self.send_json(200, {"ai_overview": {"text_blocks": [{"snippet": "Captured fixture overview after continuation."}],
                "references": [{"title": "Fixture source", "link": "https://example.com/source"}]}})
        if "absent" in keyword:
            return self.send_json(200, {"search_metadata": {"status": "Success"}, "organic_results": []})
        if "token" in keyword:
            return self.send_json(200, {"ai_overview": {"page_token": "fixture-token"}})
        return self.send_json(200, {"search_metadata": {"status": "Success"}, "ai_overview": {
            "text_blocks": [{"snippet": "Observed fixture Google AI Overview."}],
            "references": [{"title": "Fixture source", "link": "https://example.com/source"},
                           {"title": "Unsafe source", "link": "javascript:alert(1)"}]}})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        print(json.dumps(body), flush=True)  # CrUX regression inspects only safe submitted targets.
        target = body.get("url", body.get("origin", ""))
        if "no-data" in target:
            return self.send_json(404, {"error": {"status": "NOT_FOUND"}})
        if "quota" in target:
            return self.send_json(429, {"error": "do-not-leak-provider-body"})
        if "invalid" in target:
            return self.send_json(200, {"invalid": True})
        key = {"url" if "url" in body else "origin": target}
        if "formFactor" in body:
            key["formFactor"] = body["formFactor"]
        self.send_json(200, {"record": {"key": key,
            "metrics": {"largest_contentful_paint": {"percentiles": {"p75": 2400}},
                        "cumulative_layout_shift": {"percentiles": {"p75": "0.12"}}},
            "collectionPeriod": {"firstDate": {"year": 2026, "month": 10, "day": 1},
                                 "lastDate": {"year": 2026, "month": 10, "day": 28}}}})


if __name__ == "__main__":
    ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
