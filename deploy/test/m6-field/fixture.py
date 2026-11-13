"""Local-only CrUX contract fixture. No real credentials or upstream requests."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass  # Request URLs contain API keys.

    def do_POST(self):
        q = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        target = q.get("url", q.get("origin"))
        print(json.dumps(q), flush=True)  # Fixture body only; never key query.
        status = 200
        if "no-data" in target:
            status, result = 404, {"error": {"status": "NOT_FOUND"}}
        elif "quota" in target:
            status, result = 429, {"error": {"message": "do-not-leak-provider-body"}}
        elif "invalid" in target:
            result = {"invalid": True}
        else:
            key = {"url" if "url" in q else "origin": target}
            if "formFactor" in q:
                key["formFactor"] = q["formFactor"]
            result = {"record": {"key": key, "metrics": {
                "largest_contentful_paint": {"percentiles": {"p75": 2400}},
                "cumulative_layout_shift": {"percentiles": {"p75": "0.12"}},
            }, "collectionPeriod": {
                "firstDate": {"year": 2026, "month": 10, "day": 1},
                "lastDate": {"year": 2026, "month": 10, "day": 28},
            }}}
        body = json.dumps(result).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
