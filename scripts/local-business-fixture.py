#!/usr/bin/env python3
"""Offline Nominatim fixture; never contacts a public geocoder."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        q = parse_qs(urlsplit(self.path).query)
        assert q["format"] == ["jsonv2"] and q["countrycodes"]
        data = [{"osm_type": "node", "osm_id": 123, "name": "Fixture Cafe",
                 "display_name": "Fixture Cafe, Jakarta", "lat": "-6.2", "lon": "106.8",
                 "type": "cafe", "extratags": {"website": "javascript:alert(1)"}}]
        raw = json.dumps(data).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
