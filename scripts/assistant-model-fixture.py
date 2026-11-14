#!/usr/bin/env python3
"""Offline JSON-action model fixture. Only for the disposable M7 regression stack."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading
import time

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
        prompt = data["messages"][-1]["content"]
        context = json.loads(prompt.split("INPUT JSON: ", 1)[1])
        with lock:
            calls.append({"model": data["model"], "context": context})
        message = context["message"]
        steps = context["previous_actions"]
        if "SLOW" in message:
            time.sleep(3)
        if "RATE_LIMIT" in message:
            self.reply({"error": "rate limited"}, 429)
            return
        if "MALFORMED" in message:
            answer = "not JSON"
        elif "FORGE_SCOPE" in message:
            answer = json.dumps({"type": "tool", "name": "audit_summary", "project_id": "other-project"})
        elif "LOOP" in message:
            answer = json.dumps({"type": "tool", "name": "audit_summary"})
        elif not steps:
            name = "cached_serp" if "content brief" in message else "save_memory" if "UNKNOWN_TOOL" in message else "audit_summary"
            answer = json.dumps({"type": "tool", "name": name, **({"keyword": "seo audits"} if name == "cached_serp" else {})})
        else:
            answer = json.dumps({"type": "final", "answer": "Fixture answer from project_memory and " + steps[-1]["name"] + ". Stored sample only; missing evidence remains unknown."})
        self.reply({"choices": [{"finish_reason": "stop", "message": {"content": answer}}]})


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
