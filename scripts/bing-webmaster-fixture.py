#!/usr/bin/env python3
"""Disposable Bing Webmaster JSON fixture for scripts/e2e-bing.py."""
import json
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        parsed = urllib.parse.urlsplit(self.path)
        query = urllib.parse.parse_qs(parsed.query)
        if query.get("apikey") != ["fixture-bing-key"]:
            return self.send_json(400, {"ErrorCode": 3, "Message": "InvalidApiKey do-not-leak-provider-body"})
        operation = parsed.path.rsplit("/", 1)[-1]
        if operation == "GetUserSites":
            data = [
                {"__type": "Site", "Url": "https://example.com/", "IsVerified": True,
                 "AuthenticationCode": "do-not-leak-auth-code", "DnsVerificationCode": "do-not-leak-dns-code"},
                {"__type": "Site", "Url": "https://unverified.example/", "IsVerified": False,
                 "AuthenticationCode": "do-not-leak-auth-code"},
            ]
        elif query.get("siteUrl") != ["https://example.com/"]:
            return self.send_json(400, {"ErrorCode": 6, "Message": "InvalidSite do-not-leak-provider-body"})
        elif operation == "GetQueryStats":
            data = [{"__type": "QueryStats", "Query": "seo fixture", "Clicks": 7,
                     "Impressions": 70, "AvgClickPosition": 3, "AvgImpressionPosition": 4,
                     "Date": "/Date(1793491200000+0000)/"}]
        elif operation == "GetPageStats":
            data = [{"__type": "QueryStats", "Query": "https://example.com/page", "Clicks": 4,
                     "Impressions": 40, "AvgClickPosition": 2, "AvgImpressionPosition": 3}]
        elif operation == "GetCrawlStats":
            data = [{"__type": "CrawlStats", "CrawledPages": 12, "InIndex": 10, "InLinks": 21}]
        elif operation == "GetLinkCounts":
            data = {"__type": "LinkCounts", "TotalPages": 2,
                    "Links": [{"__type": "LinkCount", "Url": "https://example.com/page", "Count": 21}],
                    "RequestedPage": int(query.get("page", [0])[0])}
        else:
            return self.send_json(404, {"Message": "Unknown operation"})
        self.send_json(200, {"d": data})

    def send_json(self, status, value):
        raw = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


ThreadingHTTPServer(("127.0.0.1", 3199), Handler).serve_forever()
