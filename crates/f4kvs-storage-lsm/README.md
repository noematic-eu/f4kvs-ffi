# f4kvs-storage-lsm

On-disk LSM (Log-Structured Merge) tree storage engine for F4KVS.

## Implemented

- **MemTable** — in-memory sorted writes with flush to SSTables
- **Multi-level SSTables** — L0 (overlapping) through L1+ (non-overlapping)
- **Write-ahead log (WAL)** — crash recovery and durability
- **Bloom filters** — fast negative lookups on SSTables
- **Background compaction** — configurable strategies
- **Block cache** — read caching for SSTable blocks

## Not implemented in this repository

- **SSTable compression** (LZ4, Snappy, Zstd) — error types exist but no codec integration
- **Column families** — not exposed in this extracted codebase
- **TTL expiry** — behind the optional `ttl` feature, which requires an external `f4kvs_ttl` crate not included in this workspace

WAL compression can be toggled via `LsmConfig` (`enable_compression`), but on-disk block compression is not available.

## Quick start

```rust
use f4kvs_core::Value;
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_storage_lsm::{LsmConfig, LsmTreeEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = LsmConfig::default();
    config.data_dir = "/tmp/f4kvs_lsm".into();

    let engine = LsmTreeEngine::new(config).await?;
    engine.put("key", &Value::String("value".into())).await?;
    let value = engine.get("key").await?;
    println!("{:?}", value);
    Ok(())
}
```

## Test

```bash
cargo test -p f4kvs-storage-lsm
```

## Documentation

```bash
cargo doc --open -p f4kvs-storage-lsm
```
