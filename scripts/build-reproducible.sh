#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${REPRO_IMAGE:-ourdao-reproducible:rust-1.98.1-stellar-28.0.0}"
OUT_REL="target/reproducible-container"
OUT_DIR="$ROOT_DIR/$OUT_REL"
EXPECTED_WASM="${1:-}"

command -v docker >/dev/null 2>&1 || {
  echo "error: docker is required for reproducible builds" >&2
  exit 1
}

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "Building pinned reproducible image $IMAGE"
docker build \
  --file "$ROOT_DIR/Dockerfile.reproducible" \
  --tag "$IMAGE" \
  "$ROOT_DIR"

echo "Building ourdao-dao with $IMAGE"
docker run --rm \
  --volume "$ROOT_DIR:/workspace" \
  --workdir /workspace \
  "$IMAGE" \
  contract build \
  --locked \
  --package ourdao-dao \
  --optimize=false \
  --out-dir "/workspace/$OUT_REL"
CONTAINER_WASM="$(find "$OUT_DIR" -maxdepth 1 -type f -name '*.wasm' | sort | head -n 1)"
if [[ -z "$CONTAINER_WASM" ]]; then
  echo "error: container build produced no wasm in $OUT_DIR" >&2
  exit 1
fi

CONTAINER_HASH="$(sha256sum "$CONTAINER_WASM" | awk '{print $1}')"
echo "container wasm: $CONTAINER_WASM"
echo "container sha256: $CONTAINER_HASH"

if [[ -n "$EXPECTED_WASM" ]]; then
  if [[ ! -f "$EXPECTED_WASM" ]]; then
    echo "error: expected wasm not found: $EXPECTED_WASM" >&2
    exit 1
  fi
  EXPECTED_HASH="$(sha256sum "$EXPECTED_WASM" | awk '{print $1}')"
  echo "expected wasm: $EXPECTED_WASM"
  echo "expected sha256: $EXPECTED_HASH"
  if [[ "$CONTAINER_HASH" != "$EXPECTED_HASH" ]]; then
    echo "error: reproducible-build hash mismatch" >&2
    exit 1
  fi
  echo "reproducible build verified"
fi