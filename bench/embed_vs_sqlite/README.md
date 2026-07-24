# embed_vs_sqlite — product-shaped f4kvs vs SQLite bench

Harness: memoirs + RAG chunks (put / batch / prefix scan / random get) across f4kvs WAL profiles and SQLite WAL.

## Run (micro)

```bash
# From repo root — no OUT needed; creates a versioned DE run automatically
CHUNKS=2000 ./scripts/bench_embed_vs_sqlite.sh
```

## Run (meso — product path, 100k chunks)

```bash
# Defaults when TIER=meso: CHUNKS=100000, RANDOM_GETS=5000, PROFILES=product, INCLUDE_RELAXED=false
TIER=meso ./scripts/bench_embed_vs_sqlite.sh
```

`PROFILES=product` = `f4kvs_wal_segment` + `sqlite_wal_full` (+ post_restart integrity).  

`PROFILES=durable_compare` = segment + **group_commit_1ms** + SQLite FULL + SQLite NORMAL.

### Durability (honest)

| Profile | When the API returns | Crash mid long ingest |
|---------|----------------------|------------------------|
| **f4kvs_wal_segment** + BatchPut(500) | That 500-key batch is fsync’d | Lose at most the incomplete batch (~500 keys) |
| **f4kvs_group_commit_1ms** + Put stream | Put acks after group fsync (≤1 ms window) | Lose at most one unflushed group window |
| **sqlite_wal_full** + one big tx | Whole txn durable on `COMMIT` | Lose **all** keys of the open txn if crash before commit |
| **sqlite_wal_normal** | Weaker (SQLite may delay fsync) | Larger tail-loss window — **not** durability-matched |

SQLite `synchronous=FULL` is **not** “less secure” than f4kvs in the cryptographic sense — it is industry-grade durable commit. The difference is **checkpoint granularity** during a multi-second ingest, not “SQLite is unsafe.”

**Scale note:** above `max-per-commit-chunks` (default **10 000**), the harness skips per-commit `PutBytes` loops and uses chunked `BatchPutBytes` (slices of **500**, `SetBulkImport`) / batched SQLite tx.

**WAL rotation deadlock (fixed in f4kvs-lsm):** meso **100 000 × 4 KB** used to stall ~13–15 k keys when the 64 MiB segment filled — `batch_write_entries` called `rotate_segment` while still holding the segment write guard (`let _ = reborrow` did not drop the `RwLockWriteGuard`). Fixed by dropping the guard before rotate (same pattern as frame WAL). Unit: `batch_write_rotates_when_segment_full_no_deadlock`. Evidence: `runs/20260724T024413Z` integrity_ok=1 at 100 050 rows (needs lsm with the fix — path dep or tag ≥ post-fix / v0.3.1).

Full durability matrix (slow at 100k — overnight lab; may need `-max-per-commit-chunks=0` carefully):

```bash
TIER=meso PROFILES=all ./scripts/bench_embed_vs_sqlite.sh
# or explicit:
CHUNKS=100000 PROFILES=f4kvs_wal_segment,f4kvs_group_commit_10ms,sqlite_wal_full ./scripts/bench_embed_vs_sqlite.sh
```

Env knobs: `MEMOIRS`, `CHUNKS`, `MEMOIR_BYTES`, `CHUNK_BYTES`, `RANDOM_GETS`, `SEED`, `INCLUDE_RELAXED`, `PROFILES` (`all`|`product`|`durable_compare`|comma-list), `TIER`, `BATCH_PUT_SIZE`, `FAIR`, `BENCH_RUNS_ROOT`, `OUT` (legacy flat JSON).

## Fair vs SQLite (same durable unit size)

SQLite meso batched path uses **one transaction → one COMMIT (~one fsync)** for all keys.

Default f4kvs meso uses `BATCH_PUT_SIZE=500` → **N/500 BatchPuts** (many durable units) — **not fair**.

```bash
# Fair: one BatchPut for all keys + engine max_batch_size raised (OpenOptions)
FAIR=1 TIER=meso PROFILES=product ./scripts/bench_embed_vs_sqlite.sh
# equivalent:
BATCH_PUT_SIZE=0 TIER=meso PROFILES=f4kvs_wal_segment,sqlite_wal_full ./scripts/bench_embed_vs_sqlite.sh

# Closer-but-not-one-shot (engine default cap 10k without MaxBatchSize):
BATCH_PUT_SIZE=10000 TIER=meso PROFILES=product ./scripts/bench_embed_vs_sqlite.sh
```

| Setting | f4kvs durable units | SQLite durable units |
|---------|---------------------|----------------------|
| `BATCH_PUT_SIZE=500` (default) | ~N/500 | 1 |
| `BATCH_PUT_SIZE=10000` | ~N/10000 | 1 |
| `FAIR=1` / `BATCH_PUT_SIZE=0` | **1** | **1** |

## How to read a run

Each invocation writes:

```
runs/{run_id}/
  manifest.json      # scale, host, git shas, seed, engines, tier
  results.jsonl      # long format: one metric per line
  report.legacy.json # optional flat report (same as -out)
```

- **`run_id`**: UTC compact ISO (`20260718T102530Z`).
- **Join**: `results.jsonl.run_id` → `manifest.run_id`.
- **Long format**: one line = `(engine, phase, metric, value)`. A throughput phase emits `duration_ms` + `ops_per_s`.
- **Integrity**: phase `post_restart_row_count` with `integrity_ok` (1=pass). Fail → non-zero exit.

```bash
# Scale / git
jq .scale,.git runs/*/manifest.json

# Throughput (ms)
jq -c 'select(.metric=="duration_ms")' runs/*/results.jsonl | head

# Restart gate
jq -c 'select(.phase=="post_restart_row_count")' runs/*/results.jsonl
```

Schema id: `bench-schema-v1` (`manifest.schema_version`). Parquet conversion is optional (étape 1b).

## Legacy JSON

```bash
OUT=/tmp/report.json ./scripts/bench_embed_vs_sqlite.sh
# also written under the run dir as report.legacy.json
```
