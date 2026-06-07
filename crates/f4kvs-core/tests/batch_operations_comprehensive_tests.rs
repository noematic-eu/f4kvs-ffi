//! Comprehensive batch operations tests for F4KVS Core
//!
//! This module provides extensive test coverage for batch operations including
//! edge cases, error handling, and performance scenarios.

use f4kvs_core::{F4KVSCore, StorageMode, Value};

#[tokio::test]
async fn test_batch_put_empty() {
    let engine = F4KVSCore::new().unwrap();

    // Empty batch should succeed
    let result = engine.batch_put(vec![]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_put_large() {
    let engine = F4KVSCore::new().unwrap();

    // Create a large batch
    let mut items = Vec::new();
    for i in 0..1000 {
        items.push((format!("key{}", i), Value::String(format!("value{}", i))));
    }

    engine.batch_put(items).await.unwrap();

    // Verify all items were inserted
    for i in 0..1000 {
        let value = engine.get(&format!("key{}", i)).await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), Value::String(format!("value{}", i)));
    }
}

#[tokio::test]
async fn test_batch_put_mixed_types() {
    let engine = F4KVSCore::new().unwrap();

    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::Int64(42)),
        ("key3".to_string(), Value::UInt64(100)),
        ("key4".to_string(), Value::Float64(3.14)),
        ("key5".to_string(), Value::Bool(true)),
        ("key6".to_string(), Value::Bytes(vec![1, 2, 3])),
        (
            "key7".to_string(),
            Value::Json(serde_json::json!({"key": "value"})),
        ),
    ];

    engine.batch_put(items).await.unwrap();

    // Verify all types were stored correctly
    assert_eq!(
        engine.get("key1").await.unwrap(),
        Some(Value::String("value1".to_string()))
    );
    assert_eq!(engine.get("key2").await.unwrap(), Some(Value::Int64(42)));
    assert_eq!(engine.get("key3").await.unwrap(), Some(Value::UInt64(100)));
    assert_eq!(
        engine.get("key4").await.unwrap(),
        Some(Value::Float64(3.14))
    );
    assert_eq!(engine.get("key5").await.unwrap(), Some(Value::Bool(true)));
    assert_eq!(
        engine.get("key6").await.unwrap(),
        Some(Value::Bytes(vec![1, 2, 3]))
    );
}

#[tokio::test]
async fn test_batch_put_overwrite() {
    let engine = F4KVSCore::new().unwrap();

    // Insert initial values
    engine
        .put("key1", &Value::String("old".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("old".to_string()))
        .await
        .unwrap();

    // Batch put with new values
    let items = vec![
        ("key1".to_string(), Value::String("new1".to_string())),
        ("key2".to_string(), Value::String("new2".to_string())),
    ];

    engine.batch_put(items).await.unwrap();

    // Verify values were overwritten
    assert_eq!(
        engine.get("key1").await.unwrap(),
        Some(Value::String("new1".to_string()))
    );
    assert_eq!(
        engine.get("key2").await.unwrap(),
        Some(Value::String("new2".to_string()))
    );
}

#[tokio::test]
async fn test_batch_get_empty() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.batch_get(vec![]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_batch_get_mixed_existing_missing() {
    let engine = F4KVSCore::new().unwrap();

    // Insert some keys
    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine
        .put("key2", &Value::String("value2".to_string()))
        .await
        .unwrap();
    // key3 is not inserted

    let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
    let results = engine.batch_get(keys).await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(Value::String("value1".to_string())));
    assert_eq!(results[1], Some(Value::String("value2".to_string())));
    assert_eq!(results[2], None);
}

#[tokio::test]
async fn test_batch_get_all_missing() {
    let engine = F4KVSCore::new().unwrap();

    let keys = vec!["missing1".to_string(), "missing2".to_string()];
    let results = engine.batch_get(keys).await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0], None);
    assert_eq!(results[1], None);
}

#[tokio::test]
async fn test_batch_get_large() {
    let engine = F4KVSCore::new().unwrap();

    // Insert many keys
    for i in 0..500 {
        engine
            .put(&format!("key{}", i), &Value::String(format!("value{}", i)))
            .await
            .unwrap();
    }

    // Batch get all keys
    let keys: Vec<String> = (0..500).map(|i| format!("key{}", i)).collect();
    let results = engine.batch_get(keys).await.unwrap();

    assert_eq!(results.len(), 500);
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_some());
        assert_eq!(
            result.as_ref().unwrap(),
            &Value::String(format!("value{}", i))
        );
    }
}

#[tokio::test]
async fn test_batch_delete_empty() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.batch_delete(vec![]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_delete_mixed() {
    let engine = F4KVSCore::new().unwrap();

    // Insert keys
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

    // Delete some keys (including non-existent)
    let keys_to_delete = vec!["key1".to_string(), "key2".to_string(), "key4".to_string()];
    engine.batch_delete(keys_to_delete).await.unwrap();

    // Verify deletions
    assert!(engine.get("key1").await.unwrap().is_none());
    assert!(engine.get("key2").await.unwrap().is_none());
    assert!(engine.get("key3").await.unwrap().is_some()); // Should still exist
}

#[tokio::test]
async fn test_batch_delete_all() {
    let engine = F4KVSCore::new().unwrap();

    // Insert keys
    for i in 0..100 {
        engine
            .put(&format!("key{}", i), &Value::String(format!("value{}", i)))
            .await
            .unwrap();
    }

    // Delete all keys
    let keys: Vec<String> = (0..100).map(|i| format!("key{}", i)).collect();
    engine.batch_delete(keys).await.unwrap();

    // Verify all deleted
    for i in 0..100 {
        assert!(engine.get(&format!("key{}", i)).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn test_batch_operations_sequence() {
    let engine = F4KVSCore::new().unwrap();

    // Batch put
    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::String("value2".to_string())),
        ("key3".to_string(), Value::String("value3".to_string())),
    ];
    engine.batch_put(items).await.unwrap();

    // Batch get
    let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
    let results = engine.batch_get(keys).await.unwrap();
    assert_eq!(results.len(), 3);

    // Batch delete
    let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
    engine.batch_delete(keys_to_delete).await.unwrap();

    // Verify final state
    assert!(engine.get("key1").await.unwrap().is_none());
    assert!(engine.get("key2").await.unwrap().is_none());
    assert!(engine.get("key3").await.unwrap().is_some());
}

#[tokio::test]
async fn test_batch_with_hashmap_mode() {
    let config = f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap);
    let engine = F4KVSCore::with_config(config).unwrap();

    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::String("value2".to_string())),
    ];

    engine.batch_put(items).await.unwrap();
    let keys = vec!["key1".to_string(), "key2".to_string()];
    let results = engine.batch_get(keys).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_batch_with_btreemap_mode() {
    let config = f4kvs_core::Config::new().with_storage_mode(StorageMode::BTreeMap);
    let engine = F4KVSCore::with_config(config).unwrap();

    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::String("value2".to_string())),
    ];

    engine.batch_put(items).await.unwrap();
    let keys = vec!["key1".to_string(), "key2".to_string()];
    let results = engine.batch_get(keys).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_batch_put_duplicate_keys() {
    let engine = F4KVSCore::new().unwrap();

    // Batch put with duplicate keys (last one should win)
    let items = vec![
        ("key1".to_string(), Value::String("first".to_string())),
        ("key1".to_string(), Value::String("second".to_string())),
        ("key1".to_string(), Value::String("third".to_string())),
    ];

    engine.batch_put(items).await.unwrap();

    // Should have the last value
    assert_eq!(
        engine.get("key1").await.unwrap(),
        Some(Value::String("third".to_string()))
    );
}

#[tokio::test]
async fn test_batch_get_duplicate_keys() {
    let engine = F4KVSCore::new().unwrap();

    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();

    // Request same key multiple times
    let keys = vec!["key1".to_string(), "key1".to_string(), "key1".to_string()];
    let results = engine.batch_get(keys).await.unwrap();

    assert_eq!(results.len(), 3);
    for result in results {
        assert_eq!(result, Some(Value::String("value1".to_string())));
    }
}

#[tokio::test]
async fn test_batch_operations_count() {
    let engine = F4KVSCore::new().unwrap();

    // Initial count should be 0
    assert_eq!(engine.count().await.unwrap(), 0);

    // Batch put
    let items: Vec<(String, Value)> = (0..50)
        .map(|i| (format!("key{}", i), Value::String(format!("value{}", i))))
        .collect();
    engine.batch_put(items).await.unwrap();

    // Count should be 50
    assert_eq!(engine.count().await.unwrap(), 50);

    // Batch delete some
    let keys: Vec<String> = (0..25).map(|i| format!("key{}", i)).collect();
    engine.batch_delete(keys).await.unwrap();

    // Count should be 25
    assert_eq!(engine.count().await.unwrap(), 25);
}
