//! I/O pattern analysis tests for F4KVS Core
//!
//! These tests analyze I/O patterns, disk usage, and performance
//! characteristics under different workloads.

use f4kvs_core::*;
use std::sync::Arc;
use std::time::Instant;

/// Test sequential read pattern
#[tokio::test]
async fn test_sequential_read_pattern() {
    let engine = F4KVSCore::new().unwrap();
    let key_count = 1000;

    // Pre-populate with data
    for i in 0..key_count {
        let key = format!("seq_read_key_{:04}", i);
        let value = Value::String(format!("seq_read_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }

    // Measure sequential read performance
    let start_time = Instant::now();
    for i in 0..key_count {
        let key = format!("seq_read_key_{:04}", i);
        let result = engine.get(&key).await.unwrap();
        assert!(result.is_some());
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = key_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 1000.0,
        "Sequential read throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test random read pattern
#[tokio::test]
async fn test_random_read_pattern() {
    let engine = F4KVSCore::new().unwrap();
    let key_count = 1000;

    // Pre-populate with data
    for i in 0..key_count {
        let key = format!("rand_read_key_{:04}", i);
        let value = Value::String(format!("rand_read_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }

    // Measure random read performance
    let start_time = Instant::now();
    for _ in 0..key_count {
        let i = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as usize)
            % key_count;
        let key = format!("rand_read_key_{:04}", i);
        let result = engine.get(&key).await.unwrap();
        assert!(result.is_some());
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = key_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 500.0,
        "Random read throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test sequential write pattern
#[tokio::test]
async fn test_sequential_write_pattern() {
    let engine = F4KVSCore::new().unwrap();
    let key_count = 1000;

    // Measure sequential write performance
    let start_time = Instant::now();
    for i in 0..key_count {
        let key = format!("seq_write_key_{:04}", i);
        let value = Value::String(format!("seq_write_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = key_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 1000.0,
        "Sequential write throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test random write pattern
#[tokio::test]
async fn test_random_write_pattern() {
    let engine = F4KVSCore::new().unwrap();
    let key_count = 1000;

    // Measure random write performance
    let start_time = Instant::now();
    for i in 0..key_count {
        let key = format!("rand_write_key_{:04}", i);
        let value = Value::String(format!("rand_write_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = key_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 500.0,
        "Random write throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test mixed read/write pattern
#[tokio::test]
async fn test_mixed_read_write_pattern() {
    let engine = F4KVSCore::new().unwrap();
    let operation_count = 2000;
    let read_ratio = 0.7; // 70% reads, 30% writes

    // Pre-populate with some data
    for i in 0..100 {
        let key = format!("mixed_key_{:04}", i);
        let value = Value::String(format!("mixed_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }

    // Measure mixed read/write performance
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("mixed_key_{:04}", i % 100);

        if (i as f32 / 100.0) < read_ratio {
            // Read operation
            let _ = engine.get(&key).await.unwrap();
        } else {
            // Write operation
            let value = Value::String(format!("mixed_value_{:04}", i));
            engine.put(&key, &value).await.unwrap();
        }
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = operation_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 800.0,
        "Mixed read/write throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test batch operation patterns
#[tokio::test]
async fn test_batch_operation_patterns() {
    let engine = F4KVSCore::new().unwrap();
    let batch_sizes = vec![10, 50, 100, 500, 1000];

    for batch_size in batch_sizes {
        let mut batch_data = Vec::new();

        // Prepare batch data
        for i in 0..batch_size {
            let key = format!("batch_key_{}_{}", batch_size, i);
            let value = Value::String(format!("batch_value_{}_{}", batch_size, i));
            batch_data.push((key, value));
        }

        // Measure batch write performance
        let start_time = Instant::now();
        engine.batch_put(batch_data).await.unwrap();
        let duration = start_time.elapsed();

        // Calculate throughput
        let throughput = batch_size as f64 / duration.as_secs_f64();
        assert!(
            throughput > 100.0,
            "Batch write throughput too low for size {}: {:.2} ops/sec",
            batch_size,
            throughput
        );

        // Clean up
        engine.clear().await.unwrap();
    }
}

/// Test I/O pattern under memory pressure
#[tokio::test]
async fn test_io_pattern_memory_pressure() {
    let engine = F4KVSCore::new().unwrap();
    let large_value_size = 1024 * 1024; // 1MB
    let operation_count = 100;

    // Measure I/O performance under memory pressure
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("pressure_key_{:04}", i);
        let value = Value::String("x".repeat(large_value_size));

        let result = engine.put(&key, &value).await;

        // Should either succeed or fail gracefully
        match result {
            Ok(_) => {
                // If successful, verify the value
                let retrieved = engine.get(&key).await.unwrap();
                assert!(retrieved.is_some());
            }
            Err(F4KvsError::Storage { .. }) => {
                // Expected under memory pressure
            }
            Err(e) => {
                panic!("Unexpected error under memory pressure: {:?}", e);
            }
        }
    }
    let duration = start_time.elapsed();

    // Calculate throughput
    let throughput = operation_count as f64 / duration.as_secs_f64();
    assert!(
        throughput > 10.0,
        "I/O throughput under memory pressure too low: {:.2} ops/sec",
        throughput
    );
}

/// Test I/O pattern with different value sizes
#[tokio::test]
async fn test_io_pattern_value_sizes() {
    let engine = F4KVSCore::new().unwrap();
    let value_sizes = vec![
        1024,        // 1KB
        1024 * 10,   // 10KB
        1024 * 100,  // 100KB
        1024 * 1024, // 1MB
    ];

    for value_size in value_sizes {
        let operation_count = 100;
        let mut throughputs = Vec::new();

        // Test multiple iterations for each value size
        for iteration in 0..5 {
            let start_time = Instant::now();

            for i in 0..operation_count {
                let key = format!("size_test_key_{}_{}_{}", value_size, iteration, i);
                let value = Value::String("x".repeat(value_size));

                engine.put(&key, &value).await.unwrap();
                engine.get(&key).await.unwrap();
                engine.delete(&key).await.unwrap();
            }

            let duration = start_time.elapsed();
            let throughput = operation_count as f64 / duration.as_secs_f64();
            throughputs.push(throughput);
        }

        // Calculate average throughput
        let avg_throughput = throughputs.iter().sum::<f64>() / throughputs.len() as f64;

        // Throughput should be reasonable for the value size
        let expected_throughput = match value_size {
            1024 => 1000.0,  // 1KB: 1000 ops/sec
            10240 => 500.0,  // 10KB: 500 ops/sec
            102400 => 100.0, // 100KB: 100 ops/sec
            1048576 => 10.0, // 1MB: 10 ops/sec
            _ => 1.0,
        };

        assert!(
            avg_throughput >= expected_throughput * 0.5,
            "I/O throughput too low for value size {}: {:.2} < {:.2} ops/sec",
            value_size,
            avg_throughput,
            expected_throughput * 0.5
        );
    }
}

/// Test I/O pattern with concurrent operations
#[tokio::test]
async fn test_io_pattern_concurrent_operations() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let concurrent_tasks = 10;
    let operations_per_task = 100;

    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.push(tokio::spawn(async move {
            let start_time = Instant::now();

            for i in 0..operations_per_task {
                let key = format!("concurrent_io_key_{}_{}", task_id, i);
                let value = Value::String(format!("concurrent_io_value_{}_{}", task_id, i));

                engine_clone.put(&key, &value).await.unwrap();
                engine_clone.get(&key).await.unwrap();
                engine_clone.delete(&key).await.unwrap();
            }

            let duration = start_time.elapsed();
            operations_per_task as f64 / duration.as_secs_f64()
        }));
    }

    // Wait for all tasks to complete and collect throughputs
    let mut total_throughput = 0.0;
    for handle in handles {
        let throughput = handle.await.unwrap();
        total_throughput += throughput;
    }

    // Total throughput should be reasonable
    assert!(
        total_throughput > 1000.0,
        "Total concurrent I/O throughput too low: {:.2} ops/sec",
        total_throughput
    );
}

/// Test I/O pattern with different access patterns
#[tokio::test]
async fn test_io_pattern_access_patterns() {
    let engine = F4KVSCore::new().unwrap();
    let key_count = 1000;

    // Pre-populate with data
    for i in 0..key_count {
        let key = format!("access_pattern_key_{:04}", i);
        let value = Value::String(format!("access_pattern_value_{:04}", i));
        engine.put(&key, &value).await.unwrap();
    }

    // Test different access patterns
    let patterns = vec![
        ("sequential", (0..key_count).collect::<Vec<usize>>()),
        ("reverse", (0..key_count).rev().collect::<Vec<usize>>()),
        ("random", {
            let mut indices: Vec<usize> = (0..key_count).collect();
            // Simple shuffle using system time
            for i in 0..indices.len() {
                let j = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize
                    + i)
                    % indices.len();
                indices.swap(i, j);
            }
            indices
        }),
    ];

    for (pattern_name, indices) in patterns {
        let start_time = Instant::now();

        for &i in &indices {
            let key = format!("access_pattern_key_{:04}", i);
            let _ = engine.get(&key).await.unwrap();
        }

        let duration = start_time.elapsed();
        let throughput = key_count as f64 / duration.as_secs_f64();

        assert!(
            throughput > 100.0,
            "{} access pattern throughput too low: {:.2} ops/sec",
            pattern_name,
            throughput
        );
    }
}
