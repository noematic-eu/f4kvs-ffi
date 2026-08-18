//! Reproduce vault-sync style hangs: large FFI batch_put workloads on a shared runtime.
//!
//! These tests open real engines and write tens of megabytes. They take a
//! process-wide + cross-process lock so parallel `cargo test` / nextest
//! siblings do not starve FDs or turn a hang-regression into a flaky
//! `ErrorStorage`.

mod common;
use common::to_c_string;
use f4kvs_ffi::*;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Cross-process lock so this file's heavy tests never overlap (cargo test
/// threads *or* nextest's one-process-per-test). Stale dir (>5 min) is stolen.
struct HeavyIoGuard {
    lock_dir: PathBuf,
}

impl Drop for HeavyIoGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.lock_dir);
    }
}

fn exclusive_io() -> HeavyIoGuard {
    let lock_dir = std::env::temp_dir().join("f4kvs-ffi-deadlock-repro.lock");
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match std::fs::create_dir(&lock_dir) {
            Ok(()) => return HeavyIoGuard { lock_dir },
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Ok(meta) = std::fs::metadata(&lock_dir) {
                    if let Ok(modified) = meta.modified() {
                        if modified.elapsed().unwrap_or_default() > Duration::from_secs(300) {
                            let _ = std::fs::remove_dir(&lock_dir);
                            continue;
                        }
                    }
                }
                if Instant::now() >= deadline {
                    panic!("timed out waiting for heavy-IO lock {lock_dir:?}");
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("create lock dir {lock_dir:?}: {e}"),
        }
    }
}

fn last_error() -> String {
    unsafe {
        let p = f4kvs_get_last_error();
        if p.is_null() {
            "<null>".to_string()
        } else {
            std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

fn assert_success(result: F4KvsResult, what: &str) {
    assert_eq!(
        result,
        F4KvsResult::Success,
        "{what}: {result:?} last_error={}",
        last_error()
    );
}

fn default_open_options() -> F4KvsOpenOptions {
    F4KvsOpenOptions {
        group_commit_enabled: 0,
        group_commit_max_wait_ms: 0,
        group_commit_max_batch_size: 0,
        group_commit_wait_durable: 0,
        wal_engine: 0,
        wal_durability: 0,
        group_commit_idle_flush_ms: 0,
        max_batch_size: 0,
        compaction_background: 0,
        max_sstables_per_level: 0,
        memtable_max_size: 0,
        sstable_target_size: 0,
        sstable_max_size: 0,
    }
}

fn batch_put_n(engine: *mut F4KvsEngine, prefix: &str, start: usize, count: usize) -> F4KvsResult {
    let key_strings: Vec<_> = (0..count)
        .map(|i| to_c_string(&format!("{prefix}/file_{:08}", start + i)))
        .collect();
    let key_ptrs: Vec<*const c_char> = key_strings.iter().map(|k| k.as_ptr()).collect();
    let values: Vec<Vec<u8>> = (0..count)
        .map(|i| format!("v{}", start + i).into_bytes())
        .collect();
    let value_ptrs: Vec<*const u8> = values.iter().map(|v| v.as_ptr()).collect();
    let value_lens: Vec<usize> = values.iter().map(|v| v.len()).collect();

    unsafe {
        f4kvs_engine_batch_put_bytes(
            engine,
            key_ptrs.as_ptr(),
            value_ptrs.as_ptr(),
            value_lens.as_ptr(),
            count,
        )
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    fn walk(p: &std::path::Path) -> u64 {
        let Ok(rd) = std::fs::read_dir(p) else {
            return 0;
        };
        rd.filter_map(|e| e.ok())
            .map(|e| {
                let path = e.path();
                if path.is_dir() {
                    walk(&path)
                } else {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                }
            })
            .sum()
    }
    walk(path)
}

fn temp_engine_dir() -> (std::path::PathBuf, *mut F4KvsEngine) {
    let path = std::env::temp_dir().join(format!(
        "f4kvs_deadlock_repro_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("mkdir");
    let dir = to_c_string(path.to_str().expect("utf8"));
    let engine = unsafe { f4kvs_engine_open(dir.as_ptr()) };
    (path, engine)
}

#[test]
fn test_vault_sync_large_batch_put_no_hang() {
    let _io = exclusive_io();
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    let batch_size = 500;
    let batches = 600; // 300k keys, similar to vault sync scale
    let timeout = Duration::from_secs(120);
    let started = Instant::now();

    for b in 0..batches {
        if started.elapsed() > timeout {
            panic!(
                "HANG/TIMEOUT at batch {b}/{batches} after {:?}",
                started.elapsed()
            );
        }
        let result = batch_put_n(engine, "media-1/files", b * batch_size, batch_size);
        assert_success(result, &format!("vault batch {b}"));
        if b % 50 == 0 {
            eprintln!("batch {b}/{batches} ok ({:?})", started.elapsed());
        }
    }

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_batch_put_while_compact_no_hang() {
    let _io = exclusive_io();
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    let engine_addr = engine as usize;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_compactor = stop.clone();

    let compactor = thread::spawn(move || {
        let engine = engine_addr as *mut F4KvsEngine;
        while !stop_compactor.load(Ordering::Relaxed) {
            unsafe {
                let _ = f4kvs_engine_compact(engine);
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    let started = Instant::now();
    let timeout = Duration::from_secs(90);
    for b in 0..200 {
        if started.elapsed() > timeout {
            stop.store(true, Ordering::Relaxed);
            let _ = compactor.join();
            panic!("HANG/TIMEOUT at batch {b}");
        }
        let result = batch_put_n(engine, "media-2/files", b * 1000, 1000);
        assert_success(result, &format!("compact-race batch {b}"));
    }

    stop.store(true, Ordering::Relaxed);
    compactor.join().expect("compactor join");

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_concurrent_batch_put_same_engine_no_hang() {
    let _io = exclusive_io();
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    let engine_addr = engine as usize;
    let threads = 8;
    let batches_per_thread = 50;
    let batch_size = 500;
    let timeout = Duration::from_secs(120);
    let started = Instant::now();

    let handles: Vec<_> = (0..threads)
        .map(|tid| {
            thread::spawn(move || {
                let engine = engine_addr as *mut F4KvsEngine;
                for b in 0..batches_per_thread {
                    let prefix = format!("thread{tid}");
                    let result = batch_put_n(engine, &prefix, b * batch_size, batch_size);
                    assert_success(result, &format!("{prefix} batch {b}"));
                }
            })
        })
        .collect();

    for h in handles {
        if started.elapsed() > timeout {
            panic!("HANG/TIMEOUT after {:?}", started.elapsed());
        }
        h.join().expect("thread join");
    }

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_group_commit_wait_durable_large_batch_no_hang() {
    let _io = exclusive_io();
    let path = std::env::temp_dir().join(format!(
        "f4kvs_gc_repro_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("mkdir");

    let dir = to_c_string(path.to_str().expect("utf8"));
    let options = F4KvsOpenOptions {
        group_commit_enabled: 1,
        group_commit_max_wait_ms: 10,
        group_commit_max_batch_size: 1000,
        group_commit_wait_durable: 1,
        wal_engine: 0,
        wal_durability: 0,
        group_commit_idle_flush_ms: 0,
        max_batch_size: 0,
        compaction_background: 0,
        max_sstables_per_level: 0,
        memtable_max_size: 0,
        sstable_target_size: 0,
        sstable_max_size: 0,
    };
    let engine = unsafe { f4kvs_engine_open_ex(dir.as_ptr(), &options) };
    assert!(!engine.is_null());

    let started = Instant::now();
    for b in 0..300 {
        let result = batch_put_n(engine, "gc", b * 500, 500);
        assert_success(result, &format!("group-commit batch {b}"));
        if started.elapsed() > Duration::from_secs(90) {
            panic!("group_commit durable hang at batch {b}");
        }
    }

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn test_batch_put_slowdown_after_many_l0_flushes() {
    let _io = exclusive_io();
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    for b in 0..400 {
        let result = batch_put_n(engine, "prefill", b * 200, 200);
        assert_success(result, &format!("l0-prefill batch {b}"));
        if b % 20 == 19 {
            unsafe {
                let _ = f4kvs_engine_flush(engine);
            }
        }
    }

    let started = Instant::now();
    let result = batch_put_n(engine, "slow", 0, 500);
    let elapsed = started.elapsed();
    eprintln!("batch_put after many L0 flushes: {:?}", elapsed);
    assert_success(result, "l0-slowdown probe");
    assert!(
        elapsed < Duration::from_secs(30),
        "batch_put took too long ({elapsed:?})"
    );

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_max_batch_put_large_values_no_hang() {
    let _io = exclusive_io();
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    let count = 10_000;
    let big = vec![b'x'; 4 * 1024];
    let key_strings: Vec<_> = (0..count)
        .map(|i| to_c_string(&format!("big/file_{:08}", i)))
        .collect();
    let key_ptrs: Vec<*const c_char> = key_strings.iter().map(|k| k.as_ptr()).collect();
    let values: Vec<Vec<u8>> = (0..count).map(|_| big.clone()).collect();
    let value_ptrs: Vec<*const u8> = values.iter().map(|v| v.as_ptr()).collect();
    let value_lens: Vec<usize> = values.iter().map(|v| v.len()).collect();

    let started = Instant::now();
    let result = unsafe {
        f4kvs_engine_batch_put_bytes(
            engine,
            key_ptrs.as_ptr(),
            value_ptrs.as_ptr(),
            value_lens.as_ptr(),
            count,
        )
    };
    eprintln!("10k x 4KiB batch_put: {:?}", started.elapsed());
    assert_success(result, "10k x 4KiB batch_put");
    assert!(started.elapsed() < Duration::from_secs(60));

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn test_meso_chunked_4kb_no_hang() {
    // Hang gate: ~27 × 500 × 4KiB fills the default 64 MiB WAL segment.
    // Pre-fix: rotate_segment deadlocked on the segment write guard.
    // Compact is off so a sibling-test FD/IO storm cannot turn that gate
    // into a flaky ErrorStorage; WAL rotation still happens.
    let _io = exclusive_io();

    let path = std::env::temp_dir().join(format!(
        "f4kvs_meso_repro_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("mkdir");
    let dir = to_c_string(path.to_str().expect("utf8"));
    let mut options = default_open_options();
    options.compaction_background = 2; // off
    options.wal_engine = 0; // segment
    options.wal_durability = 0; // strict fsync — product meso path
    let engine = unsafe { f4kvs_engine_open_ex(dir.as_ptr(), &options) };
    assert!(!engine.is_null(), "open_ex failed: {}", last_error());

    let batch_size = 500;
    let batches = 60; // 30k keys, ~120 MiB — crosses WAL rotate twice
    let big = vec![b'x'; 4 * 1024];
    let timeout = Duration::from_secs(120);
    let started = Instant::now();

    for b in 0..batches {
        if started.elapsed() > timeout {
            panic!(
                "HANG/TIMEOUT at batch {b}/{batches} keys={} after {:?}",
                b * batch_size,
                started.elapsed()
            );
        }
        let start = b * batch_size;
        let key_strings: Vec<_> = (0..batch_size)
            .map(|i| {
                to_c_string(&format!(
                    "chunk:legal:doc-{:04}:chunk-{:06}",
                    (start + i) / 10,
                    start + i
                ))
            })
            .collect();
        let key_ptrs: Vec<*const c_char> = key_strings.iter().map(|k| k.as_ptr()).collect();
        let values: Vec<Vec<u8>> = (0..batch_size).map(|_| big.clone()).collect();
        let value_ptrs: Vec<*const u8> = values.iter().map(|v| v.as_ptr()).collect();
        let value_lens: Vec<usize> = values.iter().map(|v| v.len()).collect();

        let result = unsafe {
            f4kvs_engine_batch_put_bytes(
                engine,
                key_ptrs.as_ptr(),
                value_ptrs.as_ptr(),
                value_lens.as_ptr(),
                batch_size,
            )
        };
        if result != F4KvsResult::Success {
            let sst = std::fs::read_dir(&path)
                .map(|it| {
                    it.filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("sst"))
                        .count()
                })
                .unwrap_or(0);
            panic!(
                "batch {b} failed: {result:?} last_error={} sst={sst} dir_bytes={} elapsed={:?}",
                last_error(),
                dir_size(&path),
                started.elapsed()
            );
        }
        if b % 5 == 0 {
            eprintln!(
                "batch {b}/{batches} keys={} ok ({:?})",
                (b + 1) * batch_size,
                started.elapsed()
            );
        }
    }
    eprintln!("meso chunked 4kb complete: {:?}", started.elapsed());

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(path);
}
