# f4kvs-ffi

C ABI bindings for the F4KVS LSM engine.

## Deliverables

| Artifact | Path |
|----------|------|
| C header | [`include/f4kvs.h`](include/f4kvs.h) |
| Static library | `target/release/libf4kvs_ffi.a` |
| Dynamic library | `target/release/libf4kvs_ffi.so` (Linux) or `.dylib` (macOS) |

## API surface

The FFI exposes a focused subset of the Rust LSM engine:

- Engine lifecycle: `f4kvs_engine_new`, `f4kvs_engine_open`, `f4kvs_engine_close`, `f4kvs_engine_free`
- KV operations: put, get, delete, exists, batch_delete
- Binary variants: `f4kvs_engine_put_bytes`, `f4kvs_engine_get_bytes`
- Prefix scan: `f4kvs_engine_scan_prefix`
- Maintenance: `f4kvs_engine_compact`
- Error helpers: `f4kvs_get_last_error`, `f4kvs_result_to_string`

See [`include/f4kvs.h`](include/f4kvs.h) for memory ownership rules, size limits, and thread-safety notes.

## Build

```bash
cargo build -p f4kvs-ffi --release
```

The crate produces `cdylib`, `staticlib`, and `rlib` targets.

## Test

97 integration tests cover boundary conditions, thread safety, input validation, and C interop:

```bash
cargo test -p f4kvs-ffi
```

## Linking from C

```bash
cc -o demo demo.c -I crates/f4kvs-ffi/include \
   -L target/release -lf4kvs_ffi -lpthread -ldl -lm
```

On macOS, add `-framework Security` if required by your Rust toolchain's TLS stack (not needed for basic KV usage).
