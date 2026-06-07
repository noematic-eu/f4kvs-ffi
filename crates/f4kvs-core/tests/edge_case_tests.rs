//! Edge case tests for F4KVS Core
//!
//! These tests verify behavior under extreme conditions:
//! - Large values and keys
//! - High concurrency
//! - Memory pressure
//! - Error conditions

use f4kvs_core::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_large_values() {
    let engine = F4KVSCore::new().unwrap();
    let config = engine.config();

    // Test with values close to the maximum size
    let max_value_size = config.max_value_size;
    let test_sizes = vec![
        max_value_size / 2,
        max_value_size - 1000,
        max_value_size - 100,
    ];

    for size in test_sizes {
        let large_value = Value::String("x".repeat(size));
        let key = format!("large_value_{}", size);

        // Should succeed
        engine.put(&key, &large_value).await.unwrap();

        // Verify retrieval
        let retrieved = engine.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(large_value));

        // Clean up
        engine.delete(&key).await.unwrap();
    }

    // Test with value that exceeds maximum size
    let oversized_value = Value::String("x".repeat(max_value_size + 1));
    let result = engine.put("oversized_key", &oversized_value).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_large_keys() {
    let engine = F4KVSCore::new().unwrap();
    let config = engine.config();

    // Test with keys close to the maximum size
    let max_key_size = config.max_key_size;
    let test_sizes = vec![max_key_size / 2, max_key_size - 10, max_key_size - 1];

    for size in test_sizes {
        let large_key = "x".repeat(size);
        let value = Value::String("test_value".to_string());

        // Should succeed
        engine.put(&large_key, &value).await.unwrap();

        // Verify retrieval
        let retrieved = engine.get(&large_key).await.unwrap();
        assert_eq!(retrieved, Some(value));

        // Clean up
        engine.delete(&large_key).await.unwrap();
    }

    // Test with key that exceeds maximum size
    let oversized_key = "x".repeat(max_key_size + 1);
    let result = engine
        .put(&oversized_key, &Value::String("test".to_string()))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_pressure() {
    let engine = F4KVSCore::new().unwrap();

    // Fill up memory with many small values
    let num_keys = 10000;
    let value_size = 1000; // 1KB per value

    for i in 0..num_keys {
        let key = format!("memory_pressure_key_{}", i);
        let value = Value::String("x".repeat(value_size));
        engine.put(&key, &value).await.unwrap();
    }

    // Verify all values are stored
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, num_keys);
    assert!(stats.memory_usage > 0);

    // Test operations under memory pressure
    for i in 0..100 {
        let key = format!("memory_pressure_key_{}", i);
        let retrieved = engine.get(&key).await.unwrap();
        assert!(retrieved.is_some());
    }

    // Test batch operations under memory pressure
    let batch_keys: Vec<String> = (0..1000)
        .map(|i| format!("memory_pressure_key_{}", i))
        .collect();

    let batch_results = engine.batch_get(batch_keys).await.unwrap();
    assert_eq!(batch_results.len(), 1000);

    // Clean up
    engine.clear().await.unwrap();
    let final_stats = engine.stats().await.unwrap();
    assert_eq!(final_stats.key_count, 0);
}

#[tokio::test]
async fn test_high_concurrency() {
    let engine = Arc::new(F4KVSCore::new().unwrap());

    // Test with many concurrent operations
    let num_tasks = 100;
    let operations_per_task = 1000;

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            for i in 0..operations_per_task {
                let key = format!("concurrent_key_{}_{}", task_id, i);
                let value = Value::String(format!("concurrent_value_{}_{}", task_id, i));

                // Mix of operations
                match i % 3 {
                    0 => {
                        // Put
                        engine_clone.put(&key, &value).await.unwrap();
                    }
                    1 => {
                        // Get
                        let _ = engine_clone.get(&key).await;
                    }
                    2 => {
                        // Delete
                        let _ = engine_clone.delete(&key).await;
                    }
                    _ => unreachable!(),
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify engine is still functional
    assert!(engine.health_check().await.unwrap());
    let stats = engine.stats().await.unwrap();
    assert!(stats.total_operations > 0);
}

#[tokio::test]
async fn test_concurrent_memory_pressure() {
    let engine = Arc::new(F4KVSCore::new().unwrap());

    // Test concurrent operations under memory pressure
    let num_tasks = 20;
    let values_per_task = 500;

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            for i in 0..values_per_task {
                let key = format!("concurrent_memory_key_{}_{}", task_id, i);
                let value = Value::String("x".repeat(1000)); // 1KB value

                engine_clone.put(&key, &value).await.unwrap();

                // Occasionally read back
                if i % 10 == 0 {
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert!(retrieved.is_some());
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, num_tasks * values_per_task);
    assert!(stats.memory_usage > 0);

    // Verify engine is still functional
    assert!(engine.health_check().await.unwrap());
}

#[tokio::test]
async fn test_rapid_put_delete_cycles() {
    let engine = F4KVSCore::new().unwrap();

    // Test rapid put/delete cycles
    let num_cycles = 1000;

    for i in 0..num_cycles {
        let key = format!("rapid_cycle_key_{}", i);
        let value = Value::String(format!("rapid_cycle_value_{}", i));

        // Put
        engine.put(&key, &value).await.unwrap();

        // Verify
        let retrieved = engine.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(value));

        // Delete
        engine.delete(&key).await.unwrap();

        // Verify deletion
        let retrieved_after_delete = engine.get(&key).await.unwrap();
        assert!(retrieved_after_delete.is_none());
    }

    // Verify final state
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, 0);
}

#[tokio::test]
async fn test_mixed_value_types_under_load() {
    let engine = Arc::new(F4KVSCore::new().unwrap());

    // Test with mixed value types under concurrent load
    let num_tasks = 10;
    let operations_per_task = 100;

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            for i in 0..operations_per_task {
                let key = format!("mixed_type_key_{}_{}", task_id, i);

                // Use different value types
                let value = match i % 6 {
                    0 => Value::String(format!("string_value_{}_{}", task_id, i)),
                    1 => Value::Int64(i as i64),
                    2 => Value::Bool(i % 2 == 0),
                    3 => Value::Float64(i as f64 * std::f64::consts::PI),
                    4 => Value::Bytes(vec![i as u8; 10]),
                    5 => Value::Json(serde_json::json!({
                        "list_item": "value",
                        "number": i
                    })),
                    _ => unreachable!(),
                };

                engine_clone.put(&key, &value).await.unwrap();

                // Verify
                let retrieved = engine_clone.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, num_tasks * operations_per_task);
}

#[tokio::test]
async fn test_timeout_behavior() {
    let engine = F4KVSCore::new().unwrap();

    // Test that operations complete within reasonable time
    let timeout_duration = Duration::from_secs(5);

    // Test single operation timeout
    let result = timeout(
        timeout_duration,
        engine.put("timeout_key", &Value::String("test".to_string())),
    )
    .await;
    assert!(result.is_ok());

    // Test batch operation timeout
    let batch_items: Vec<(String, Value)> = (0..1000)
        .map(|i| {
            (
                format!("timeout_batch_key_{}", i),
                Value::String(format!("value_{}", i)),
            )
        })
        .collect();

    let result = timeout(timeout_duration, engine.batch_put(batch_items)).await;
    assert!(result.is_ok());

    // Test health check timeout
    let result = timeout(timeout_duration, engine.health_check()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_error_recovery() {
    let engine = F4KVSCore::new().unwrap();

    // Test that engine recovers from error conditions
    let config = engine.config();

    // Try to put a value that's too large
    let oversized_value = Value::String("x".repeat(config.max_value_size + 1));
    let result = engine.put("oversized_key", &oversized_value).await;
    assert!(result.is_err());

    // Engine should still work normally after error
    let normal_value = Value::String("normal_value".to_string());
    engine.put("normal_key", &normal_value).await.unwrap();

    let retrieved = engine.get("normal_key").await.unwrap();
    assert_eq!(retrieved, Some(normal_value));

    // Health check should still pass
    assert!(engine.health_check().await.unwrap());
}

#[tokio::test]
async fn test_stats_accuracy_under_load() {
    let engine = Arc::new(F4KVSCore::new().unwrap());

    // Test that stats remain accurate under concurrent load
    let num_tasks = 5;
    let operations_per_task = 100;

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            for i in 0..operations_per_task {
                let key = format!("stats_key_{}_{}", task_id, i);
                let value = Value::String(format!("stats_value_{}_{}", task_id, i));

                engine_clone.put(&key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify stats accuracy
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, num_tasks * operations_per_task);
    assert!(stats.memory_usage > 0);
    assert!(stats.total_operations > 0);

    // Test that stats are consistent across multiple reads
    let stats1 = engine.stats().await.unwrap();
    let stats2 = engine.stats().await.unwrap();
    assert_eq!(stats1.key_count, stats2.key_count);
    assert_eq!(stats1.memory_usage, stats2.memory_usage);
}

#[tokio::test]
async fn test_clear_under_load() {
    let engine = Arc::new(F4KVSCore::new().unwrap());

    // Pre-populate with data
    for i in 0..1000 {
        let key = format!("clear_load_key_{}", i);
        let value = Value::String(format!("clear_load_value_{}", i));
        engine.put(&key, &value).await.unwrap();
    }

    // Verify data is there
    let stats_before = engine.stats().await.unwrap();
    assert_eq!(stats_before.key_count, 1000);

    // Clear while other operations might be happening
    let clear_handle = {
        let engine_clone = Arc::clone(&engine);
        tokio::spawn(async move {
            engine_clone.clear().await.unwrap();
        })
    };

    // Wait for clear to complete
    clear_handle.await.unwrap();

    // Verify clear worked
    let stats_after = engine.stats().await.unwrap();
    assert_eq!(stats_after.key_count, 0);

    // Verify engine is still functional
    assert!(engine.health_check().await.unwrap());
}
