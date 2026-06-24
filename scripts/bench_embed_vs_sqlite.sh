#!/usr/bin/env bash
# Product-shaped f4kvs-ffi vs SQLite benchmark.
# See docs/f4kvs-sqlite-benchmark.md in projects-tracker (or bench output JSON).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "Building f4kvs-ffi (release)..."
cargo build -p f4kvs-ffi --release

BENCH_DIR="$ROOT/bench/embed_vs_sqlite"
cd "$BENCH_DIR"

export CGO_ENABLED=1
export CGO_CFLAGS="-I${ROOT}/crates/f4kvs-ffi/include"
export CGO_LDFLAGS="-L${ROOT}/target/release -lf4kvs_ffi -Wl,-rpath,${ROOT}/target/release"

MEMOIRS="${MEMOIRS:-50}"
CHUNKS="${CHUNKS:-2000}"
MEMOIR_BYTES="${MEMOIR_BYTES:-200000}"
CHUNK_BYTES="${CHUNK_BYTES:-4096}"
RANDOM_GETS="${RANDOM_GETS:-1000}"
OUT="${OUT:-}"

ARGS=(
  -memoirs="$MEMOIRS"
  -chunks="$CHUNKS"
  -memoir-bytes="$MEMOIR_BYTES"
  -chunk-bytes="$CHUNK_BYTES"
  -random-gets="$RANDOM_GETS"
)
if [[ -n "$OUT" ]]; then
  ARGS+=(-out="$OUT")
fi

go run . "${ARGS[@]}"