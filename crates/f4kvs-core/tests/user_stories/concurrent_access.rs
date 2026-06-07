//! User Story: Concurrent Access
//!
//! As a developer
//! I want to access the database concurrently
//! So that my application can handle multiple users

use f4kvs_core::{F4KVSCore, Value};
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_user_story_concurrent_reads() {
    // Given: A new F4KVS instance with data
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // Setup: Insert some data
    for i in 0..100 {
        let key = format!("read_key_{}", i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I perform 100+ concurrent read operations
    let num_readers = 100;
    let mut handles = Vec::new();

    for reader_id in 0..num_readers {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let key = format!("read_key_{}", reader_id % 100);
            engine_clone.get(&key).await
        });
        handles.push(handle);
    }

    // Then: All reads should succeed
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_readers,
        "All concurrent reads should succeed"
    );
}

#[tokio::test]
async fn test_user_story_concurrent_writes() {
    // Given: A new F4KVS instance
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // When: I perform 100+ concurrent write operations
    let num_writers = 100;
    let mut handles = Vec::new();

    for writer_id in 0..num_writers {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let key = format!("write_key_{}", writer_id);
            let value = Value::String(format!("value_{}", writer_id));
            engine_clone.put(&key, &value).await
        });
        handles.push(handle);
    }

    // Then: All writes should succeed
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_writers,
        "All concurrent writes should succeed"
    );

    // Verify all values were written correctly
    for writer_id in 0..num_writers {
        let key = format!("write_key_{}", writer_id);
        let retrieved = engine.get(&key).await.expect("Failed to get value");
        match retrieved {
            Some(Value::String(s)) => {
                assert_eq!(
                    s,
                    format!("value_{}", writer_id),
                    "Value should match for key: {}",
                    key
                );
            }
            _ => panic!("Value should be a String for key: {}", key),
        }
    }
}

#[tokio::test]
async fn test_user_story_concurrent_mixed() {
    // Given: A new F4KVS instance
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // Setup: Insert initial data
    for i in 0..50 {
        let key = format!("mixed_key_{}", i);
        engine
            .put(&key, &Value::String(format!("initial_value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I perform mixed read/write workload concurrently
    let num_operations = 200;
    let mut handles = Vec::new();

    for op_id in 0..num_operations {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            if op_id % 2 == 0 {
                // Read operation
                let key = format!("mixed_key_{}", op_id % 50);
                engine_clone.get(&key).await.map(|_| ())
            } else {
                // Write operation
                let key = format!("mixed_key_{}", op_id % 50);
                let value = Value::String(format!("updated_value_{}", op_id));
                engine_clone.put(&key, &value).await
            }
        });
        handles.push(handle);
    }

    // Then: All operations should complete
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_operations,
        "All mixed concurrent operations should succeed"
    );
}

#[tokio::test]
async fn test_user_story_concurrent_same_key() {
    // Given: A new F4KVS instance
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // When: Multiple writers write to the same key concurrently
    let shared_key = "shared_key";
    let num_writers = 50;
    let mut handles = Vec::new();

    for writer_id in 0..num_writers {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let value = Value::String(format!("value_from_writer_{}", writer_id));
            engine_clone.put(shared_key, &value).await
        });
        handles.push(handle);
    }

    // Then: All writes should complete (last write wins)
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_writers,
        "All concurrent writes to same key should succeed"
    );

    // Verify the key has a value (last writer wins)
    let retrieved = engine.get(shared_key).await.expect("Failed to get value");
    assert!(
        retrieved.is_some(),
        "Key should have a value after concurrent writes"
    );
}

#[tokio::test]
async fn test_user_story_concurrent_scan() {
    // Given: A new F4KVS instance with data
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // Setup: Insert data with different prefixes
    for i in 0..100 {
        let key = format!("scan_prefix_a:{}", i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    for i in 0..100 {
        let key = format!("scan_prefix_b:{}", i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I perform concurrent scan operations
    let num_scanners = 20;
    let mut handles = Vec::new();

    for scanner_id in 0..num_scanners {
        let engine_clone = engine.clone();
        let prefix = if scanner_id % 2 == 0 {
            "scan_prefix_a:"
        } else {
            "scan_prefix_b:"
        };
        let handle = tokio::spawn(async move { engine_clone.scan_prefix(prefix).await });
        handles.push(handle);
    }

    // Then: All scans should succeed
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            let scanned = result.unwrap();
            assert_eq!(scanned.len(), 100, "Each scan should find 100 keys");
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_scanners,
        "All concurrent scans should succeed"
    );
}

#[tokio::test]
async fn test_user_story_concurrent_batch_operations() {
    // Given: A new F4KVS instance
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // When: I perform concurrent batch operations
    let num_batches = 10;
    let batch_size = 100;
    let mut handles = Vec::new();

    for batch_id in 0..num_batches {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let mut batch_data = Vec::new();
            for i in 0..batch_size {
                let key = format!("batch_{}_key_{}", batch_id, i);
                let value = Value::String(format!("batch_{}_value_{}", batch_id, i));
                batch_data.push((key, value));
            }
            engine_clone.batch_put(batch_data).await
        });
        handles.push(handle);
    }

    // Then: All batch operations should succeed
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_batches,
        "All concurrent batch operations should succeed"
    );

    // Verify data was written correctly
    for batch_id in 0..num_batches {
        for i in 0..10 {
            // Sample check
            let key = format!("batch_{}_key_{}", batch_id, i);
            let retrieved = engine.get(&key).await.expect("Failed to get value");
            match retrieved {
                Some(Value::String(s)) => {
                    assert_eq!(
                        s,
                        format!("batch_{}_value_{}", batch_id, i),
                        "Value should match"
                    );
                }
                _ => panic!("Value should be a String"),
            }
        }
    }
}

#[tokio::test]
async fn test_user_story_concurrent_read_write_consistency() {
    // Given: A new F4KVS instance
    let engine = Arc::new(F4KVSCore::new().expect("Failed to create F4KVS instance"));

    // Setup: Insert initial value
    let test_key = "consistency_key";
    engine
        .put(test_key, &Value::String("initial".to_string()))
        .await
        .expect("Failed to put");

    // When: I perform concurrent reads and writes
    let num_operations = 100;
    let barrier = Arc::new(Barrier::new(num_operations + 1));
    let mut handles = Vec::new();

    for op_id in 0..num_operations {
        let engine_clone = engine.clone();
        let barrier_clone = barrier.clone();
        let handle = tokio::spawn(async move {
            // Wait for all tasks to start
            barrier_clone.wait().await;

            if op_id % 2 == 0 {
                // Read operation
                engine_clone.get(test_key).await.map(|_| ())
            } else {
                // Write operation
                let value = Value::String(format!("value_{}", op_id));
                engine_clone.put(test_key, &value).await
            }
        });
        handles.push(handle);
    }

    // Start all operations simultaneously
    barrier.wait().await;

    // Then: All operations should complete successfully
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(
        success_count, num_operations,
        "All concurrent read/write operations should succeed"
    );

    // Verify final state is consistent
    let final_value = engine
        .get(test_key)
        .await
        .expect("Failed to get final value");
    assert!(
        final_value.is_some(),
        "Key should have a value after concurrent operations"
    );
}
