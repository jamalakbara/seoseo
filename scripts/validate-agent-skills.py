#!/usr/bin/env python3
"""Validate the portable SeoSeo workflow package against the actual MCP tool surface."""
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
source = (root / "backend/crates/mcp/src/lib.rs").read_text(encoding="utf-8")
tools = set(re.findall(r"#\[tool\([\s\S]*?\)\]\s*async fn (\w+)", source))
skills = sorted((root / "plugins/seoseo/skills").glob("*/SKILL.md"))
assert len(skills) >= 14, "Expected twelve adapted workflows and at least two new workflows"
catalog = []
for path in skills:
    text = path.read_text(encoding="utf-8")
    assert text.startswith("---\n") and "\n---\n" in text[4:], path
    header = text.split("---\n", 2)[1]
    name = re.search(r"^name: ([a-z0-9-]+)$", header, re.M)
    description = re.search(r"^description: (.+)$", header, re.M)
    assert name and name[1] == path.parent.name and description, path
    desc = json.loads(description[1])
    assert len(desc) > 20 and "TODO" not in text, path
    used = set(re.findall(r"`([a-z][a-z0-9_]+)`", text))
    assert used <= tools, (path, "Unknown MCP tool references", sorted(used - tools))
    assert (path.parent / "agents/openai.yaml").exists(), path
    catalog.append({"name": name[1], "description": desc, "path": str(path.relative_to(root)).replace("\\", "/")})
print(f"PASS: {len(skills)} portable skills; all referenced tools exist among {len(tools)} MCP tools")
