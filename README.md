# f4kvs-ffi

Embedded key-value store with an on-disk LSM engine and a C FFI for language bindings (Go, etc.).

Licensed under [Apache 2.0](LICENSE-APACHE).

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Consumers: Go, C, other languages                      │
└──────────────────────────┬──────────────────────────────┘
                           │ f4kvs.h
                           ▼
                    ┌─────────────┐
                    │  f4kvs-ffi  │  thin C ABI wrapper
                    └──────┬──────┘
                           │
                           ▼
              ┌────────────────────────┐
              │  f4kvs-lsm (external)  │
              │  value → storage-core  │
              │         → LSM engine   │
              └────────────────────────┘
```

| Crate | Role |
|-------|------|
| [`f4kvs-ffi`](crates/f4kvs-ffi) | C ABI (`f4kvs.h` + `libf4kvs_ffi`) wrapping the LSM engine |
| [`f4kvs-lsm`](../f4kvs-lsm) | Canonical `f4kvs-value`, `f4kvs-storage-core`, and `f4kvs-lsm` crates |

The [f4kvs-v2](https://github.com/f4kvs/f4kvs-v2) monorepo shares the same `f4kvs-lsm` dependency but does **not** route its server through this FFI layer.

## Prerequisites

- Rust ≥ 1.75
- `f4kvs-lsm` cloned at `../f4kvs-lsm` (path dependency)

## Quick start — Rust (persistent LSM)

```rust
use f4kvs_lsm::{LsmConfig, LsmTreeEngine};
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = LsmConfig::default();
    config.data_dir = std::path::PathBuf::from("./data");
    let engine = LsmTreeEngine::new(config).await?;
    engine.put("key", &Value::from("value")).await?;
    Ok(())
}
```

## Quick start — C

```c
#include "f4kvs.h"

F4KvsEngine *engine = f4kvs_open(NULL);
f4kvs_put(engine, "greeting", "hello");
char *value = f4kvs_get(engine, "greeting");
f4kvs_free_string(value);
f4kvs_close(engine);
```

Build the shared library:

```bash
make build   # cargo build -p f4kvs-ffi --release
make test    # cargo test -p f4kvs-ffi
```

Header: [`crates/f4kvs-ffi/include/f4kvs.h`](crates/f4kvs-ffi/include/f4kvs.h)

## Benchmark vs SQLite

Product-shaped workloads (memoir blobs, RAG chunk ingest, prefix scan, random gets):

```bash
./scripts/bench_embed_vs_sqlite.sh
```

Results and interpretation: `projects-tracker/docs/f4kvs-sqlite-benchmark.md` (portfolio repo).

## Release alignment

Pin the same `f4kvs-lsm` version/tag as `f4kvs-v2` on every release. See [`../f4kvs-lsm/RELEASING.md`](../f4kvs-lsm/RELEASING.md).