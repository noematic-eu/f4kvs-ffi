//! Unit tests for Optimized Memory Storage
//!
//! This module provides comprehensive unit tests for the optimized memory storage implementation.

use f4kvs_core::optimized_memory_storage::OptimizedMemoryStorage;
use f4kvs_core::{StorageEngine, StorageMode, Value};

#[cfg(test)]
mod optimized_memory_storage_tests {
    use super::*;

    #[tokio::test]
    async fn test_optimized_memory_storage_new() {
        let storage = OptimizedMemoryStorage::new();
        assert!(storage.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_with_mode() {
        let storage = OptimizedMemoryStorage::with_mode(StorageMode::HashMap);
        assert!(storage.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_get_put_delete() {
        let storage = OptimizedMemoryStorage::new();

        // Test put
        let value = Value::String("test_value".to_string());
        storage.put("test_key", &value).await.expect("Put failed");

        // Test get
        let retrieved = storage.get("test_key").await.expect("Get failed");
        assert_eq!(retrieved, Some(value.clone()));

        // Test exists
        let exists = storage.exists("test_key").await.expect("Exists failed");
        assert!(exists);

        // Test delete
        storage.delete("test_key").await.expect("Delete failed");
        let retrieved_after_delete = storage
            .get("test_key")
            .await
            .expect("Get after delete failed");
        assert_eq!(retrieved_after_delete, None);
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_batch_operations() {
        let storage = OptimizedMemoryStorage::new();

        // Test batch put
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::String("value2".to_string())),
        ];
        storage.batch_put(items).await.expect("Batch put failed");

        // Test batch get
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let results = storage.batch_get(keys).await.expect("Batch get failed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(Value::String("value1".to_string())));
        assert_eq!(results[1], Some(Value::String("value2".to_string())));

        // Test batch delete
        let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
        storage
            .batch_delete(keys_to_delete)
            .await
            .expect("Batch delete failed");
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_scan_operations() {
        let storage = OptimizedMemoryStorage::new();

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
        storage.batch_put(items).await.expect("Batch put failed");

        // Test scan prefix
        let keys = storage
            .scan_prefix("prefix_")
            .await
            .expect("Scan prefix failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix_key1".to_string()));
        assert!(keys.contains(&"prefix_key2".to_string()));

        // Test scan range. The StorageEngine contract is half-open [start, end), so to
        // include both prefix_key1 and prefix_key2 we need an end that sorts strictly after
        // prefix_key2 (`prefix_key3` works).
        let keys = storage
            .scan_range("prefix_key1", "prefix_key3")
            .await
            .expect("Scan range failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix_key1".to_string()));
        assert!(keys.contains(&"prefix_key2".to_string()));
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_cache_behavior() {
        let storage = OptimizedMemoryStorage::new();

        // Put a value
        let value = Value::String("cached_value".to_string());
        storage.put("cache_key", &value).await.expect("Put failed");

        // Get it again - should use cache
        let retrieved = storage.get("cache_key").await.expect("Get failed");
        assert_eq!(retrieved, Some(value.clone()));

        // Test that the cache pool has entries
        let (key_pool_size, value_pool_size) = storage.get_pool_stats();
        assert_eq!(key_pool_size, 1);
        assert_eq!(value_pool_size, 1);
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_clear_pools() {
        let storage = OptimizedMemoryStorage::new();

        // Put some data
        let value = Value::String("test_value".to_string());
        storage.put("key1", &value).await.expect("Put failed");

        // Check pools have data
        let (key_pool_size, value_pool_size) = storage.get_pool_stats();
        assert!(key_pool_size > 0);
        assert!(value_pool_size > 0);

        // Clear pools
        storage.clear_pools();

        // Check pools are empty
        let (key_pool_size, value_pool_size) = storage.get_pool_stats();
        assert_eq!(key_pool_size, 0);
        assert_eq!(value_pool_size, 0);
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_stats() {
        let storage = OptimizedMemoryStorage::new();
        let stats = storage.stats().await.expect("Stats failed");
        assert_eq!(stats.total_operations, 0);
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_with_btreemap_mode() {
        let storage = OptimizedMemoryStorage::with_mode(StorageMode::BTreeMap);
        assert!(storage.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_optimized_memory_storage_optimized_batch_operations() {
        let storage = OptimizedMemoryStorage::new();

        // Test optimized batch put
        let items = vec![
            (
                "opt_key1".to_string(),
                Value::String("opt_value1".to_string()),
            ),
            (
                "opt_key2".to_string(),
                Value::String("opt_value2".to_string()),
            ),
        ];
        storage
            .optimized_batch_put(items)
            .await
            .expect("Optimized batch put failed");

        // Test that items were stored
        let retrieved = storage.get("opt_key1").await.expect("Get failed");
        assert_eq!(retrieved, Some(Value::String("opt_value1".to_string())));
    }
}
