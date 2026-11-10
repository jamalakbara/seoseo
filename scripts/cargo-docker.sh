#!/usr/bin/env bash
# Run cargo inside the official Rust image (no local toolchain needed).
# Usage: scripts/cargo-docker.sh check --workspace
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))"
MSYS_NO_PATHCONV=1 docker run --rm ${DOCKER_ARGS:-} \
  -v "$ROOT/backend:/src" \
  -v seoseo-cargo-registry:/usr/local/cargo/registry \
  -v seoseo-target:/src/target \
  -e DATABASE_URL="${DATABASE_URL:-}" \
  -w /src rust:1.99-slim-bookworm \
  bash -c "rustup component add clippy rustfmt >/dev/null 2>&1; cargo $*"
