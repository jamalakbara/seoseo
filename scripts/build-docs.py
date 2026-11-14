#!/usr/bin/env python3
"""Package canonical Markdown for the self-contained public documentation site."""
import json
import sys
from pathlib import Path
root = Path(__file__).resolve().parents[1]
docs = [{"slug": p.stem, "title": p.read_text(encoding="utf-8").splitlines()[0].lstrip("# "), "markdown": p.read_text(encoding="utf-8")} for p in sorted((root / "docs").glob("*.md"))]
output = json.dumps(docs, ensure_ascii=False, indent=2) + "\n"
target = root / "frontend/lib/docs.generated.json"
if "--check" in sys.argv:
    assert target.read_text(encoding="utf-8") == output, "Run python scripts/build-docs.py"
else:
    target.write_text(output, encoding="utf-8", newline="\n")
print(f"Packaged {len(docs)} documentation pages")
