//! Comprehensive tests for concurrent batch operation failures
//!
//! These tests verify that batch operations handle failures correctly
//! when operations fail concurrently or partially.

use f4kvs_core::{engine::F4KVSCore, error::F4KvsError, Value};
use std::sync::Arc;
use tokio::sync::Barrier;

/// Test concurrent batch operations with some failures
#[tokio::test]
async fn test_concurrent_batch_put_with_failures() {
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));

    // Create batch operations with some invalid keys (too long)
    let mut handles = Vec::new();
    let barrier = Arc::new(Barrier::new(10));

    for batch_id in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let barrier_clone = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            // Wait for all tasks to start simultaneously
            barrier_clone.wait().await;

            // Create batch with mix of valid and invalid keys
            let mut items = Vec::new();

            // Valid items
            for i in 0..20 {
                let key = format!("batch_{}_key_{}", batch_id, i);
                let value = Value::String(format!("value_{}_{}", batch_id, i));
                items.push((key, value));
            }

            // Some batches have invalid keys (too long)
            if batch_id % 3 == 0 {
                // Add a key that's too long (should fail)
                let long_key = "x".repeat(10000); // Assuming max key size is smaller
                let value = Value::String("too_long".to_string());
                items.push((long_key, value));
            }

            // Execute batch put
            engine_clone.batch_put(items).await
        });

        handles.push(handle);
    }

    // Wait for all operations
    let mut success_count = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(_)) => {
                // Some batches may fail due to invalid keys - this is expected
            }
            Err(_) => panic!("Task panicked"),
        }
    }

    // Some batches should succeed, some may fail
    assert!(success_count > 0, "At least some batches should succeed");

    // Verify successful batches wrote data
    for batch_id in 0..10 {
        if batch_id % 3 != 0 {
            // These batches should have succeeded
            for i in 0..20 {
                let key = format!("batch_{}_key_{}", batch_id, i);
                let expected_value = Value::String(format!("value_{}_{}", batch_id, i));
                let result = engine.get(&key).await.expect("Get should succeed");
                assert_eq!(
                    result,
                    Some(expected_value),
                    "Batch data should be readable"
                );
            }
        }
    }
}

/// Test batch operations with concurrent reads during failures
#[tokio::test]
async fn test_batch_operations_with_concurrent_reads() {
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));

    // First, write some data
    let mut initial_items = Vec::new();
    for i in 0..100 {
        let key = format!("initial_key_{}", i);
        let value = Value::String(format!("initial_value_{}", i));
        initial_items.push((key, value));
    }
    engine
        .batch_put(initial_items)
        .await
        .expect("Initial batch should succeed");

    // Now perform concurrent batch operations and reads
    let mut write_handles = Vec::new();
    let mut read_handles = Vec::new();
    let barrier = Arc::new(Barrier::new(11)); // 10 writers + 1 reader coordinator

    // Spawn batch writers
    for batch_id in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let barrier_clone = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            // Create batch
            let mut items = Vec::new();
            for i in 0..50 {
                let key = format!("concurrent_batch_{}_key_{}", batch_id, i);
                let value = Value::String(format!("concurrent_value_{}_{}", batch_id, i));
                items.push((key, value));
            }

            engine_clone.batch_put(items).await
        });

        write_handles.push(handle);
    }

    // Spawn concurrent readers
    let engine_read = Arc::clone(&engine);
    let barrier_read = Arc::clone(&barrier);
    let read_handle = tokio::spawn(async move {
        barrier_read.wait().await;

        // Read from initial data while batches are being written
        for _round in 0..5 {
            for i in 0..100 {
                let key = format!("initial_key_{}", i);
                let expected_value = Value::String(format!("initial_value_{}", i));
                let result = engine_read.get(&key).await.expect("Get should succeed");
                assert_eq!(
                    result,
                    Some(expected_value),
                    "Initial data should remain readable during concurrent batches"
                );
            }
        }

        Ok::<(), F4KvsError>(())
    });

    read_handles.push(read_handle);

    // Wait for all operations
    for handle in write_handles {
        let result = handle.await.expect("Task should complete");
        assert!(result.is_ok(), "Batch writes should succeed");
    }

    for handle in read_handles {
        handle
            .await
            .expect("Read task should complete")
            .expect("Reads should succeed");
    }

    // Verify all batch data was written
    for batch_id in 0..10 {
        for i in 0..50 {
            let key = format!("concurrent_batch_{}_key_{}", batch_id, i);
            let expected_value = Value::String(format!("concurrent_value_{}_{}", batch_id, i));
            let result = engine.get(&key).await.expect("Get should succeed");
            assert_eq!(
                result,
                Some(expected_value),
                "Batch data should be readable after concurrent operations"
            );
        }
    }
}

/// Test batch get with non-existent keys
#[tokio::test]
async fn test_batch_get_with_missing_keys() {
    let engine = F4KVSCore::new().expect("Failed to create engine");

    // Write some keys
    let mut items = Vec::new();
    for i in 0..50 {
        let key = format!("batch_get_key_{}", i);
        let value = Value::String(format!("batch_get_value_{}", i));
        items.push((key, value));
    }
    engine
        .batch_put(items)
        .await
        .expect("Batch put should succeed");

    // Batch get with mix of existing and non-existing keys
    let mut keys = Vec::new();

    // Existing keys
    for i in 0..50 {
        keys.push(format!("batch_get_key_{}", i));
    }

    // Non-existing keys
    for i in 50..100 {
        keys.push(format!("batch_get_key_{}", i));
    }

    let results = engine
        .batch_get(keys)
        .await
        .expect("Batch get should succeed");

    // Verify results
    assert_eq!(results.len(), 100, "Should have 100 results");

    for (i, result) in results.iter().enumerate() {
        if i < 50 {
            // Existing keys should have values
            assert!(result.is_some(), "Key {} should exist", i);
            if let Some(value) = result {
                match value {
                    Value::String(s) => {
                        assert_eq!(s, &format!("batch_get_value_{}", i));
                    }
                    _ => panic!("Expected String value"),
                }
            }
        } else {
            // Non-existing keys should be None
            assert!(result.is_none(), "Key {} should not exist", i);
        }
    }
}

/// Test batch delete with concurrent operations
#[tokio::test]
async fn test_batch_delete_with_concurrent_operations() {
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));

    // Write initial data
    let mut items = Vec::new();
    for i in 0..200 {
        let key = format!("delete_test_key_{}", i);
        let value = Value::String(format!("delete_test_value_{}", i));
        items.push((key, value));
    }
    engine
        .batch_put(items)
        .await
        .expect("Initial batch put should succeed");

    // Concurrent batch deletes
    let mut delete_handles = Vec::new();
    let barrier = Arc::new(Barrier::new(5));

    for batch_id in 0..5 {
        let engine_clone = Arc::clone(&engine);
        let barrier_clone = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            // Delete keys for this batch
            let mut keys = Vec::new();
            for i in 0..40 {
                let key = format!("delete_test_key_{}", batch_id * 40 + i);
                keys.push(key);
            }

            engine_clone.batch_delete(keys).await
        });

        delete_handles.push(handle);
    }

    // Wait for all deletes
    for handle in delete_handles {
        handle
            .await
            .expect("Delete task should complete")
            .expect("Batch delete should succeed");
    }

    // Verify keys were deleted
    for i in 0..200 {
        let key = format!("delete_test_key_{}", i);
        let result = engine.get(&key).await.expect("Get should succeed");
        assert!(result.is_none(), "Key {} should be deleted", i);
    }
}

/// Test batch operations under high load
#[tokio::test]
async fn test_batch_operations_under_load() {
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));

    // High load: many concurrent batch operations
    let mut handles = Vec::new();
    let num_batches = 50;
    let items_per_batch = 100;

    for batch_id in 0..num_batches {
        let engine_clone = Arc::clone(&engine);

        let handle = tokio::spawn(async move {
            // Create batch
            let mut items = Vec::new();
            for i in 0..items_per_batch {
                let key = format!("load_test_batch_{}_key_{}", batch_id, i);
                let value = Value::String(format!("load_test_value_{}_{}", batch_id, i));
                items.push((key, value));
            }

            engine_clone.batch_put(items).await
        });

        handles.push(handle);
    }

    // Wait for all operations
    let mut success_count = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => panic!("Batch operation failed: {:?}", e),
            Err(e) => panic!("Task panicked: {:?}", e),
        }
    }

    assert_eq!(success_count, num_batches, "All batches should succeed");

    // Verify all data was written
    for batch_id in 0..num_batches {
        for i in 0..items_per_batch {
            let key = format!("load_test_batch_{}_key_{}", batch_id, i);
            let expected_value = Value::String(format!("load_test_value_{}_{}", batch_id, i));
            let result = engine.get(&key).await.expect("Get should succeed");
            assert_eq!(
                result,
                Some(expected_value),
                "Data should be readable after load test"
            );
        }
    }
}

/// Test batch operations with mixed value types
#[tokio::test]
async fn test_batch_operations_mixed_types() {
    let engine = F4KVSCore::new().expect("Failed to create engine");

    // Create batch with different value types
    let mut items = Vec::new();

    items.push((
        "string_key".to_string(),
        Value::String("string_value".to_string()),
    ));
    items.push(("int_key".to_string(), Value::Int64(42)));
    items.push(("uint_key".to_string(), Value::UInt64(100)));
    items.push(("float_key".to_string(), Value::Float64(3.14)));
    items.push(("bool_key".to_string(), Value::Bool(true)));
    items.push((
        "bytes_key".to_string(),
        Value::Bytes(b"bytes_value".to_vec()),
    ));
    items.push(("null_key".to_string(), Value::Null));

    engine
        .batch_put(items)
        .await
        .expect("Batch put should succeed");

    // Verify all values
    let keys = vec![
        "string_key".to_string(),
        "int_key".to_string(),
        "uint_key".to_string(),
        "float_key".to_string(),
        "bool_key".to_string(),
        "bytes_key".to_string(),
        "null_key".to_string(),
    ];

    let results = engine
        .batch_get(keys)
        .await
        .expect("Batch get should succeed");

    assert_eq!(results.len(), 7);
    assert_eq!(results[0], Some(Value::String("string_value".to_string())));
    assert_eq!(results[1], Some(Value::Int64(42)));
    assert_eq!(results[2], Some(Value::UInt64(100)));
    assert_eq!(results[3], Some(Value::Float64(3.14)));
    assert_eq!(results[4], Some(Value::Bool(true)));
    assert_eq!(results[5], Some(Value::Bytes(b"bytes_value".to_vec())));
    assert_eq!(results[6], Some(Value::Null));
}

/// Test batch operations with empty batches
#[tokio::test]
async fn test_batch_operations_empty_batches() {
    let engine = F4KVSCore::new().expect("Failed to create engine");

    // Empty batch put should succeed (no-op)
    let empty_items: Vec<(String, Value)> = Vec::new();
    engine
        .batch_put(empty_items)
        .await
        .expect("Empty batch put should succeed");

    // Empty batch get should return empty vector
    let empty_keys: Vec<String> = Vec::new();
    let results = engine
        .batch_get(empty_keys)
        .await
        .expect("Empty batch get should succeed");
    assert_eq!(
        results.len(),
        0,
        "Empty batch get should return empty results"
    );

    // Empty batch delete should succeed (no-op)
    let empty_delete_keys: Vec<String> = Vec::new();
    engine
        .batch_delete(empty_delete_keys)
        .await
        .expect("Empty batch delete should succeed");
}

/// Test batch operations with very large batches
#[tokio::test]
async fn test_batch_operations_large_batch() {
    let engine = F4KVSCore::new().expect("Failed to create engine");

    // Create a large batch (1000 items)
    let mut items = Vec::new();
    for i in 0..1000 {
        let key = format!("large_batch_key_{}", i);
        let value = Value::String(format!("large_batch_value_{}", i));
        items.push((key, value));
    }

    engine
        .batch_put(items)
        .await
        .expect("Large batch put should succeed");

    // Verify random samples
    let test_indices = vec![0, 100, 500, 999];
    for &i in &test_indices {
        let key = format!("large_batch_key_{}", i);
        let expected_value = Value::String(format!("large_batch_value_{}", i));
        let result = engine.get(&key).await.expect("Get should succeed");
        assert_eq!(
            result,
            Some(expected_value),
            "Large batch item {} should be readable",
            i
        );
    }

    // Batch get all items
    let mut keys = Vec::new();
    for i in 0..1000 {
        keys.push(format!("large_batch_key_{}", i));
    }

    let results = engine
        .batch_get(keys)
        .await
        .expect("Large batch get should succeed");
    assert_eq!(results.len(), 1000, "Should have 1000 results");

    // Verify all results are Some
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_some(), "Result {} should be Some", i);
        if let Some(value) = result {
            match value {
                Value::String(s) => {
                    assert_eq!(s, &format!("large_batch_value_{}", i));
                }
                _ => panic!("Expected String value"),
            }
        }
    }
}
