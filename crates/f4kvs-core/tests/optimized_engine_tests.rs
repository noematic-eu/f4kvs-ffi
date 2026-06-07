//! Unit tests for Optimized F4KVS Core Engine
//!
//! This module provides comprehensive unit tests for the optimized engine implementation.

use f4kvs_core::optimized_engine::OptimizedF4KVSCore;
use f4kvs_core::{Config, StorageMode, Value};
use std::sync::Arc;

#[cfg(test)]
mod optimized_engine_tests {
    use super::*;

    #[tokio::test]
    async fn test_optimized_engine_new() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_engine_with_config() {
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine =
            OptimizedF4KVSCore::with_config(config).expect("Failed to create optimized engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_engine_with_storage() {
        use f4kvs_core::MemoryStorage;
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let engine =
            OptimizedF4KVSCore::with_storage(storage).expect("Failed to create optimized engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_engine_with_config_and_storage() {
        use f4kvs_core::MemoryStorage;
        let config = Config::new();
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let engine = OptimizedF4KVSCore::with_config_and_storage(config, storage)
            .expect("Failed to create optimized engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_engine_get_put_delete() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");

        // Test put
        let value = Value::String("test_value".to_string());
        engine.put("test_key", &value).await.expect("Put failed");

        // Test get
        let retrieved = engine.get("test_key").await.expect("Get failed");
        assert_eq!(retrieved, Some(value.clone()));

        // Test exists
        let exists = engine.exists("test_key").await.expect("Exists failed");
        assert!(exists);

        // Test delete
        engine.delete("test_key").await.expect("Delete failed");
        let retrieved_after_delete = engine
            .get("test_key")
            .await
            .expect("Get after delete failed");
        assert_eq!(retrieved_after_delete, None);
    }

    #[tokio::test]
    async fn test_optimized_engine_batch_operations() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");

        // Test batch put
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::String("value2".to_string())),
        ];
        engine.batch_put(items).await.expect("Batch put failed");

        // Test batch get
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let results = engine.batch_get(keys).await.expect("Batch get failed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(Value::String("value1".to_string())));
        assert_eq!(results[1], Some(Value::String("value2".to_string())));

        // Test batch delete
        let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
        engine
            .batch_delete(keys_to_delete)
            .await
            .expect("Batch delete failed");
    }

    #[tokio::test]
    async fn test_optimized_engine_scan_operations() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");

        // Put some test data
        let items = vec![
            (
                "prefix_key1".to_string(),
                Value::String("value1".to_string()),
            ),
            (
                "prefix_key2".to_string(),
                Value::String("value2".to_string()),
            ),
            ("other_key".to_string(), Value::String("value3".to_string())),
        ];
        engine.batch_put(items).await.expect("Batch put failed");

        // Test scan prefix
        let keys = engine
            .scan_prefix("prefix_")
            .await
            .expect("Scan prefix failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix_key1".to_string()));
        assert!(keys.contains(&"prefix_key2".to_string()));

        // Test scan range. The StorageEngine contract is half-open [start, end), so to
        // include both prefix_key1 and prefix_key2 we need an end that sorts strictly after
        // prefix_key2 (`prefix_key3` works).
        let keys = engine
            .scan_range("prefix_key1", "prefix_key3")
            .await
            .expect("Scan range failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix_key1".to_string()));
        assert!(keys.contains(&"prefix_key2".to_string()));
    }

    #[tokio::test]
    async fn test_optimized_engine_performance_metrics() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");

        // Test initial metrics
        let (ops, hits, misses) = engine.get_performance_metrics();
        assert_eq!(ops, 0);
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);

        // Perform some operations
        let value = Value::String("test_value".to_string());
        engine.put("test_key", &value).await.expect("Put failed");

        // Check that operation count increased
        let (ops_after, _, _) = engine.get_performance_metrics();
        assert_eq!(ops_after, 1);

        // Reset metrics
        engine.reset_performance_metrics();
        let (ops_reset, _, _) = engine.get_performance_metrics();
        assert_eq!(ops_reset, 0);
    }

    #[tokio::test]
    async fn test_optimized_engine_stats() {
        let engine = OptimizedF4KVSCore::new().expect("Failed to create optimized engine");
        let stats = engine.stats().await.expect("Stats failed");
        assert_eq!(stats.total_operations, 0);
    }
}
