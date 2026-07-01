#!/usr/bin/env bash
# Measure FFI read tax: Go CGO GetBytes vs native Rust get on the same store.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "Building f4kvs-ffi (release)..."
cargo build -p f4kvs-ffi --release --example read_tax

BENCH_DIR="$ROOT/bench/ffi_read_tax"
cd "$BENCH_DIR"

export CGO_ENABLED=1
export CGO_CFLAGS="-I${ROOT}/crates/f4kvs-ffi/include"
export CGO_LDFLAGS="-L${ROOT}/target/release -lf4kvs_ffi -Wl,-rpath,${ROOT}/target/release"

STORE_DIR="$(mktemp -d /tmp/f4kvs-read-tax-XXXXXX)"
trap 'rm -rf "$STORE_DIR"' EXIT

CHUNKS="${CHUNKS:-2000}"
RANDOM_GETS="${RANDOM_GETS:-500}"
BULK_IMPORT="${BULK_IMPORT:-false}"

PREP_ARGS=(
  -dir="$STORE_DIR"
  -chunks="$CHUNKS"
  -prepare-only=true
)
if [[ "$BULK_IMPORT" == "true" ]]; then
  PREP_ARGS+=(-bulk-import=true)
fi

echo "=== Phase 1: ingest (BatchPutBytes) ==="
PREP_OUT="$(go run . "${PREP_ARGS[@]}")"
echo "$PREP_OUT"
INGEST_MS="$(echo "$PREP_OUT" | awk -F= '/^ingest_ms=/{print $2; exit}')"

echo ""
echo "=== Phase 2: native Rust reads (reopened store) ==="
NATIVE_OUT="$("${ROOT}/target/release/examples/read_tax" \
  --dir "$STORE_DIR" \
  --chunks "$CHUNKS" \
  --random-gets "$RANDOM_GETS")"
echo "$NATIVE_OUT"
NATIVE_MS="$(echo "$NATIVE_OUT" | awk -F'[= ]' '/^native_read_ms=/{print $2; exit}')"

echo ""
echo "=== Phase 3: Go FFI reads (reopened store) ==="
FFI_OUT="$(go run . -dir="$STORE_DIR" -chunks="$CHUNKS" -random-gets="$RANDOM_GETS" -read-only=true)"
echo "$FFI_OUT"
FFI_MS="$(echo "$FFI_OUT" | awk -F'[= ]' '/^ffi_read_ms=/{print $2; exit}')"

echo ""
echo "=== FFI read tax summary ==="
printf "ingest_ms=%s chunks=%s bulk_import=%s\n" "$INGEST_MS" "$CHUNKS" "$BULK_IMPORT"
printf "ffi_read_ms=%s native_read_ms=%s\n" "$FFI_MS" "$NATIVE_MS"

python3 - "$FFI_MS" "$NATIVE_MS" <<'PY'
import sys
ffi = float(sys.argv[1])
native = float(sys.argv[2])
if native > 0 and ffi > 0:
    ratio = ffi / native
    if ratio > 1.05:
        print(f"ffi_tax={ratio:.2f}x (Go CGO slower than native)")
    elif ratio < 0.95:
        print(f"ffi_tax={ratio:.2f}x (Go CGO faster — likely sync runtime reuse vs per-get block_on)")
    else:
        print(f"ffi_tax={ratio:.2f}x (parity)")
PY