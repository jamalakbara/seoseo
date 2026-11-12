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
        native = "anthropic" if self.path == "/v1/messages" else "gemini" if self.path.startswith("/v1beta/models/") else None
        if self.path != "/v1/chat/completions" and native is None:
            self.reply({"error": "unexpected endpoint"}, 404)
            return
        model = data.get("model", self.path.split("/models/")[-1].split(":generateContent")[0])
        prompt = data["contents"][0]["parts"][0]["text"] if native == "gemini" else data["messages"][-1]["content"]
        system = data["systemInstruction"]["parts"][0]["text"] if native == "gemini" else data.get("system", data.get("messages", [{}])[0].get("content", ""))
        if native == "anthropic" and (not self.headers.get("x-api-key") or data.get("tools", [{}])[0].get("max_uses") != 2):
            self.reply({"error": "invalid Anthropic contract"}, 400)
            return
        if native == "gemini" and (not self.headers.get("x-goog-api-key") or data.get("tools")):
            self.reply({"error": "invalid Gemini contract"}, 400)
            return
        with lock:
            calls.append({"model": model, "prompt": prompt, "system": system, "engine": native or "compatible"})
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
        if "Return ONLY a JSON array" in prompt:
            answer = json.dumps(["Which tools support technical SEO audits?", "How can a small team monitor search rankings?", "What matters when comparing SEO platforms?", "Which workflows help prioritize website issues?", "How can teams evaluate search performance?"])
        if native == "anthropic":
            self.reply({"stop_reason": "end_turn", "content": [{"type": "text", "text": answer, "citations": [{"type": "web_search_result_location", "url": "https://sources.example/anthropic", "title": "Anthropic fixture source"}]}]})
            return
        if native == "gemini":
            self.reply({"candidates": [{"finishReason": "STOP", "content": {"parts": [{"text": answer}]}} ]})
            return
        self.reply({"choices": [{"finish_reason": "stop", "message": {"content": answer}}]})


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
