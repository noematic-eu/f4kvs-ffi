# f4kvs-core

Core key-value store engine for F4KVS: in-memory storage, value model, and shared primitives used by the LSM and FFI layers.

## API principale

| Area | Modules | Status |
|------|---------|--------|
| Engine | `engine`, `value`, `config`, `error`, `sync` | Stable |
| In-memory storage | `hashmap`, `btreemap`, `memory_storage` | Stable |
| Batch / scan | `engine` (batch_put, scan_prefix, scan_range, count_*) | Stable |
| Concurrency | `lockfree` (DashMap-backed safe wrappers) | Stable |
| Memory pools | `memory_pool`, `safe_memory_pool` | Stable |
| Auth / RBAC / encryption | `auth`, `rbac`, `encryption`, `security` | **Experimental** — stub implementations, not production-ready |
| Database / query | `database`, `query` | Advanced / evolving |

### Experimental security modules

`auth`, `rbac`, and `encryption` exist for API compatibility with the upstream monorepo. Password hashing, JWT signing, and some encryption paths use simplified placeholder logic. Do not rely on them for production security without a full security audit and hardening.

## Quick start

```rust
use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::new().with_storage_mode(StorageMode::HashMap);
    let engine = F4KVSCore::with_config(config)?;

    engine.put("greeting", &Value::from("hello world")).await?;
    let value = engine.get("greeting").await?;
    println!("{:?}", value);

    Ok(())
}
```

## Safety posture

- Crate-level linting denies `unwrap`/`expect` in non-test code paths.
- Public lock-free types are backed by safe wrappers (`safe_concurrency_wrappers`).

## Test

```bash
cargo test -p f4kvs-core
```

## Documentation

```bash
cargo doc --open -p f4kvs-core
```
