//! Comprehensive tests for Memory Storage
//!
//! This module provides extensive tests for Memory Storage implementations
//! to increase code coverage.

use f4kvs_core::{MemoryStorage, StorageEngine, StorageMode, Value};
use std::sync::Arc;

#[cfg(test)]
mod memory_storage_tests {
    use super::*;

    #[tokio::test]
    async fn test_hashmap_storage() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        let value = storage.get("key1").await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value1".to_string())));
    }

    #[tokio::test]
    async fn test_btreemap_storage() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::BTreeMap));

        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        let value = storage.get("key1").await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value1".to_string())));
    }

    #[tokio::test]
    async fn test_storage_delete() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        storage.delete("key1").await.expect("Delete failed");

        let value = storage.get("key1").await.expect("Get failed");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_storage_exists() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        assert!(!storage.exists("key1").await.expect("Exists failed"));

        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        assert!(storage.exists("key1").await.expect("Exists failed"));
    }

    #[tokio::test]
    async fn test_storage_keys() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        for i in 0..10 {
            storage
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let keys = storage.keys().await.expect("Keys failed");
        assert_eq!(keys.len(), 10);
    }

    #[tokio::test]
    async fn test_storage_count() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        assert_eq!(storage.count().await.expect("Count failed"), 0);

        for i in 0..5 {
            storage
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        assert_eq!(storage.count().await.expect("Count failed"), 5);
    }

    #[tokio::test]
    async fn test_storage_clear() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        for i in 0..10 {
            storage
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        storage.clear().await.expect("Clear failed");

        assert_eq!(storage.count().await.expect("Count failed"), 0);
    }

    #[tokio::test]
    async fn test_storage_flush() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        storage.flush().await.expect("Flush failed");
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        for i in 0..10 {
            storage
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let stats = storage.stats().await.expect("Stats failed");
        assert!(stats.key_count >= 10);
    }

    #[tokio::test]
    async fn test_storage_scan_prefix() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));

        storage
            .put("user:1", &Value::String("user1".to_string()))
            .await
            .expect("Put failed");
        storage
            .put("user:2", &Value::String("user2".to_string()))
            .await
            .expect("Put failed");

        let keys = storage
            .scan_prefix("user:")
            .await
            .expect("Scan prefix failed");
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_storage_scan_range() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::BTreeMap));

        for i in 0..10 {
            storage
                .put(&format!("key{:02}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let keys = storage
            .scan_range("key03", "key07")
            .await
            .expect("Scan range failed");
        assert!(keys.len() >= 4);
    }
}
