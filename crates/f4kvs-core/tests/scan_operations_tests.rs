//! Comprehensive scan operations tests for F4KVS Core
//!
//! This module provides extensive test coverage for scan operations including
//! prefix scanning, range scanning, counting, and edge cases.

use f4kvs_core::{F4KVSCore, StorageMode, Value};

#[tokio::test]
async fn test_scan_prefix_basic() {
    let engine = F4KVSCore::new().unwrap();

    // Insert keys with different prefixes
    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine
        .put("user:3", &Value::String("Charlie".to_string()))
        .await
        .unwrap();
    engine
        .put("order:1", &Value::String("Order 1".to_string()))
        .await
        .unwrap();
    engine
        .put("order:2", &Value::String("Order 2".to_string()))
        .await
        .unwrap();

    // Scan user keys
    let user_keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(user_keys.len(), 3);
    assert!(user_keys.contains(&"user:1".to_string()));
    assert!(user_keys.contains(&"user:2".to_string()));
    assert!(user_keys.contains(&"user:3".to_string()));

    // Scan order keys
    let order_keys = engine.scan_prefix("order:").await.unwrap();
    assert_eq!(order_keys.len(), 2);
}

#[tokio::test]
async fn test_scan_prefix_pairs() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine.put("user:3", &Value::Int64(42)).await.unwrap();

    let pairs = engine.scan_prefix_pairs("user:").await.unwrap();
    assert_eq!(pairs.len(), 3);

    // Verify all pairs have correct keys
    let keys: Vec<String> = pairs.iter().map(|(k, _)| k.clone()).collect();
    assert!(keys.contains(&"user:1".to_string()));
    assert!(keys.contains(&"user:2".to_string()));
    assert!(keys.contains(&"user:3".to_string()));

    // Verify values
    for (key, value) in pairs {
        if key == "user:1" {
            assert_eq!(value, Value::String("Alice".to_string()));
        } else if key == "user:2" {
            assert_eq!(value, Value::String("Bob".to_string()));
        } else if key == "user:3" {
            assert_eq!(value, Value::Int64(42));
        }
    }
}

#[tokio::test]
async fn test_scan_prefix_empty() {
    let engine = F4KVSCore::new().unwrap();

    // Scan with prefix that doesn't exist
    let keys = engine.scan_prefix("nonexistent:").await.unwrap();
    assert!(keys.is_empty());

    let pairs = engine.scan_prefix_pairs("nonexistent:").await.unwrap();
    assert!(pairs.is_empty());
}

#[tokio::test]
async fn test_scan_range_basic() {
    let engine = F4KVSCore::new().unwrap();

    // Insert keys in order
    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();
    engine
        .put("key3", &Value::String("value3".to_string()))
        .await
        .unwrap();
    engine
        .put("key4", &Value::String("value4".to_string()))
        .await
        .unwrap();
    engine
        .put("key5", &Value::String("value5".to_string()))
        .await
        .unwrap();

    // Scan range key2 to key4 (inclusive start, exclusive end)
    let keys = engine.scan_range("key2", "key4").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"key2".to_string()));
    assert!(keys.contains(&"key3".to_string()));
    assert!(!keys.contains(&"key4".to_string()));
}

#[tokio::test]
async fn test_scan_range_pairs() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("a", &Value::String("A".to_string()))
        .await
        .unwrap();
    engine
        .put("b", &Value::String("B".to_string()))
        .await
        .unwrap();
    engine
        .put("c", &Value::String("C".to_string()))
        .await
        .unwrap();
    engine
        .put("d", &Value::String("D".to_string()))
        .await
        .unwrap();

    let pairs = engine.scan_range_pairs("b", "d").await.unwrap();
    assert_eq!(pairs.len(), 2);

    // Verify keys
    let keys: Vec<String> = pairs.iter().map(|(k, _)| k.clone()).collect();
    assert!(keys.contains(&"b".to_string()));
    assert!(keys.contains(&"c".to_string()));
    assert!(!keys.contains(&"d".to_string()));
}

#[tokio::test]
async fn test_scan_range_empty() {
    let engine = F4KVSCore::new().unwrap();

    // Scan range with no matching keys
    let keys = engine.scan_range("x", "z").await.unwrap();
    assert!(keys.is_empty());

    let pairs = engine.scan_range_pairs("x", "z").await.unwrap();
    assert!(pairs.is_empty());
}

#[tokio::test]
async fn test_count_prefix() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine
        .put("user:3", &Value::String("Charlie".to_string()))
        .await
        .unwrap();
    engine
        .put("order:1", &Value::String("Order 1".to_string()))
        .await
        .unwrap();

    let user_count = engine.count_prefix("user:").await.unwrap();
    assert_eq!(user_count, 3);

    let order_count = engine.count_prefix("order:").await.unwrap();
    assert_eq!(order_count, 1);

    let nonexistent_count = engine.count_prefix("nonexistent:").await.unwrap();
    assert_eq!(nonexistent_count, 0);
}

#[tokio::test]
async fn test_count_range() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();
    engine
        .put("key3", &Value::String("value3".to_string()))
        .await
        .unwrap();
    engine
        .put("key4", &Value::String("value4".to_string()))
        .await
        .unwrap();

    let count = engine.count_range("key2", "key4").await.unwrap();
    assert_eq!(count, 2);

    let count_all = engine.count_range("key1", "key5").await.unwrap();
    assert_eq!(count_all, 4);

    let count_empty = engine.count_range("x", "z").await.unwrap();
    assert_eq!(count_empty, 0);
}

#[tokio::test]
async fn test_scan_with_hashmap_mode() {
    let config = f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap);
    let engine = F4KVSCore::with_config(config).unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();

    let keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn test_scan_with_btreemap_mode() {
    let config = f4kvs_core::Config::new().with_storage_mode(StorageMode::BTreeMap);
    let engine = F4KVSCore::with_config(config).unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();

    let keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn test_scan_prefix_partial_match() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:10", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine
        .put("user:100", &Value::String("Charlie".to_string()))
        .await
        .unwrap();
    engine
        .put("user_profile:1", &Value::String("Profile".to_string()))
        .await
        .unwrap();

    // Should only match user: prefix, not user_profile:
    let keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(keys.len(), 3);
    assert!(!keys.contains(&"user_profile:1".to_string()));
}

#[tokio::test]
async fn test_scan_range_inclusive_exclusive() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();
    engine
        .put("key3", &Value::String("value3".to_string()))
        .await
        .unwrap();

    // Range from key1 to key3 should include key1 and key2, but not key3
    let keys = engine.scan_range("key1", "key3").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"key1".to_string()));
    assert!(keys.contains(&"key2".to_string()));
    assert!(!keys.contains(&"key3".to_string()));
}

#[tokio::test]
async fn test_scan_after_delete() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine
        .put("user:3", &Value::String("Charlie".to_string()))
        .await
        .unwrap();

    let count_before = engine.count_prefix("user:").await.unwrap();
    assert_eq!(count_before, 3);

    engine.delete("user:2").await.unwrap();

    let count_after = engine.count_prefix("user:").await.unwrap();
    assert_eq!(count_after, 2);

    let keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(!keys.contains(&"user:2".to_string()));
}

#[tokio::test]
async fn test_scan_with_different_value_types() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key:1", &Value::String("string".to_string()))
        .await
        .unwrap();
    engine.put("key:2", &Value::Int64(42)).await.unwrap();
    engine.put("key:3", &Value::Bool(true)).await.unwrap();
    engine
        .put("key:4", &Value::Bytes(vec![1, 2, 3]))
        .await
        .unwrap();

    let pairs = engine.scan_prefix_pairs("key:").await.unwrap();
    assert_eq!(pairs.len(), 4);

    // Verify all value types are present
    let values: Vec<&Value> = pairs.iter().map(|(_, v)| v).collect();
    assert!(values.iter().any(|v| matches!(v, Value::String(_))));
    assert!(values.iter().any(|v| matches!(v, Value::Int64(_))));
    assert!(values.iter().any(|v| matches!(v, Value::Bool(_))));
    assert!(values.iter().any(|v| matches!(v, Value::Bytes(_))));
}

#[tokio::test]
async fn test_scan_empty_prefix() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();

    // Empty prefix should match all keys
    let keys = engine.scan_prefix("").await.unwrap();
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn test_count_all_keys() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();
    engine
        .put("key3", &Value::String("value3".to_string()))
        .await
        .unwrap();

    let total_count = engine.count().await.unwrap();
    assert_eq!(total_count, 3);

    // Count with empty prefix should match all
    let prefix_count = engine.count_prefix("").await.unwrap();
    assert_eq!(prefix_count, 3);
}
