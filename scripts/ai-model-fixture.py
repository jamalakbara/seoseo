#!/usr/bin/env python3
"""Offline compatible model API fixture for disposable AI visibility regressions."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading

calls = []
lock = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        with lock:
            self.reply({"calls": list(calls)})

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        if self.path == "/control":
            with lock:
                calls.clear()
            self.reply({"ok": True})
            return
        if self.path != "/v1/chat/completions":
            self.reply({"error": "unexpected endpoint"}, 404)
            return
        model = data["model"]
        prompt = data["messages"][-1]["content"]
        with lock:
            calls.append({"model": model, "prompt": prompt, "system": data["messages"][0]["content"]})
        if "FAIL" in prompt:
            self.reply({"error": "fixture rate limit"}, 429)
            return
        if "TRUNCATE" in prompt:
            self.reply({"choices": [{"finish_reason": "length", "message": {"content": "Acme"}}]})
            return
        if "REFUSE" in prompt:
            self.reply({"choices": [{"finish_reason": "stop", "message": {"content": ""}}]})
            return
        answer = {"fixture-a": "Acme and Rival are examples. [Reference](https://sources.example/guide).",
            "fixture-b": "Rival is one example. https://sources.example/guide",
            "fixture-c": "There are several available tools; investigate your requirements."}.get(model, "Acme")
        self.reply({"choices": [{"finish_reason": "stop", "message": {"content": answer}}]})


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
