#!/usr/bin/env bash
# Product-shaped f4kvs-ffi vs SQLite benchmark.
# Default: DE run layout under bench/embed_vs_sqlite/runs/{run_id}/
# Optional: OUT=path.json also writes legacy flat report (and report.legacy.json in run dir).
# See bench/embed_vs_sqlite/README.md
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
RANDOM_GETS="${RANDOM_GETS:-500}"
INCLUDE_RELAXED="${INCLUDE_RELAXED:-true}"
SEED="${SEED:-42}"
TIER="${TIER:-}"
# all | product | durable_compare | comma-separated (see bench/embed_vs_sqlite/README.md)
PROFILES="${PROFILES:-all}"
# BatchPut slice size: 500 default; 0 = one-shot all keys (fair vs SQLite one COMMIT)
BATCH_PUT_SIZE="${BATCH_PUT_SIZE:-500}"
# FAIR=1 → batch-put-size=0 + engine max_batch raised (parity with SQLite single txn)
FAIR="${FAIR:-0}"
# DE runs root: env override, default next to harness (gitignored)
BENCH_RUNS_ROOT="${BENCH_RUNS_ROOT:-$BENCH_DIR/runs}"
OUT="${OUT:-}"

# Convenience: TIER=meso fills product-shaped defaults unless already overridden.
if [[ "${TIER}" == "meso" ]]; then
  if [[ "${CHUNKS}" == "2000" ]]; then CHUNKS=100000; fi
  if [[ "${RANDOM_GETS}" == "500" ]]; then RANDOM_GETS=5000; fi
  if [[ "${PROFILES}" == "all" ]]; then PROFILES=product; fi
  if [[ "${INCLUDE_RELAXED}" == "true" ]]; then INCLUDE_RELAXED=false; fi
fi

mkdir -p "$BENCH_RUNS_ROOT"

ARGS=(
  -memoirs="$MEMOIRS"
  -chunks="$CHUNKS"
  -memoir-bytes="$MEMOIR_BYTES"
  -chunk-bytes="$CHUNK_BYTES"
  -random-gets="$RANDOM_GETS"
  -seed="$SEED"
  -runs-root="$BENCH_RUNS_ROOT"
  -profiles="$PROFILES"
  -batch-put-size="$BATCH_PUT_SIZE"
)
if [[ -n "$TIER" ]]; then
  ARGS+=(-tier="$TIER")
fi
if [[ "$INCLUDE_RELAXED" == "false" ]]; then
  ARGS+=(-include-relaxed=false)
fi
if [[ "$FAIR" == "1" || "$FAIR" == "true" ]]; then
  ARGS+=(-fair)
fi
if [[ -n "$OUT" ]]; then
  ARGS+=(-out="$OUT")
fi

echo "BENCH_RUNS_ROOT=$BENCH_RUNS_ROOT PROFILES=$PROFILES CHUNKS=$CHUNKS TIER=${TIER:-auto} BATCH_PUT_SIZE=$BATCH_PUT_SIZE FAIR=$FAIR" >&2
go run . "${ARGS[@]}"
