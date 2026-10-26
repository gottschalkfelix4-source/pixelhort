#!/usr/bin/env bash
# Run cargo inside the official Rust Docker image (no local toolchain needed).
# Usage: scripts/cargo.sh check            (main workspace)
#        PH_CRATE_DIR=crates/ml scripts/cargo.sh build --release
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && { pwd -W 2>/dev/null || pwd; })"
DIR="${PH_CRATE_DIR:-.}"
VOL="pixelhort-target-$(echo "$DIR" | tr '/.' '__')"
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$ROOT":/work \
  -v pixelhort-cargo-registry:/usr/local/cargo/registry \
  -v pixelhort-cargo-git:/usr/local/cargo/git \
  -v "$VOL":/work/$DIR/target \
  -w /work/$DIR \
  ${PH_DOCKER_ARGS:-} \
  rust:1-bookworm cargo "$@"
