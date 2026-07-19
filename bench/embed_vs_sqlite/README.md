# embed_vs_sqlite — product-shaped f4kvs vs SQLite bench

Harness: memoirs + RAG chunks (put / batch / prefix scan / random get) across f4kvs WAL profiles and SQLite WAL.

## Run (micro)

```bash
# From repo root — no OUT needed; creates a versioned DE run automatically
CHUNKS=2000 ./scripts/bench_embed_vs_sqlite.sh
```

Env knobs: `MEMOIRS`, `CHUNKS`, `MEMOIR_BYTES`, `CHUNK_BYTES`, `RANDOM_GETS`, `SEED`, `INCLUDE_RELAXED`, `BENCH_RUNS_ROOT`, `OUT` (legacy flat JSON).

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
