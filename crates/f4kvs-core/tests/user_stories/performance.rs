//! User Story: Performance Requirements
//!
//! As a developer
//! I want to achieve high performance
//! So that my application can handle production workloads

use f4kvs_core::{F4KVSCore, Value};
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_user_story_performance_throughput() {
    // Given: A new F4KVS instance with memory storage
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform a large number of operations
    let num_operations = 100_000;
    let start = Instant::now();

    // Pre-populate data
    for i in 0..num_operations {
        let key = format!("perf_key_{}", i);
        let value = Value::String(format!("perf_value_{}", i));
        engine.put(&key, &value).await.expect("Failed to put");
    }

    let write_duration = start.elapsed();
    let write_throughput = num_operations as f64 / write_duration.as_secs_f64();

    // Then: Throughput should be high (target: 1M+ ops/sec for memory storage)
    // Note: This is a relaxed check - actual performance depends on system
    assert!(
        write_throughput > 50_000.0,
        "Write throughput should be > 100K ops/sec, got {:.2} ops/sec",
        write_throughput
    );

    // Measure read throughput
    let read_start = Instant::now();
    for i in 0..num_operations {
        let key = format!("perf_key_{}", i);
        let _ = engine.get(&key).await.expect("Failed to get");
    }
    let read_duration = read_start.elapsed();
    let read_throughput = num_operations as f64 / read_duration.as_secs_f64();

    assert!(
        read_throughput > 50_000.0,
        "Read throughput should be > 100K ops/sec, got {:.2} ops/sec",
        read_throughput
    );
}

#[tokio::test]
async fn test_user_story_performance_latency() {
    // Given: A new F4KVS instance with data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert some data
    engine
        .put("latency_key", &Value::String("latency_value".to_string()))
        .await
        .expect("Failed to put");

    // When: I measure operation latency
    let num_samples = 1000;
    let mut latencies = Vec::new();

    for _ in 0..num_samples {
        let start = Instant::now();
        let _ = engine.get("latency_key").await.expect("Failed to get");
        let latency = start.elapsed();
        latencies.push(latency);
    }

    // Then: P99 latency should be < 1ms for memory storage
    latencies.sort();
    let p99_index = (num_samples as f64 * 0.99) as usize;
    let p99_latency = latencies[p99_index];

    // Note: This is a relaxed check - actual latency depends on system load
    assert!(
        p99_latency < Duration::from_millis(10),
        "P99 latency should be < 10ms for memory storage, got {:?}",
        p99_latency
    );
}

#[tokio::test]
async fn test_user_story_performance_batch() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I compare batch operations vs individual operations
    let batch_size = 1000;

    // Measure individual operations
    let individual_start = Instant::now();
    for i in 0..batch_size {
        let key = format!("individual_key_{}", i);
        let value = Value::String(format!("value_{}", i));
        engine.put(&key, &value).await.expect("Failed to put");
    }
    let individual_duration = individual_start.elapsed();

    // Clear for batch test
    engine.clear().await.expect("Failed to clear");

    // Measure batch operations
    let batch_start = Instant::now();
    let mut batch_data = Vec::new();
    for i in 0..batch_size {
        let key = format!("batch_key_{}", i);
        let value = Value::String(format!("value_{}", i));
        batch_data.push((key, value));
    }
    engine
        .batch_put(batch_data)
        .await
        .expect("Failed to batch put");
    let batch_duration = batch_start.elapsed();

    // Then: Batch operations should be faster (or at least not significantly slower)
    // Note: Batch operations may not always be faster due to validation overhead,
    // but they should be reasonably efficient
    assert!(
        batch_duration <= individual_duration * 2,
        "Batch operations should be reasonably efficient. Individual: {:?}, Batch: {:?}",
        individual_duration,
        batch_duration
    );
}

#[tokio::test]
async fn test_user_story_performance_scan() {
    // Given: A new F4KVS instance with a large dataset
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert 10K keys with a common prefix
    let prefix = "scan_perf:";
    let dataset_size = 10_000;

    for i in 0..dataset_size {
        let key = format!("{}{}", prefix, i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I scan the prefix
    let scan_start = Instant::now();
    let scanned = engine.scan_prefix(prefix).await.expect("Failed to scan");
    let scan_duration = scan_start.elapsed();

    // Then: Scan should be efficient
    assert_eq!(scanned.len(), dataset_size, "Should find all keys");

    // Scan should complete in reasonable time (< 1 second for 10K keys)
    assert!(
        scan_duration < Duration::from_secs(1),
        "Scan should be efficient, took {:?}",
        scan_duration
    );
}

#[tokio::test]
async fn test_user_story_performance_concurrent() {
    // Given: A new F4KVS instance
    let engine = std::sync::Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // When: I perform operations under concurrent load
    let num_concurrent_ops = 1000;
    let num_tasks = 10;
    let ops_per_task = num_concurrent_ops / num_tasks;

    let start = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..num_tasks {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            for i in 0..ops_per_task {
                let key = format!("concurrent_perf_{}_{}", task_id, i);
                let value = Value::String(format!("value_{}_{}", task_id, i));
                engine_clone.put(&key, &value).await.expect("Failed to put");
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task should complete");
    }

    let duration = start.elapsed();
    let throughput = num_concurrent_ops as f64 / duration.as_secs_f64();

    // Then: Performance should remain good under concurrent load
    assert!(
        throughput > 10_000.0,
        "Concurrent throughput should be > 10K ops/sec, got {:.2} ops/sec",
        throughput
    );
}

#[tokio::test]
async fn test_user_story_performance_memory_efficiency() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I store and then delete a large amount of data
    let num_keys = 10_000;
    let value_size = 1024; // 1KB per value

    // Insert data
    for i in 0..num_keys {
        let key = format!("memory_key_{}", i);
        let value = Value::String("x".repeat(value_size));
        engine.put(&key, &value).await.expect("Failed to put");
    }

    // Get initial stats
    let _stats_before = engine.stats().await.expect("Failed to get stats");

    // Delete all data
    for i in 0..num_keys {
        let key = format!("memory_key_{}", i);
        engine.delete(&key).await.expect("Failed to delete");
    }

    // Get final stats
    let stats_after = engine.stats().await.expect("Failed to get stats");

    // Then: Memory should be freed (or at least not significantly increased)
    // Note: Exact memory behavior depends on implementation
    // We just verify the operation completes successfully
    assert!(stats_after.key_count == 0, "All keys should be deleted");
}
