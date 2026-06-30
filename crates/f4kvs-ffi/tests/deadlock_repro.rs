//! Reproduce vault-sync style hangs: large FFI batch_put workloads on a shared runtime.

mod common;
use common::to_c_string;
use f4kvs_ffi::*;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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
        assert_eq!(result, F4KvsResult::Success, "batch {b} failed");
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
        assert_eq!(result, F4KvsResult::Success);
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
                    assert_eq!(result, F4KvsResult::Success);
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
    };
    let engine = unsafe { f4kvs_engine_open_ex(dir.as_ptr(), &options) };
    assert!(!engine.is_null());

    let started = Instant::now();
    for b in 0..300 {
        let result = batch_put_n(engine, "gc", b * 500, 500);
        assert_eq!(result, F4KvsResult::Success);
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
    let (dir, engine) = temp_engine_dir();
    assert!(!engine.is_null());

    for b in 0..400 {
        let result = batch_put_n(engine, "prefill", b * 200, 200);
        assert_eq!(result, F4KvsResult::Success);
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
    assert_eq!(result, F4KvsResult::Success);
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
    assert_eq!(result, F4KvsResult::Success);
    assert!(started.elapsed() < Duration::from_secs(60));

    unsafe {
        f4kvs_engine_free(engine);
    }
    let _ = std::fs::remove_dir_all(dir);
}