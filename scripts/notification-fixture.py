"""Local-only webhook receiver used by M7 acceptance."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading

calls = []
lock = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.path.startswith("/places/"):
            with lock:
                calls.append({"path": self.path, "field_mask": self.headers.get("X-Goog-FieldMask")})
            body = json.dumps({"id": self.path.rsplit("/", 1)[-1], "displayName": {"text": "Fixture Business"}, "formattedAddress": "Fixture Street", "rating": 4.6, "userRatingCount": 12, "googleMapsUri": "https://maps.google.com/", "reviews": [{"text": {"text": "Synthetic review"}, "rating": 5, "authorAttribution": {"displayName": "Fixture author", "uri": "https://maps.google.com/"}}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path.startswith("/beacon"):
            with lock:
                calls.append({"path": self.path})
        with lock:
            body = json.dumps(calls).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        with lock:
            calls.append({"path": self.path, "delivery_id": self.headers.get("X-SeoSeo-Delivery-Id"), "data": json.loads(body)})
        if self.path == "/drop":
            self.close_connection = True
            return
        self.send_response(302 if self.path == "/redirect" else 429 if self.path == "/fail" else 204)
        if self.path == "/redirect":
            self.send_header("Location", "http://receiver:8080/target")
        self.end_headers()


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
