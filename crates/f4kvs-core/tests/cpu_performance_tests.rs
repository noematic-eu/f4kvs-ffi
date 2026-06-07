//! CPU usage and performance tests for F4KVS Core
//!
//! These tests monitor CPU usage, performance characteristics,
//! and resource utilization under different workloads.

use f4kvs_core::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test CPU usage under sustained load
#[tokio::test]
async fn test_cpu_usage_sustained_load() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(10); // 10 seconds for testing
    let start_time = Instant::now();

    let mut operation_count = 0;
    let mut handles = Vec::new();

    // Spawn multiple tasks to create CPU load
    for task_id in 0..5 {
        let engine_clone = engine.clone();
        handles.push(tokio::spawn(async move {
            let mut task_operations = 0;

            while start_time.elapsed() < test_duration {
                for i in 0..100 {
                    let key = format!("cpu_test_key_{}_{}", task_id, i);
                    let value = Value::String(format!("cpu_test_value_{}_{}", task_id, i));

                    engine_clone.put(&key, &value).await.unwrap();
                    engine_clone.get(&key).await.unwrap();
                    engine_clone.delete(&key).await.unwrap();

                    task_operations += 1;
                }

                // Small delay to prevent overwhelming
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            task_operations
        }));
    }

    // Wait for all tasks to complete
    for handle in handles {
        operation_count += handle.await.unwrap();
    }

    let total_duration = start_time.elapsed();
    let throughput = operation_count as f64 / total_duration.as_secs_f64();

    // Throughput should be reasonable
    assert!(
        throughput > 1000.0,
        "CPU performance too low: {:.2} ops/sec",
        throughput
    );
}

/// Test CPU usage with different operation types
#[tokio::test]
async fn test_cpu_usage_operation_types() {
    let engine = F4KVSCore::new().unwrap();
    let operation_count = 1000;

    // Test PUT operations
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_put_key_{}", i);
        let value = Value::String(format!("cpu_put_value_{}", i));
        engine.put(&key, &value).await.unwrap();
    }
    let put_duration = start_time.elapsed();
    let put_throughput = operation_count as f64 / put_duration.as_secs_f64();
    assert!(
        put_throughput > 1000.0,
        "PUT throughput too low: {:.2} ops/sec",
        put_throughput
    );

    // Test GET operations
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_put_key_{}", i);
        let _ = engine.get(&key).await.unwrap();
    }
    let get_duration = start_time.elapsed();
    let get_throughput = operation_count as f64 / get_duration.as_secs_f64();
    assert!(
        get_throughput > 2000.0,
        "GET throughput too low: {:.2} ops/sec",
        get_throughput
    );

    // Test DELETE operations
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_put_key_{}", i);
        engine.delete(&key).await.unwrap();
    }
    let delete_duration = start_time.elapsed();
    let delete_throughput = operation_count as f64 / delete_duration.as_secs_f64();
    assert!(
        delete_throughput > 1500.0,
        "DELETE throughput too low: {:.2} ops/sec",
        delete_throughput
    );
}

/// Test CPU usage with batch operations
#[tokio::test]
async fn test_cpu_usage_batch_operations() {
    let engine = F4KVSCore::new().unwrap();
    let batch_sizes = vec![10, 50, 100, 500];

    for batch_size in batch_sizes {
        let mut batch_data = Vec::new();

        // Prepare batch data
        for i in 0..batch_size {
            let key = format!("cpu_batch_key_{}_{}", batch_size, i);
            let value = Value::String(format!("cpu_batch_value_{}_{}", batch_size, i));
            batch_data.push((key, value));
        }

        let start_time = Instant::now();
        engine.batch_put(batch_data).await.unwrap();
        let duration = start_time.elapsed();

        let throughput = batch_size as f64 / duration.as_secs_f64();

        // Batch operations should be efficient
        assert!(
            throughput > 100.0,
            "Batch operation throughput too low for size {}: {:.2} ops/sec",
            batch_size,
            throughput
        );

        // Clean up
        engine.clear().await.unwrap();
    }
}

/// Test CPU usage with concurrent operations
#[tokio::test]
async fn test_cpu_usage_concurrent_operations() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let concurrent_tasks = 20;
    let operations_per_task = 100;

    let start_time = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.push(tokio::spawn(async move {
            let mut task_operations = 0;

            for i in 0..operations_per_task {
                let key = format!("cpu_concurrent_key_{}_{}", task_id, i);
                let value = Value::String(format!("cpu_concurrent_value_{}_{}", task_id, i));

                engine_clone.put(&key, &value).await.unwrap();
                engine_clone.get(&key).await.unwrap();
                engine_clone.delete(&key).await.unwrap();

                task_operations += 1;
            }

            task_operations
        }));
    }

    // Wait for all tasks to complete
    let mut total_operations = 0;
    for handle in handles {
        total_operations += handle.await.unwrap();
    }

    let duration = start_time.elapsed();
    let throughput = total_operations as f64 / duration.as_secs_f64();

    // Concurrent operations should scale well
    assert!(
        throughput > 2000.0,
        "Concurrent operation throughput too low: {:.2} ops/sec",
        throughput
    );
}

/// Test CPU usage with different data sizes
#[tokio::test]
async fn test_cpu_usage_data_sizes() {
    let engine = F4KVSCore::new().unwrap();
    let data_sizes = vec![
        1024,        // 1KB
        1024 * 10,   // 10KB
        1024 * 100,  // 100KB
        1024 * 1024, // 1MB
    ];

    for data_size in data_sizes {
        let operation_count = 100;
        let mut throughputs = Vec::new();

        // Test multiple iterations for each data size
        for iteration in 0..3 {
            let start_time = Instant::now();

            for i in 0..operation_count {
                let key = format!("cpu_size_key_{}_{}_{}", data_size, iteration, i);
                let value = Value::String("x".repeat(data_size));

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

        // Throughput should scale reasonably with data size
        let expected_throughput = match data_size {
            1024 => 1000.0,  // 1KB: 1000 ops/sec
            10240 => 500.0,  // 10KB: 500 ops/sec
            102400 => 100.0, // 100KB: 100 ops/sec
            1048576 => 10.0, // 1MB: 10 ops/sec
            _ => 1.0,
        };

        assert!(
            avg_throughput >= expected_throughput * 0.3,
            "CPU performance too low for data size {}: {:.2} < {:.2} ops/sec",
            data_size,
            avg_throughput,
            expected_throughput * 0.3
        );
    }
}

/// Test CPU usage with different key patterns
#[tokio::test]
async fn test_cpu_usage_key_patterns() {
    let engine = F4KVSCore::new().unwrap();
    let operation_count = 1000;

    // Test sequential keys
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_seq_key_{:06}", i);
        let value = Value::String(format!("cpu_seq_value_{}", i));

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let seq_duration = start_time.elapsed();
    let seq_throughput = operation_count as f64 / seq_duration.as_secs_f64();
    assert!(
        seq_throughput > 500.0,
        "Sequential key throughput too low: {:.2} ops/sec",
        seq_throughput
    );

    // Test short keys
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("k{}", i);
        let value = Value::String(format!("cpu_short_value_{}", i));

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let short_duration = start_time.elapsed();
    let short_throughput = operation_count as f64 / short_duration.as_secs_f64();
    assert!(
        short_throughput > 500.0,
        "Short key throughput too low: {:.2} ops/sec",
        short_throughput
    );

    // Test long keys
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_very_long_key_name_with_many_characters_{:06}", i);
        let value = Value::String(format!("cpu_long_value_{}", i));

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let long_duration = start_time.elapsed();
    let long_throughput = operation_count as f64 / long_duration.as_secs_f64();
    assert!(
        long_throughput > 500.0,
        "Long key throughput too low: {:.2} ops/sec",
        long_throughput
    );
}

/// Test CPU usage with different value types
#[tokio::test]
async fn test_cpu_usage_value_types() {
    let engine = F4KVSCore::new().unwrap();
    let operation_count = 1000;

    // Test string values
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_string_key_{}", i);
        let value = Value::String(format!("cpu_string_value_{}", i));

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let string_duration = start_time.elapsed();
    let string_throughput = operation_count as f64 / string_duration.as_secs_f64();
    assert!(
        string_throughput > 500.0,
        "String value throughput too low: {:.2} ops/sec",
        string_throughput
    );

    // Test integer values
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_int_key_{}", i);
        let value = Value::Int64(i as i64);

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let int_duration = start_time.elapsed();
    let int_throughput = operation_count as f64 / int_duration.as_secs_f64();
    assert!(
        int_throughput > 500.0,
        "Integer value throughput too low: {:.2} ops/sec",
        int_throughput
    );

    // Test boolean values
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_bool_key_{}", i);
        let value = Value::Bool(i % 2 == 0);

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let bool_duration = start_time.elapsed();
    let bool_throughput = operation_count as f64 / bool_duration.as_secs_f64();
    assert!(
        bool_throughput > 500.0,
        "Boolean value throughput too low: {:.2} ops/sec",
        bool_throughput
    );

    // Test bytes values
    let start_time = Instant::now();
    for i in 0..operation_count {
        let key = format!("cpu_bytes_key_{}", i);
        let value = Value::Bytes(vec![i as u8; 100]);

        engine.put(&key, &value).await.unwrap();
        engine.get(&key).await.unwrap();
        engine.delete(&key).await.unwrap();
    }
    let bytes_duration = start_time.elapsed();
    let bytes_throughput = operation_count as f64 / bytes_duration.as_secs_f64();
    assert!(
        bytes_throughput > 500.0,
        "Bytes value throughput too low: {:.2} ops/sec",
        bytes_throughput
    );
}

/// Test CPU usage under memory pressure
#[tokio::test]
async fn test_cpu_usage_memory_pressure() {
    let engine = F4KVSCore::new().unwrap();
    let large_value_size = 1024 * 1024; // 1MB
    let operation_count = 50;

    let start_time = Instant::now();

    for i in 0..operation_count {
        let key = format!("cpu_pressure_key_{}", i);
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
    let throughput = operation_count as f64 / duration.as_secs_f64();

    // CPU performance should be reasonable even under memory pressure
    assert!(
        throughput > 1.0,
        "CPU performance too low under memory pressure: {:.2} ops/sec",
        throughput
    );
}

/// Test CPU usage with timeout operations
#[tokio::test]
async fn test_cpu_usage_timeout_operations() {
    let engine = F4KVSCore::new().unwrap();
    let operation_count = 100;
    let timeout_duration = Duration::from_millis(100);

    let start_time = Instant::now();

    for i in 0..operation_count {
        let key = format!("cpu_timeout_key_{}", i);
        let value = Value::String(format!("cpu_timeout_value_{}", i));

        // Operations should complete within timeout
        let result = tokio::time::timeout(timeout_duration, engine.put(&key, &value)).await;
        assert!(result.is_ok());

        let result = tokio::time::timeout(timeout_duration, engine.get(&key)).await;
        assert!(result.is_ok());

        let result = tokio::time::timeout(timeout_duration, engine.delete(&key)).await;
        assert!(result.is_ok());
    }

    let duration = start_time.elapsed();
    let throughput = operation_count as f64 / duration.as_secs_f64();

    // CPU performance should be reasonable with timeouts
    assert!(
        throughput > 100.0,
        "CPU performance too low with timeouts: {:.2} ops/sec",
        throughput
    );
}
