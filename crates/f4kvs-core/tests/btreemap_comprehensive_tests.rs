//! Comprehensive BTreeMap storage tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for BTreeMap storage scenarios including:
//! - Range query performance
//! - Concurrent range operations
//! - Large ordered dataset handling
//! - Memory efficiency validation

use f4kvs_core::btreemap::BTreeMapStorage;
use f4kvs_core::storage_traits::StorageEngine;
use f4kvs_core::Value;
use std::sync::Arc;
use std::time::Instant;

#[tokio::test]
async fn test_range_query_performance() {
    let storage = BTreeMapStorage::new();

    // Insert large ordered dataset
    for i in 0..10000 {
        let key = format!("key_{:05}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Measure range query performance
    let start = Instant::now();
    let range_keys = storage.scan_range("key_01000", "key_05000").await.unwrap();
    let duration = start.elapsed();

    assert_eq!(range_keys.len(), 4000);
    assert!(duration.as_millis() < 100); // Should be fast for BTreeMap
}

#[tokio::test]
async fn test_concurrent_range_operations() {
    let storage = Arc::new(BTreeMapStorage::new());

    // Add large dataset
    for i in 0..10000 {
        let key = format!("key_{:05}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Spawn multiple concurrent range queries
    let mut handles = Vec::new();
    for i in 0..20 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            let start_key = format!("key_{:05}", i * 500);
            let end_key = format!("key_{:05}", (i + 1) * 500);
            storage_clone.scan_range(&start_key, &end_key).await
        });
        handles.push(handle);
    }

    // Wait for all queries
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let keys = result.unwrap();
        assert!(!keys.is_empty());
    }
}

#[tokio::test]
async fn test_large_ordered_dataset_handling() {
    let storage = BTreeMapStorage::new();

    // Insert very large ordered dataset
    let count = 50000;
    for i in 0..count {
        let key = format!("key_{:06}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Verify ordering is maintained
    let keys = storage.keys().await.unwrap();
    assert_eq!(keys.len(), count as usize);

    // Verify keys are in sorted order
    for i in 1..keys.len() {
        assert!(keys[i - 1] < keys[i]);
    }

    // Verify range queries work correctly
    let range_keys = storage
        .scan_range("key_010000", "key_020000")
        .await
        .unwrap();
    assert_eq!(range_keys.len(), 10000);
}

#[tokio::test]
async fn test_memory_efficiency_validation() {
    let storage = BTreeMapStorage::new();

    let initial_memory = storage.get_current_memory_usage().await;

    // Add data
    for i in 0..1000 {
        let key = format!("key_{:04}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    let memory_after = storage.get_current_memory_usage().await;
    assert!(memory_after > initial_memory);

    // Delete half
    for i in 0..500 {
        let key = format!("key_{:04}", i);
        storage.delete(&key).await.unwrap();
    }

    let memory_after_delete = storage.get_current_memory_usage().await;
    assert!(memory_after_delete < memory_after);

    // Clear all
    storage.clear().await.unwrap();
    let memory_after_clear = storage.get_current_memory_usage().await;
    assert_eq!(memory_after_clear, initial_memory);
}

#[tokio::test]
async fn test_prefix_scan_performance() {
    let storage = BTreeMapStorage::new();

    // Add keys with various prefixes
    for i in 0..1000 {
        storage
            .put(
                &format!("prefix_a_{:04}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
        storage
            .put(
                &format!("prefix_b_{:04}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
    }

    // Measure prefix scan performance
    let start = Instant::now();
    let prefix_keys = storage.scan_prefix("prefix_a").await.unwrap();
    let duration = start.elapsed();

    assert_eq!(prefix_keys.len(), 1000);
    assert!(duration.as_millis() < 100);
}

#[tokio::test]
async fn test_ordered_batch_operations() {
    let storage = BTreeMapStorage::new();

    // Batch put with unordered keys
    let mut items = Vec::new();
    for i in (0..100).rev() {
        items.push((
            format!("key_{:03}", i),
            Value::String(format!("value_{}", i)),
        ));
    }

    storage.batch_put(items).await.unwrap();

    // Keys should be in sorted order
    let keys = storage.keys().await.unwrap();
    assert_eq!(keys.len(), 100);
    for i in 1..keys.len() {
        assert!(keys[i - 1] < keys[i]);
    }
}

#[tokio::test]
async fn test_range_query_edge_cases() {
    let storage = BTreeMapStorage::new();

    // Add keys
    for i in 0..100 {
        let key = format!("key_{:03}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Test various range queries
    let all_keys = storage.scan_range("key_000", "key_100").await.unwrap();
    assert_eq!(all_keys.len(), 100);

    let first_half = storage.scan_range("key_000", "key_050").await.unwrap();
    assert_eq!(first_half.len(), 50);

    let second_half = storage.scan_range("key_050", "key_100").await.unwrap();
    assert_eq!(second_half.len(), 50);

    // Empty range
    let empty = storage.scan_range("key_100", "key_000").await.unwrap();
    assert_eq!(empty.len(), 0);
}

#[tokio::test]
async fn test_concurrent_prefix_scans() {
    let storage = Arc::new(BTreeMapStorage::new());

    // Add data with multiple prefixes
    for prefix in ['a', 'b', 'c', 'd'] {
        for i in 0..100 {
            let key = format!("prefix_{}_{:03}", prefix, i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }
    }

    // Spawn concurrent prefix scans
    let mut handles = Vec::new();
    for prefix in ['a', 'b', 'c', 'd'] {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            storage_clone
                .scan_prefix(&format!("prefix_{}_", prefix))
                .await
        });
        handles.push(handle);
    }

    // Wait for all scans
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let keys = result.unwrap();
        assert_eq!(keys.len(), 100);
    }
}

#[tokio::test]
async fn test_large_range_query() {
    let storage = BTreeMapStorage::new();

    // Insert very large dataset
    for i in 0..100000 {
        let key = format!("key_{:06}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Large range query
    let start = Instant::now();
    let range_keys = storage
        .scan_range("key_000000", "key_050000")
        .await
        .unwrap();
    let duration = start.elapsed();

    assert_eq!(range_keys.len(), 50000);
    assert!(duration.as_secs() < 5); // Should complete in reasonable time
}

#[tokio::test]
async fn test_ordered_iteration_consistency() {
    let storage = BTreeMapStorage::new();

    // Add keys in random order
    let keys_to_add = vec!["zebra", "apple", "banana", "cherry", "date"];
    for key in &keys_to_add {
        storage
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .unwrap();
    }

    // Keys should always be returned in sorted order
    let keys1 = storage.keys().await.unwrap();
    let keys2 = storage.keys().await.unwrap();

    assert_eq!(keys1, keys2);
    assert_eq!(keys1, vec!["apple", "banana", "cherry", "date", "zebra"]);
}

#[tokio::test]
async fn test_range_pairs_ordering() {
    let storage = BTreeMapStorage::new();

    // Add keys
    for i in 0..100 {
        let key = format!("key_{:03}", i);
        storage
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .unwrap();
    }

    // Get range pairs
    let pairs = storage
        .scan_range_pairs("key_010", "key_050")
        .await
        .unwrap();

    // Verify ordering
    assert_eq!(pairs.len(), 40);
    for i in 1..pairs.len() {
        assert!(pairs[i - 1].0 < pairs[i].0);
    }

    // Verify values match
    for (i, (key, value)) in pairs.iter().enumerate() {
        assert_eq!(key, &format!("key_{:03}", i + 10));
        assert_eq!(value, &Value::String(format!("value_{}", i + 10)));
    }
}
