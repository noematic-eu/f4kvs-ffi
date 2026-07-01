//! Native read baseline for FFI read-tax bench (`bench/ffi_read_tax`).
//!
//! Open a store prepared by the Go harness and time random `get` calls with the
//! same key pattern and FFI-matching LsmConfig defaults.

use f4kvs_lsm::core::config::WalSyncMode;
use f4kvs_lsm::{LsmConfig, LsmTreeEngine};
use f4kvs_storage_core::traits::StorageEngine;
use std::env;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = env::args().skip(1);
    let mut dir = None;
    let mut chunks = 2000usize;
    let mut random_gets = 500usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--chunks" => chunks = args.next().and_then(|s| s.parse().ok()).expect("--chunks"),
            "--random-gets" => {
                random_gets = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--random-gets")
            }
            "--help" | "-h" => {
                eprintln!(
                    "read_tax — native get baseline for bench/ffi_read_tax\n\
                     \n\
                     Usage:\n\
                       read_tax --dir PATH [--chunks N] [--random-gets N]\n"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let dir = dir.expect("--dir is required");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let keys: Vec<String> = (0..chunks)
        .map(|i| format!("chunk:legal:doc-{:04}:chunk-{:06}", i / 10, i))
        .collect();

    let mut config = LsmConfig::default();
    config.data_dir = dir.clone();
    config.wal.dir = dir.join("wal");
    config.wal.sync_mode = WalSyncMode::FsyncAsync;

    let engine = rt
        .block_on(LsmTreeEngine::new(config))
        .expect("open engine");

    let started = Instant::now();
    for i in 0..random_gets {
        let key = &keys[i % keys.len()];
        let value = rt
            .block_on(engine.get(key))
            .expect("get")
            .expect("missing key");
        let _ = value;
    }
    let elapsed = started.elapsed();
    let ms = elapsed.as_secs_f64() * 1000.0;
    let ops_per_s = if ms > 0.0 {
        random_gets as f64 / (ms / 1000.0)
    } else {
        0.0
    };

    println!("native_read_ms={ms:.1} ops/s={ops_per_s:.0}");
}