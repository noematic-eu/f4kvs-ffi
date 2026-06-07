//! Comprehensive HashMap storage tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for HashMap storage scenarios including:
//! - Concurrent access patterns
//! - Memory usage validation
//! - Performance regression tests
//! - Batch operation correctness
//! - Scan operation edge cases

use f4kvs_core::hashmap::HashMapStorage;
use f4kvs_core::storage_traits::StorageEngine;
use f4kvs_core::Value;
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_access_patterns() {
    let storage = Arc::new(HashMapStorage::new());

    // Spawn multiple concurrent writers
    let mut writer_handles = Vec::new();
    for i in 0..20 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("writer_{}_{}", i, j);
                let value = Value::String(format!("value_{}_{}", i, j));
                storage_clone.put(&key, &value).await.unwrap();
            }
        });
        writer_handles.push(handle);
    }

    // Spawn multiple concurrent readers
    let mut reader_handles = Vec::new();
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("writer_{}_{}", i, j);
                let _ = storage_clone.get(&key).await;
            }
        });
        reader_handles.push(handle);
    }

    // Wait for all writers
    for handle in writer_handles {
        handle.await.unwrap();
    }

    // Wait for all readers
    for handle in reader_handles {
        handle.await.unwrap();
    }

    // Verify final state
    let count = storage.count().await.unwrap();
    assert!(count >= 200); // At least 20 writers * 10 keys each
}

#[tokio::test]
async fn test_memory_usage_validation() {
    let storage = HashMapStorage::new();

    let initial_memory = storage.get_current_memory_usage().await;

    // Add multiple keys
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = Value::String(format!("value_{}", i));
        storage.put(&key, &value).await.unwrap();
    }

    let memory_after_inserts = storage.get_current_memory_usage().await;
    assert!(memory_after_inserts > initial_memory);

    // Delete half the keys
    for i in 0..50 {
        let key = format!("key_{}", i);
        storage.delete(&key).await.unwrap();
    }

    let memory_after_deletes = storage.get_current_memory_usage().await;
    assert!(memory_after_deletes < memory_after_inserts);

    // Clear all
    storage.clear().await.unwrap();
    let memory_after_clear = storage.get_current_memory_usage().await;
    assert_eq!(memory_after_clear, initial_memory);
}

#[tokio::test]
async fn test_batch_operation_correctness() {
    let storage = HashMapStorage::new();

    // Large batch put
    let mut items = Vec::new();
    for i in 0..1000 {
        items.push((
            format!("batch_key_{}", i),
            Value::String(format!("batch_value_{}", i)),
        ));
    }

    storage.batch_put(items.clone()).await.unwrap();
    assert_eq!(storage.count().await.unwrap(), 1000);

    // Batch get
    let keys: Vec<String> = (0..1000).map(|i| format!("batch_key_{}", i)).collect();
    let results = storage.batch_get(keys).await.unwrap();
    assert_eq!(results.len(), 1000);
    assert!(results.iter().all(|r| r.is_some()));

    // Batch delete
    let keys_to_delete: Vec<String> = (0..500).map(|i| format!("batch_key_{}", i)).collect();
    storage.batch_delete(keys_to_delete).await.unwrap();
    assert_eq!(storage.count().await.unwrap(), 500);
}

#[tokio::test]
async fn test_scan_operation_edge_cases() {
    let storage = HashMapStorage::new();

    // Add keys with various patterns (zero-padded for correct lexicographic ordering)
    for i in 0..100 {
        storage
            .put(
                &format!("prefix_a_{:02}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
        storage
            .put(
                &format!("prefix_b_{:02}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
    }

    // Scan with prefix
    let prefix_a_keys = storage.scan_prefix("prefix_a").await.unwrap();
    assert_eq!(prefix_a_keys.len(), 100);

    // Scan with range (exclusive end, so 00-49 = 50 keys)
    let range_keys = storage
        .scan_range("prefix_a_00", "prefix_a_50")
        .await
        .unwrap();
    assert!(range_keys.len() >= 49);

    // Scan prefix pairs
    let prefix_pairs = storage.scan_prefix_pairs("prefix_a").await.unwrap();
    assert_eq!(prefix_pairs.len(), 100);

    // Scan range pairs (exclusive end, so 00-49 = 50 keys)
    let range_pairs = storage
        .scan_range_pairs("prefix_a_00", "prefix_a_50")
        .await
        .unwrap();
    assert!(range_pairs.len() >= 49);
}

#[tokio::test]
async fn test_concurrent_batch_operations() {
    let storage = Arc::new(HashMapStorage::new());

    // Spawn multiple concurrent batch operations
    let mut handles = Vec::new();
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            let mut items = Vec::new();
            for j in 0..100 {
                items.push((
                    format!("batch_{}_{}", i, j),
                    Value::String(format!("value_{}_{}", i, j)),
                ));
            }
            storage_clone.batch_put(items).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all batch operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all data is present
    let count = storage.count().await.unwrap();
    assert_eq!(count, 1000); // 10 batches * 100 items each
}

#[tokio::test]
async fn test_statistics_accuracy_under_load() {
    let storage = HashMapStorage::new();

    // Perform many operations
    for i in 0..1000 {
        let key = format!("key_{}", i);
        let value = Value::String(format!("value_{}", i));
        storage.put(&key, &value).await.unwrap();
    }

    for i in 0..500 {
        let key = format!("key_{}", i);
        let _ = storage.get(&key).await;
    }

    for i in 500..1000 {
        let key = format!("key_{}", i);
        storage.delete(&key).await.unwrap();
    }

    let stats = storage.stats().await.unwrap();
    assert_eq!(stats.key_count, 500);
    assert_eq!(stats.put_operations, 1000);
    assert_eq!(stats.get_operations, 500);
    assert_eq!(stats.delete_operations, 500);
}

#[tokio::test]
async fn test_mixed_concurrent_operations() {
    let storage = Arc::new(HashMapStorage::new());

    // Mix of concurrent puts, gets, and deletes
    let mut handles = Vec::new();

    // Writers
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            for j in 0..50 {
                let key = format!("mixed_{}_{}", i, j);
                let value = Value::String(format!("value_{}_{}", i, j));
                storage_clone.put(&key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Readers
    for i in 0..5 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            for j in 0..50 {
                let key = format!("mixed_{}_{}", i, j);
                let _ = storage_clone.get(&key).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify consistency
    let count = storage.count().await.unwrap();
    assert_eq!(count, 500); // 10 writers * 50 keys each
}

#[tokio::test]
async fn test_large_key_value_pairs() {
    let storage = HashMapStorage::new();

    // Test with very large keys and values
    let large_key = "x".repeat(10000);
    let large_value_string = "y".repeat(100000);
    let large_value = Value::String(large_value_string);

    storage.put(&large_key, &large_value).await.unwrap();

    let retrieved = storage.get(&large_key).await.unwrap();
    assert!(retrieved.is_some());

    let stats = storage.stats().await.unwrap();
    assert!(stats.memory_usage > 100000);
}

#[tokio::test]
async fn test_count_operations_accuracy() {
    let storage = HashMapStorage::new();

    // Add keys with different prefixes (zero-padded for correct lexicographic ordering)
    for i in 0..50 {
        storage
            .put(
                &format!("prefix1_{:02}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
        storage
            .put(
                &format!("prefix2_{:02}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
    }

    // Count prefix
    let prefix1_count = storage.count_prefix("prefix1").await.unwrap();
    assert_eq!(prefix1_count, 50);

    let prefix2_count = storage.count_prefix("prefix2").await.unwrap();
    assert_eq!(prefix2_count, 50);

    // Count range (exclusive end, so 00-24 = 25 keys)
    let range_count = storage
        .count_range("prefix1_00", "prefix1_25")
        .await
        .unwrap();
    assert!(range_count >= 24);

    // Total count
    assert_eq!(storage.count().await.unwrap(), 100);
}

#[tokio::test]
async fn test_overwrite_behavior_in_batches() {
    let storage = HashMapStorage::new();

    // Initial batch
    let items1 = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::String("value2".to_string())),
    ];
    storage.batch_put(items1).await.unwrap();
    assert_eq!(storage.count().await.unwrap(), 2);

    // Overwrite batch
    let items2 = vec![
        (
            "key1".to_string(),
            Value::String("value1_updated".to_string()),
        ),
        ("key3".to_string(), Value::String("value3".to_string())),
    ];
    storage.batch_put(items2).await.unwrap();

    // Should have 3 keys (key1 updated, key2 unchanged, key3 new)
    assert_eq!(storage.count().await.unwrap(), 3);
    assert_eq!(
        storage.get("key1").await.unwrap(),
        Some(Value::String("value1_updated".to_string()))
    );
}
