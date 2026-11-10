#!/usr/bin/env bash
# Regenerate frontend/openapi.json from the backend and the TypeScript types from it.
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/cargo-docker.sh "run -q -p app -- openapi" 2>/dev/null > frontend/openapi.json.new
python -c "import json,sys; json.load(open('frontend/openapi.json.new'))" && mv frontend/openapi.json.new frontend/openapi.json
(cd frontend && pnpm -s gen:api)
