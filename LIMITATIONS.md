# Known limitations — f4kvs-ffi

Current, verified limitations of the engine and its FFI bindings. Each entry
states the impact and the recommended workaround. This file should be updated
whenever a limitation is fixed or a new one is found.

## Durability

- **Group-commit / amortized WAL profiles acknowledge asynchronously.**
  With `wal_durability=1` (amortized) or group commit enabled, a write is
  durable only after the commit window (10–100 ms depending on options), not
  at call return. A process kill or power loss inside the window can lose
  acknowledged writes. Clean `Close()` always flushes and is safe.
  *Workaround: use strict durability (default) for data you cannot lose, or
  call `FlushWAL()` at points where a durable boundary is required.*
- **Crash-path durability is not yet test-covered.**
  The benchmark integrity gate (`post_restart_row_count`) validates clean
  shutdown and reopen. There is no automated test that kills the process
  inside a group-commit window to measure actual loss behavior.

## Performance

- **Strict per-put fsync is slow.** With default (strict) durability, single
  `Put` calls are ~6× slower than SQLite WAL FULL (~4.6 ms/put on Apple M4
  Max). Batch ingest and group-commit profiles are unaffected and outperform
  SQLite by 4–175×.
  *Workaround: use `BatchPutBytes`, or enable group commit / amortized WAL
  when the durability window is acceptable.*
- **All FFI calls are serialized by a global mutex** (`FFI_MUTEX` in
  `src/lib.rs`). Concurrent callers — even on independent engine handles —
  execute one at a time. Concurrent use is safe but yields no throughput
  scaling. This may be a workaround for an upstream LSM issue; it needs to be
  either justified with a reference or removed.

## API and FFI

- **`BatchPutBytes` is capped at 10,000 items per call** (`max_batch_size`
  in f4kvs-lsm, DoS protection). The limit is not exposed in
  `F4KvsOpenOptions` and is not documented in `f4kvs.h`; exceeding it returns
  a generic storage error.
  *Workaround: split larger ingests into batches of ≤ 10,000 items.*
- **Keys must be valid UTF-8 C strings** (no embedded NUL bytes, ≤ 1 MB).
  Binary keys are not supported. Values up to 100 MB.
- **String/binary value APIs must not be mixed on the same keyspace.**
  `f4kvs_engine_get` (string) applied to a value written via `put_bytes`
  runs lossy UTF-8 conversion and silently corrupts binary data. Values
  written by non-FFI writers (Int64, Json, …) are flattened to strings.
- **Go `Transaction.Commit` is not atomic.** It applies puts then deletes as
  two separate WAL batches; a failure between them leaves a partial commit.
  Transactions are a staging convenience, not an isolation mechanism, and are
  not goroutine-safe.
- **`Sync()` compacts the LSM.** Despite its name it triggers full
  compaction, not a WAL sync. Use `Flush()` / `FlushWAL()` for durability
  boundaries. (Naming fix planned.)
- **Error state is thread-local.** `f4kvs_get_last_error()` must be called
  on the same OS thread as the failed call. cgo callers can observe
  "unknown error" if the goroutine migrates threads between the two calls.
- **Some Go read paths swallow errors.** `ScanPrefixKeys` / `GetAllKeys`
  return `nil` on engine error, indistinguishable from an empty result.
- **Ephemeral engines leak disk.** `f4kvs_engine_new` creates a temp
  directory that is never removed.

## Platform and packaging

- **cgo is required** for the Go bindings; the `!cgo` build returns
  `ErrCGORequired` for every call.
- **Dynamic linking has no rpath** in the Go bindings: consumers must set
  `DYLD_LIBRARY_PATH` / `LD_LIBRARY_PATH` or link the staticlib.
- **No CI** runs the Rust + Go test matrix; regressions across the two
  language surfaces are caught manually.
