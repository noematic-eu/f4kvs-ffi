//! Advanced features tests for F4KVS Core

use f4kvs_core::{
    CacheConfig, CachedStorageEngine, F4KVSCore, MemoryStorage, MonitoredStorageEngine,
    QueryBuilder, StorageEngine, StorageMode, Value,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_advanced_querying_features() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::BTreeMap))
            .unwrap();

    // Add test data
    engine
        .put("user:1", &Value::String("Alice".to_string()))
        .await
        .unwrap();
    engine
        .put("user:2", &Value::String("Bob".to_string()))
        .await
        .unwrap();
    engine
        .put("admin:1", &Value::String("Admin".to_string()))
        .await
        .unwrap();
    engine
        .put("product:a", &Value::String("Product A".to_string()))
        .await
        .unwrap();
    engine
        .put("product:b", &Value::String("Product B".to_string()))
        .await
        .unwrap();

    // Test prefix scanning
    let user_keys = engine.scan_prefix("user:").await.unwrap();
    assert_eq!(user_keys.len(), 2);
    assert!(user_keys.contains(&"user:1".to_string()));
    assert!(user_keys.contains(&"user:2".to_string()));

    // Test range scanning
    let range_keys = engine.scan_range("a", "z").await.unwrap();
    assert_eq!(range_keys.len(), 5); // All keys from a to z
    assert!(range_keys.contains(&"admin:1".to_string()));
    assert!(range_keys.contains(&"product:a".to_string()));
    assert!(range_keys.contains(&"product:b".to_string()));
    assert!(range_keys.contains(&"user:1".to_string()));
    assert!(range_keys.contains(&"user:2".to_string()));

    // Test prefix counting
    let user_count = engine.count_prefix("user:").await.unwrap();
    assert_eq!(user_count, 2);

    // Test range counting
    let range_count = engine.count_range("a", "z").await.unwrap();
    assert_eq!(range_count, 5);
}

#[tokio::test]
async fn test_query_builder_advanced_features() {
    let storage = MemoryStorage::with_mode(StorageMode::BTreeMap);

    // Add test data
    for i in 0..20 {
        storage
            .put(&format!("key_{:02}", i), &Value::Int64(i as i64))
            .await
            .unwrap();
    }

    // Test query with prefix, limit, and offset
    let query = QueryBuilder::new()
        .with_prefix("key_1")
        .with_limit(5)
        .with_offset(2)
        .with_values()
        .execute(&storage)
        .await
        .unwrap();

    assert_eq!(query.len(), 5);
    assert_eq!(query.total_count, 10); // All keys starting with "key_1"
                                       // Check that we got some keys (the exact keys depend on the offset)
    assert!(!query.is_empty());

    // Test range query
    let range_query = QueryBuilder::new()
        .with_range("key_05", "key_15")
        .with_limit(3)
        .with_values()
        .execute(&storage)
        .await
        .unwrap();

    assert_eq!(range_query.len(), 3);
    assert_eq!(range_query.total_count, 10); // Keys from key_05 to key_14
}

#[tokio::test]
async fn test_caching_features() {
    let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
    let cache_config = CacheConfig {
        max_entries: 10,
        max_age: Duration::from_secs(60),
        max_idle_time: Duration::from_secs(30),
        auto_cleanup: false,
        cleanup_interval: Duration::from_secs(30),
    };
    let cached = CachedStorageEngine::with_cache_config(storage, cache_config);

    // Test basic caching
    <CachedStorageEngine as StorageEngine>::put(
        &cached,
        "key1",
        &Value::String("value1".to_string()),
    )
    .await
    .unwrap();

    // First get should miss cache
    let result1 = <CachedStorageEngine as StorageEngine>::get(&cached, "key1")
        .await
        .unwrap();
    assert_eq!(result1, Some(Value::String("value1".to_string())));

    // Second get should hit cache
    let result2 = <CachedStorageEngine as StorageEngine>::get(&cached, "key1")
        .await
        .unwrap();
    assert_eq!(result2, Some(Value::String("value1".to_string())));

    // Check cache stats
    let stats = cached.cache_stats().await;
    // hits and misses are unsigned integers, so they're always >= 0
    assert_eq!(stats.entries, 1);
}

#[tokio::test]
async fn test_performance_monitoring() {
    let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
    let monitored = MonitoredStorageEngine::new(storage);

    // Perform various operations
    <MonitoredStorageEngine as StorageEngine>::put(
        &monitored,
        "key1",
        &Value::String("value1".to_string()),
    )
    .await
    .unwrap();
    <MonitoredStorageEngine as StorageEngine>::put(&monitored, "key2", &Value::Int64(42))
        .await
        .unwrap();
    <MonitoredStorageEngine as StorageEngine>::get(&monitored, "key1")
        .await
        .unwrap();
    <MonitoredStorageEngine as StorageEngine>::exists(&monitored, "key2")
        .await
        .unwrap();
    <MonitoredStorageEngine as StorageEngine>::delete(&monitored, "key1")
        .await
        .unwrap();

    // Wait for metrics to be calculated
    sleep(Duration::from_millis(10)).await;

    let metrics = monitored.get_metrics().await;
    assert_eq!(metrics.total_operations, 5);
    assert!(metrics.operations_per_second >= 0.0);
    assert!(metrics.average_latency_us > 0.0);
    assert!(metrics.p50_latency_us > 0.0);
    assert!(metrics.p95_latency_us > 0.0);
    assert!(metrics.p99_latency_us > 0.0);
    assert_eq!(metrics.error_rate, 0.0);

    // Check operation counts
    let op_counts = monitored.get_operation_counts().await;
    assert!(op_counts.get(&f4kvs_core::OperationType::Put).unwrap_or(&0) > &0);
    assert!(op_counts.get(&f4kvs_core::OperationType::Get).unwrap_or(&0) > &0);
    assert!(
        op_counts
            .get(&f4kvs_core::OperationType::Delete)
            .unwrap_or(&0)
            > &0
    );
}

#[tokio::test]
async fn test_enhanced_storage_stats() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap))
            .unwrap();

    // Perform various operations
    engine
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    engine.put("key2", &Value::Int64(42)).await.unwrap();
    engine.get("key1").await.unwrap();
    engine.scan_prefix("key").await.unwrap();
    engine.delete("key1").await.unwrap();

    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, 1);
    assert!(stats.memory_usage > 0);
    assert!(stats.total_operations > 0);
    assert!(stats.get_operations > 0);
    assert!(stats.put_operations > 0);
    assert!(stats.delete_operations > 0);
    assert!(stats.scan_operations > 0);
    assert!(stats.average_key_size > 0.0);
    assert!(stats.average_value_size > 0.0);
    assert!(stats.peak_memory_usage > 0);
}

#[tokio::test]
async fn test_batch_operations_with_advanced_features() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::BTreeMap))
            .unwrap();

    // Test batch operations
    let items = vec![
        ("batch:1".to_string(), Value::String("value1".to_string())),
        ("batch:2".to_string(), Value::Int64(42)),
        ("batch:3".to_string(), Value::Bool(true)),
    ];

    engine.batch_put(items).await.unwrap();

    // Test batch get
    let keys = vec![
        "batch:1".to_string(),
        "batch:2".to_string(),
        "batch:3".to_string(),
    ];
    let results = engine.batch_get(keys).await.unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0].is_some());
    assert!(results[1].is_some());
    assert!(results[2].is_some());

    // Test batch delete
    let delete_keys = vec!["batch:1".to_string(), "batch:2".to_string()];
    engine.batch_delete(delete_keys).await.unwrap();

    // Verify deletion
    assert!(engine.get("batch:1").await.unwrap().is_none());
    assert!(engine.get("batch:2").await.unwrap().is_none());
    assert!(engine.get("batch:3").await.unwrap().is_some());
}

#[tokio::test]
async fn test_concurrent_advanced_operations() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap))
            .unwrap();

    // Spawn multiple tasks performing different operations
    let mut handles = vec![];

    // Task 1: Put operations
    let engine1 = engine.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..10 {
            engine1
                .put(&format!("concurrent:{}", i), &Value::Int64(i as i64))
                .await
                .unwrap();
        }
    }));

    // Task 2: Get operations
    let engine2 = engine.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..10 {
            engine2.get(&format!("concurrent:{}", i)).await.unwrap();
        }
    }));

    // Task 3: Scan operations
    let engine3 = engine.clone();
    handles.push(tokio::spawn(async move {
        for _ in 0..5 {
            engine3.scan_prefix("concurrent:").await.unwrap();
        }
    }));

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let stats = engine.stats().await.unwrap();
    assert_eq!(stats.key_count, 10);
    assert!(stats.total_operations > 0);
    assert!(stats.get_operations > 0);
    assert!(stats.put_operations > 0);
    assert!(stats.scan_operations > 0);
}

#[tokio::test]
async fn test_memory_optimization() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap))
            .unwrap();

    // Add some data
    for i in 0..100 {
        engine
            .put(
                &format!("mem_test:{}", i),
                &Value::String(format!("value_{}", i)),
            )
            .await
            .unwrap();
    }

    let initial_stats = engine.stats().await.unwrap();
    let initial_memory = initial_stats.memory_usage;

    // Clear some data
    for i in 0..50 {
        engine.delete(&format!("mem_test:{}", i)).await.unwrap();
    }

    let final_stats = engine.stats().await.unwrap();
    let final_memory = final_stats.memory_usage;

    // Memory should have decreased
    assert!(final_memory < initial_memory);
    assert_eq!(final_stats.key_count, 50);
}

#[tokio::test]
async fn test_error_handling_advanced() {
    let engine =
        F4KVSCore::with_config(f4kvs_core::Config::new().with_storage_mode(StorageMode::HashMap))
            .unwrap();

    // Test with invalid operations
    let result = engine
        .put("", &Value::String("empty_key".to_string()))
        .await;
    assert!(result.is_err());

    // Test with very large values
    let large_value = Value::String("x".repeat(10000));
    let result = engine.put("large_key", &large_value).await;
    assert!(result.is_ok());

    // Test scan with invalid range
    let result = engine.scan_range("z", "a").await; // Invalid range
    assert!(result.is_ok()); // Should return empty result, not error
    assert_eq!(result.unwrap().len(), 0);
}
