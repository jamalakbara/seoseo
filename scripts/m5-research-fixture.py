#!/usr/bin/env python3
"""Local SERP/source-page fixture. Use only on the isolated M5 test network."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from urllib.parse import urlparse

mode = "baseline"
requests = []


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def send(self, body, status=200, mime="text/html", location=None):
        if not isinstance(body, bytes):
            body = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", mime)
        if location:
            self.send_header("Location", location)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        global mode
        data = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        mode = data["mode"]
        requests.clear()
        self.send(json.dumps({"mode": mode}), mime="application/json")

    def do_GET(self):
        host = self.headers.get("Host", "").split(":")[0]
        path = urlparse(self.path).path
        if path == "/state":
            self.send(json.dumps({"mode": mode, "requests": requests}), mime="application/json")
            return
        requests.append([host, path])
        if path == "/search":
            self.send(json.dumps({"results": [{"url": "http://source-one.example/page", "title": "One", "content": "Reference"}, {"url": "http://source-two.example/page", "title": "Two", "content": "Reference"}, {"url": "http://robots-source.example/page", "title": "Blocked", "content": "Reference"}]}), mime="application/json")
            return
        if path == "/robots.txt":
            body = "User-agent: *\nDisallow: /page\n" if host == "robots-source.example" else "User-agent: *\nAllow: /\n"
            self.send(body, mime="text/plain")
            return
        if host == "source-two.example" and mode == "blocked":
            self.send("Access denied", 403)
            return
        if host == "source-two.example" and mode == "redirect":
            self.send("", 302, location="http://127.0.0.1/private")
            return
        if host == "source-two.example" and mode == "missing":
            self.send("Missing", 404)
            return
        if host in ("source-one.example", "source-two.example") and mode == "over_limit":
            self.send("<html>" + "".join(f"<a href='https://example.com/many/{i}'>Link {i}</a>" for i in range(600)) + "</html>")
            return
        if host == "source-one.example":
            body = "<title>Publisher one</title><p>Source text</p><a href='https://example.com/one'>Acme</a>"
            if mode == "baseline":
                body += "<a href='https://example.com/removed' rel='nofollow'>Second</a>"
        else:
            body = "<title>Publisher two</title><a href='https://example.com/two'>Acme</a>"
        self.send("<html>" + body + "</html>")


ThreadingHTTPServer(("0.0.0.0", 80), Handler).serve_forever()
