#!/usr/bin/env python3
"""M4 memory/reports regression against a running AUTH_MODE=none API."""

from __future__ import annotations

import argparse
import json
import urllib.error
import urllib.request


def request(base: str, method: str, path: str, body: dict | None = None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(
        base + path,
        data=data,
        method=method,
        headers={"content-type": "application/json", "x-seoseo-csrf": "1"},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            raw = response.read()
            content_type = response.headers.get_content_type()
            parsed = json.loads(raw) if raw and content_type == "application/json" else raw
            return response.status, response.headers, parsed
    except urllib.error.HTTPError as error:
        raw = error.read()
        return error.code, error.headers, json.loads(raw) if raw else None


def expect_status(actual: int, expected: int, step: str) -> None:
    if actual != expected:
        raise AssertionError(f"{step}: expected HTTP {expected}, got {actual}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--api", default="http://127.0.0.1:8080/api/v1")
    args = parser.parse_args()
    base = args.api.rstrip("/")

    status, _, project = request(
        base, "POST", "/projects", {"name": "M4 e2e", "domain": "example.com"}
    )
    expect_status(status, 201, "create project")
    project_id = project["id"]

    try:
        context_body = {
            "revision": 0,
            "sections": [
                {"key": "positioning", "title": "Positioning", "content": "Free SEO"}
            ],
            "competitors": [{"domain": "competitor.com", "name": None, "notes": None}],
            "key_pages": [
                {"url": "https://example.com/service", "role": "money", "topic": "Service"}
            ],
            "research_note": "Initial research",
        }
        status, _, context = request(base, "PUT", f"/projects/{project_id}/context", context_body)
        expect_status(status, 200, "save context")
        assert context["revision"] == 1 and len(context["research_log"]) == 1

        status, _, problem = request(base, "PUT", f"/projects/{project_id}/context", context_body)
        expect_status(status, 409, "reject stale context")
        assert problem["type"] == "conflict"

        status, _, template = request(
            base,
            "POST",
            f"/projects/{project_id}/report-templates",
            {"name": "Monthly", "description": None, "instructions": "Summarize progress."},
        )
        expect_status(status, 201, "create template")
        template_id = template["id"]
        status, _, template = request(
            base,
            "PUT",
            f"/projects/{project_id}/report-templates/{template_id}",
            {"name": "Monthly update", "description": "Client brief", "instructions": "Show progress."},
        )
        expect_status(status, 200, "update template")
        assert template["name"] == "Monthly update"

        report_input = {
            "title": "M4 report",
            "summary": "Regression",
            "html": "<!doctype html><html><body><h1>M4 works</h1></body></html>",
            "skill": "seo-report",
            "template_id": template_id,
        }
        status, _, report = request(base, "POST", f"/projects/{project_id}/reports", report_input)
        expect_status(status, 201, "create report")
        report_id = report["id"]

        status, _, share = request(
            base, "POST", f"/projects/{project_id}/reports/{report_id}/share"
        )
        expect_status(status, 200, "share report")
        token = share["token"]
        assert len(token) == 32

        status, _, public = request(base, "GET", f"/shared-reports/{token}")
        expect_status(status, 200, "public report metadata")
        assert public["title"] == "M4 report"
        assert "html" not in public and "project_id" not in public

        status, headers, _ = request(base, "GET", f"/shared-reports/{token}/raw")
        expect_status(status, 200, "public report HTML")
        assert "sandbox" in headers["content-security-policy"]
        assert headers["x-content-type-options"] == "nosniff"

        report_input["title"] = "Edited report"
        status, _, _ = request(
            base, "PUT", f"/projects/{project_id}/reports/{report_id}", report_input
        )
        expect_status(status, 200, "edit report")
        status, _, _ = request(base, "GET", f"/shared-reports/{token}")
        expect_status(status, 404, "edit revokes sharing")
    finally:
        request(base, "POST", f"/projects/{project_id}/archive")
        request(base, "DELETE", f"/projects/{project_id}")

    print("M4 memory/reports e2e passed")


if __name__ == "__main__":
    main()
