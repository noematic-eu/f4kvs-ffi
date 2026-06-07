//! Memory pressure and leak detection tests for F4KVS Core
//!
//! These tests verify memory usage patterns, leak detection, and behavior
//! under memory pressure conditions.

use f4kvs_core::*;
use std::sync::Arc;
use std::time::Duration;

/// Test memory usage under sustained load
#[tokio::test]
async fn test_memory_usage_sustained_load() {
    let engine = F4KVSCore::new().unwrap();
    let initial_stats = engine.stats().await.unwrap();
    let initial_memory = initial_stats.memory_usage;

    // Generate sustained load
    let iterations = 10000;
    let batch_size = 100;

    for batch in 0..(iterations / batch_size) {
        let mut batch_data = Vec::new();

        for i in 0..batch_size {
            let key = format!("memory_test_key_{}_{}", batch, i);
            let value = Value::String(format!("memory_test_value_{}_{}", batch, i));
            batch_data.push((key, value));
        }

        // Batch put
        engine.batch_put(batch_data).await.unwrap();

        // Periodically check memory usage
        if batch % 10 == 0 {
            let stats = engine.stats().await.unwrap();
            let current_memory = stats.memory_usage;

            // Memory should not grow unbounded
            let max_expected = if initial_memory == 0 {
                (batch + 1) * 10000
            } else {
                initial_memory + (batch * 10000)
            };
            assert!(
                current_memory < max_expected,
                "Memory usage too high: {} > {} (batch: {}, initial: {})",
                current_memory,
                max_expected,
                batch,
                initial_memory
            );
        }
    }

    // Final memory check
    let final_stats = engine.stats().await.unwrap();
    let final_memory = final_stats.memory_usage;

    // Memory should be reasonable relative to data size
    let expected_memory = if initial_memory == 0 {
        iterations * 2000
    } else {
        initial_memory + (iterations * 2000)
    };
    assert!(
        final_memory < expected_memory * 2,
        "Memory usage excessive: {} > {}",
        final_memory,
        expected_memory * 2
    );
}

/// Test memory leak detection over time
#[tokio::test]
async fn test_memory_leak_detection() {
    let engine = F4KVSCore::new().unwrap();
    let iterations = 1000;
    let cycles = 10;

    let mut memory_samples = Vec::new();

    for cycle in 0..cycles {
        // Add data
        for i in 0..iterations {
            let key = format!("leak_test_key_{}_{}", cycle, i);
            let value = Value::String(format!("leak_test_value_{}_{}", cycle, i));
            engine.put(&key, &value).await.unwrap();
        }

        // Remove data
        for i in 0..iterations {
            let key = format!("leak_test_key_{}_{}", cycle, i);
            engine.delete(&key).await.unwrap();
        }

        // Force garbage collection if available
        engine.clear().await.unwrap();

        // Sample memory usage
        let stats = engine.stats().await.unwrap();
        memory_samples.push(stats.memory_usage);

        // Small delay to allow cleanup
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Check for memory leaks
    let first_memory = memory_samples[0];
    let last_memory = memory_samples[memory_samples.len() - 1];

    // Memory should not grow significantly after cleanup
    assert!(
        last_memory <= first_memory * 2,
        "Potential memory leak detected: {} > {}",
        last_memory,
        first_memory * 2
    );
}

/// Test memory pressure handling
#[tokio::test]
async fn test_memory_pressure_handling() {
    let engine = F4KVSCore::new().unwrap();
    let large_value_size = 1024 * 1024; // 1MB
    let iterations = 100;

    // Create memory pressure by adding large values
    for i in 0..iterations {
        let key = format!("pressure_key_{}", i);
        let value = Value::String("x".repeat(large_value_size));

        let result = engine.put(&key, &value).await;

        // Should either succeed or fail gracefully due to memory pressure
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
}

/// Test memory usage with different value sizes
#[tokio::test]
async fn test_memory_usage_value_sizes() {
    let engine = F4KVSCore::new().unwrap();
    let value_sizes = vec![
        1024,        // 1KB
        1024 * 10,   // 10KB
        1024 * 100,  // 100KB
        1024 * 1024, // 1MB
    ];

    let mut memory_usage = Vec::new();

    for size in value_sizes {
        let key = format!("size_test_key_{}", size);
        let value = Value::String("x".repeat(size));

        engine.put(&key, &value).await.unwrap();

        let stats = engine.stats().await.unwrap();
        memory_usage.push((size, stats.memory_usage));

        // Clean up
        engine.delete(&key).await.unwrap();
    }

    // Verify memory usage scales reasonably with value size
    for i in 1..memory_usage.len() {
        let (prev_size, prev_memory) = memory_usage[i - 1];
        let (curr_size, curr_memory) = memory_usage[i];

        let size_ratio = curr_size as f64 / prev_size as f64;
        let memory_ratio = curr_memory as f64 / prev_memory as f64;

        // Memory usage should scale roughly with value size
        assert!(
            memory_ratio <= size_ratio * 1.5,
            "Memory usage doesn't scale properly: {} > {}",
            memory_ratio,
            size_ratio * 1.5
        );
    }
}

/// Test memory usage with concurrent operations
#[tokio::test]
async fn test_memory_usage_concurrent_operations() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let concurrent_tasks = 10;
    let iterations_per_task = 100;

    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..iterations_per_task {
                let key = format!("concurrent_memory_key_{}_{}", task_id, i);
                let value = Value::String(format!("concurrent_memory_value_{}_{}", task_id, i));

                engine_clone.put(&key, &value).await.unwrap();

                // Periodically check memory usage
                if i % 10 == 0 {
                    let stats = engine_clone.stats().await.unwrap();
                    // Memory should not grow unbounded
                    assert!(
                        stats.memory_usage < 100 * 1024 * 1024, // 100MB limit
                        "Memory usage too high in concurrent operations: {}",
                        stats.memory_usage
                    );
                }
            }
        }));
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Final memory check
    let final_stats = engine.stats().await.unwrap();
    assert!(
        final_stats.memory_usage < 200 * 1024 * 1024, // 200MB limit
        "Final memory usage too high: {}",
        final_stats.memory_usage
    );
}

/// Test memory usage during batch operations
#[tokio::test]
async fn test_memory_usage_batch_operations() {
    let engine = F4KVSCore::new().unwrap();
    let batch_sizes = vec![10, 100, 1000, 10000];

    for batch_size in batch_sizes {
        let mut batch_data = Vec::new();

        for i in 0..batch_size {
            let key = format!("batch_memory_key_{}", i);
            let value = Value::String(format!("batch_memory_value_{}", i));
            batch_data.push((key, value));
        }

        let stats_before = engine.stats().await.unwrap();
        let memory_before = stats_before.memory_usage;

        engine.batch_put(batch_data).await.unwrap();

        let stats_after = engine.stats().await.unwrap();
        let memory_after = stats_after.memory_usage;

        // Memory usage should increase reasonably with batch size
        let memory_increase = memory_after - memory_before;
        let expected_increase = batch_size * 100; // Rough estimate

        assert!(
            memory_increase <= expected_increase * 2,
            "Memory usage increase too high for batch size {}: {} > {}",
            batch_size,
            memory_increase,
            expected_increase * 2
        );

        // Clean up
        engine.clear().await.unwrap();
    }
}

/// Test memory usage with different data types
#[tokio::test]
async fn test_memory_usage_data_types() {
    let engine = F4KVSCore::new().unwrap();
    let iterations = 1000;

    let data_types = vec![
        ("string", Value::String("test_string".to_string())),
        ("integer", Value::Int64(42)),
        ("float", Value::Float64(std::f64::consts::PI)),
        ("boolean", Value::Bool(true)),
        ("bytes", Value::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])),
    ];

    let mut memory_usage = Vec::new();

    for (type_name, value) in data_types {
        for i in 0..iterations {
            let key = format!("type_test_key_{}_{}", type_name, i);
            engine.put(&key, &value).await.unwrap();
        }

        let stats = engine.stats().await.unwrap();
        memory_usage.push((type_name, stats.memory_usage));

        // Clean up
        engine.clear().await.unwrap();
    }

    // Verify memory usage is reasonable for all data types
    for (type_name, memory) in memory_usage {
        assert!(
            memory < 50 * 1024 * 1024, // 50MB limit
            "Memory usage too high for {}: {}",
            type_name,
            memory
        );
    }
}
