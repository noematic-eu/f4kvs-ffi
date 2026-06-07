# f4kvs-storage-core

Shared traits, configuration, and utilities for F4KVS storage engine implementations.

## Role in the stack

This crate sits between `f4kvs-core` (value model and in-memory engine) and concrete backends like `f4kvs-storage-lsm`. It defines the contract that all persistent engines must implement.

## Key exports

| Module | Purpose |
|--------|---------|
| `traits::StorageEngine` | Async put/get/delete/scan interface |
| `config` | `StorageConfig`, WAL/compaction/cache settings |
| `stats` | `StorageStats`, compaction and I/O metrics |
| `monitoring` | Health checks and alert primitives |

## Usage

Storage backends depend on this crate and implement `StorageEngine`:

```rust,ignore
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_storage_core::{Result, Value};

struct MyEngine { /* ... */ }

#[async_trait::async_trait]
impl StorageEngine for MyEngine {
    // Implement required methods...
}
```

The LSM implementation lives in [`f4kvs-storage-lsm`](../f4kvs-storage-lsm).

## Test

```bash
cargo test -p f4kvs-storage-core
```

## Documentation

```bash
cargo doc --open -p f4kvs-storage-core
```
