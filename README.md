# f4kvs-ffi

Embedded key-value store with an on-disk LSM engine and a C FFI for language bindings (Go, etc.).

Extracted from the [f4kvs-v2](https://github.com/f4kvs) monorepo. Licensed under [Apache 2.0](LICENSE-APACHE).

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Consumers: Go, C, other languages  │  Rust apps        │
└──────────────┬──────────────────────┴─────────┬──────────┘
               │ f4kvs.h                        │ F4KVSCore / LsmTreeEngine
               ▼                                ▼
        ┌─────────────┐                  ┌─────────────┐
        │  f4kvs-ffi  │                  │ f4kvs-core  │  in-memory engine
        └──────┬──────┘                  └──────┬──────┘
               │                                │
               ▼                                ▼
        ┌─────────────┐                  ┌──────────────────┐
        │f4kvs-storage│◄─────────────────│ f4kvs-storage-   │
        │    -lsm     │  StorageEngine   │     core         │
        └─────────────┘  trait           └──────────────────┘
```

| Crate | Role |
|-------|------|
| [`f4kvs-core`](crates/f4kvs-core) | In-memory KVS (`F4KVSCore`), `Value` model, batch/scan. Also carries advanced modules (auth, RBAC, encryption) that are **experimental**. |
| [`f4kvs-storage-core`](crates/f4kvs-storage-core) | `StorageEngine` trait, shared config and stats types. |
| [`f4kvs-storage-lsm`](crates/f4kvs-storage-lsm) | Persistent LSM engine: WAL, memtable, bloom filters, compaction. |
| [`f4kvs-ffi`](crates/f4kvs-ffi) | C ABI (`f4kvs.h` + `libf4kvs_ffi`) wrapping the LSM engine. |

## Prerequisites

- Rust ≥ 1.75

## Quick start — Rust (in-memory)

```rust
use f4kvs_core::{Config, F4KVSCore, Result, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let engine = F4KVSCore::with_config(Config::default())?;
    engine.put("greeting", &Value::from("hello")).await?;
    let value = engine.get("greeting").await?;
    println!("{:?}", value);
    Ok(())
}
```

## Quick start — Rust (persistent LSM)

```rust
use f4kvs_core::Value;
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_storage_lsm::{LsmConfig, LsmTreeEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = LsmConfig::default();
    config.data_dir = "/tmp/f4kvs_data".into();

    let engine = LsmTreeEngine::new(config).await?;
    engine.put("key", &Value::String("value".into())).await?;
    let value = engine.get("key").await?;
    println!("{:?}", value);
    Ok(())
}
```

## Quick start — C FFI

```c
#include "f4kvs.h"
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    F4KvsEngine *engine = f4kvs_engine_open("/tmp/f4kvs_ffi_demo");
    if (!engine) {
        fprintf(stderr, "open failed: %s\n", f4kvs_get_last_error());
        return 1;
    }

    if (f4kvs_engine_put(engine, "hello", "world") != F4KVS_SUCCESS) {
        fprintf(stderr, "put failed: %s\n", f4kvs_get_last_error());
        f4kvs_engine_free(engine);
        return 1;
    }

    char *value = NULL;
    if (f4kvs_engine_get(engine, "hello", &value) != F4KVS_SUCCESS) {
        fprintf(stderr, "get failed: %s\n", f4kvs_get_last_error());
        f4kvs_engine_free(engine);
        return 1;
    }

    printf("hello = %s\n", value);
    f4kvs_string_free(value);

    f4kvs_engine_close(engine);
    f4kvs_engine_free(engine);
    return 0;
}
```

See [`crates/f4kvs-ffi/include/f4kvs.h`](crates/f4kvs-ffi/include/f4kvs.h) for the full API (memory ownership, limits, error handling).

## Build

```bash
cargo build -p f4kvs-ffi --release
# or
make build
```

Artifacts in `target/release/`:

- `libf4kvs_ffi.a` (static)
- `libf4kvs_ffi.dylib` (macOS) or `libf4kvs_ffi.so` (Linux)

C header: [`crates/f4kvs-ffi/include/f4kvs.h`](crates/f4kvs-ffi/include/f4kvs.h)

## Test

```bash
cargo test -p f4kvs-core
cargo test -p f4kvs-storage-lsm
cargo test -p f4kvs-ffi
# or
make test
```

## Known limitations

- **FFI surface** is a subset of the Rust API: put/get/delete/exists/batch_delete/scan_prefix (+ byte variants). No batch put, range scan, or count via C.
- **FFI limits**: keys ≤ 1 MB, values ≤ 100 MB. Keys must be valid UTF-8.
- **Security modules** (`auth`, `rbac`, `encryption` in `f4kvs-core`) are experimental stubs — not suitable for production authentication or encryption.
- **LSM compression**: WAL compression is configurable; SSTable compression (LZ4/Zstd/etc.) is not implemented in this repository.
- **TTL**: optional `ttl` feature exists but requires an external `f4kvs_ttl` dependency not included in this workspace.

## Integration example (ai-rag-agent)

Point `F4KVS_ROOT` at this checkout when building [ai-rag-agent](https://github.com/noematic-eu/ai-rag-agent):

```bash
make f4kvs F4KVS_ROOT=/path/to/f4kvs-ffi
```

When the FFI surface changes, sync `internal/f4kvs/include/f4kvs.h` from `crates/f4kvs-ffi/include/f4kvs.h`.

## Crate documentation

| Crate | README |
|-------|--------|
| f4kvs-core | [crates/f4kvs-core/README.md](crates/f4kvs-core/README.md) |
| f4kvs-storage-core | [crates/f4kvs-storage-core/README.md](crates/f4kvs-storage-core/README.md) |
| f4kvs-storage-lsm | [crates/f4kvs-storage-lsm/README.md](crates/f4kvs-storage-lsm/README.md) |
| f4kvs-ffi | [crates/f4kvs-ffi/README.md](crates/f4kvs-ffi/README.md) |

Generate Rust API docs locally:

```bash
cargo doc --open -p f4kvs-core
cargo doc --open -p f4kvs-storage-lsm
```

## License

Licensed under the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
