//! Comprehensive tests for F4KVS Core Engine
//!
//! This module provides extensive tests for the F4KVS Core Engine to increase
//! code coverage, focusing on all public methods, edge cases, and error handling.

use f4kvs_core::{Config, F4KVSCore, StorageMode, Value};
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
mod engine_constructor_tests {
    use super::*;

    #[tokio::test]
    async fn test_new_engine() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_with_config() {
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine = F4KVSCore::with_config(config).expect("Failed to create engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_with_storage() {
        use f4kvs_core::MemoryStorage;
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let engine = F4KVSCore::with_storage(storage).expect("Failed to create engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_with_config_and_storage() {
        use f4kvs_core::MemoryStorage;
        let config = Config::new();
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let engine =
            F4KVSCore::with_config_and_storage(config, storage).expect("Failed to create engine");
        assert!(engine.exists("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_config_accessor() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let config = engine.config();
        assert_eq!(config.storage_mode, StorageMode::BTreeMap);
    }
}

#[cfg(test)]
mod engine_operation_tests {
    use super::*;

    #[tokio::test]
    async fn test_put_get_delete_sequence() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Put
        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        // Get
        let value = engine.get("key1").await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value1".to_string())));

        // Delete
        engine.delete("key1").await.expect("Delete failed");

        // Verify deleted
        let value = engine.get("key1").await.expect("Get failed");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_exists() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        assert!(!engine.exists("nonexistent").await.expect("Exists failed"));

        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        assert!(engine.exists("key1").await.expect("Exists failed"));
    }

    #[tokio::test]
    async fn test_keys() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Initially empty
        let keys = engine.keys().await.expect("Keys failed");
        assert!(keys.is_empty());

        // Add keys
        for i in 0..10 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let keys = engine.keys().await.expect("Keys failed");
        assert_eq!(keys.len(), 10);
    }

    #[tokio::test]
    async fn test_count() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        assert_eq!(engine.count().await.expect("Count failed"), 0);

        for i in 0..5 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        assert_eq!(engine.count().await.expect("Count failed"), 5);
    }

    #[tokio::test]
    async fn test_clear() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Add keys
        for i in 0..10 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        assert_eq!(engine.count().await.expect("Count failed"), 10);

        // Clear
        engine.clear().await.expect("Clear failed");

        assert_eq!(engine.count().await.expect("Count failed"), 0);
        assert!(engine.keys().await.expect("Keys failed").is_empty());
    }
}

#[cfg(test)]
mod batch_operation_tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_put() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        let items: Vec<(String, Value)> = (0..100)
            .map(|i| (format!("key{}", i), Value::Int64(i as i64)))
            .collect();

        engine.batch_put(items).await.expect("Batch put failed");

        assert_eq!(engine.count().await.expect("Count failed"), 100);
    }

    #[tokio::test]
    async fn test_batch_get() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Insert items
        for i in 0..10 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        // Batch get
        let keys: Vec<String> = (0..10).map(|i| format!("key{}", i)).collect();
        let results = engine.batch_get(keys).await.expect("Batch get failed");

        assert_eq!(results.len(), 10);
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result, &Some(Value::Int64(i as i64)));
        }
    }

    #[tokio::test]
    async fn test_batch_get_nonexistent() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        let keys: Vec<String> = (0..5).map(|i| format!("nonexistent{}", i)).collect();
        let results = engine.batch_get(keys).await.expect("Batch get failed");

        assert_eq!(results.len(), 5);
        for result in results {
            assert_eq!(result, None);
        }
    }

    #[tokio::test]
    async fn test_batch_delete() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Insert items
        for i in 0..10 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        // Batch delete
        let keys: Vec<String> = (0..5).map(|i| format!("key{}", i)).collect();
        engine
            .batch_delete(keys)
            .await
            .expect("Batch delete failed");

        assert_eq!(engine.count().await.expect("Count failed"), 5);
    }
}

#[cfg(test)]
mod scan_operation_tests {
    use super::*;

    #[tokio::test]
    async fn test_scan_prefix() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Insert keys with different prefixes
        engine
            .put("user:1", &Value::String("user1".to_string()))
            .await
            .expect("Put failed");
        engine
            .put("user:2", &Value::String("user2".to_string()))
            .await
            .expect("Put failed");
        engine
            .put("admin:1", &Value::String("admin1".to_string()))
            .await
            .expect("Put failed");

        let keys = engine
            .scan_prefix("user:")
            .await
            .expect("Scan prefix failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user:1".to_string()));
        assert!(keys.contains(&"user:2".to_string()));
    }

    #[tokio::test]
    async fn test_scan_range() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        // Insert keys
        for i in 0..10 {
            engine
                .put(&format!("key{:02}", i), &Value::Int64(i))
                .await
                .expect("Put failed");
        }

        let keys = engine
            .scan_range("key03", "key07")
            .await
            .expect("Scan range failed");
        assert!(keys.len() >= 4);
    }

    #[tokio::test]
    async fn test_scan_prefix_pairs() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("prefix:1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");
        engine
            .put("prefix:2", &Value::String("value2".to_string()))
            .await
            .expect("Put failed");

        let pairs = engine
            .scan_prefix_pairs("prefix:")
            .await
            .expect("Scan prefix pairs failed");
        assert_eq!(pairs.len(), 2);
    }

    #[tokio::test]
    async fn test_scan_range_pairs() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        for i in 0..5 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let pairs = engine
            .scan_range_pairs("key1", "key3")
            .await
            .expect("Scan range pairs failed");
        assert!(pairs.len() >= 2);
    }

    #[tokio::test]
    async fn test_count_prefix() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        for i in 0..5 {
            engine
                .put(&format!("user:{}", i), &Value::Int64(i))
                .await
                .expect("Put failed");
        }

        let count = engine
            .count_prefix("user:")
            .await
            .expect("Count prefix failed");
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_count_range() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        for i in 0..10 {
            engine
                .put(&format!("key{:02}", i), &Value::Int64(i))
                .await
                .expect("Put failed");
        }

        let count = engine
            .count_range("key03", "key07")
            .await
            .expect("Count range failed");
        assert!(count >= 4);
    }
}

#[cfg(test)]
mod production_feature_tests {
    use super::*;

    #[tokio::test]
    async fn test_flush() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        engine.flush().await.expect("Flush failed");
    }

    #[tokio::test]
    async fn test_sync_wal() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        engine.sync_wal().await.expect("Sync WAL failed");
    }

    #[tokio::test]
    async fn test_shutdown() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        engine
            .shutdown(Some(Duration::from_secs(5)))
            .await
            .expect("Shutdown failed");
    }

    #[tokio::test]
    async fn test_shutdown_timeout() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .shutdown(Some(Duration::from_secs(1)))
            .await
            .expect("Shutdown failed");
    }

    #[tokio::test]
    async fn test_health_check() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        let healthy = engine.health_check().await.expect("Health check failed");
        assert!(healthy);
    }

    #[tokio::test]
    async fn test_stats() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        let stats = engine.stats().await.expect("Stats failed");
        assert!(stats.key_count >= 1);
    }

    #[tokio::test]
    async fn test_info() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        let info = engine.info();
        assert!(!info.version.is_empty());
    }

    #[tokio::test]
    async fn test_monitoring_hooks_accessor() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let _hooks = engine.monitoring_hooks();
    }

    #[tokio::test]
    async fn test_memory_tracker_accessor() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let _tracker = engine.memory_tracker();
    }

    #[tokio::test]
    async fn test_leak_detector_accessor() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let _detector = engine.leak_detector();
    }

    #[tokio::test]
    async fn test_is_memory_leak_detected() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        // Just verify it doesn't panic - the result can be true or false
        let _detected = engine.is_memory_leak_detected().await;
    }
}

#[cfg(test)]
mod value_type_tests {
    use super::*;

    #[tokio::test]
    async fn test_string_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        engine
            .put("key", &Value::String("value".to_string()))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value".to_string())));
    }

    #[tokio::test]
    async fn test_integer_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        engine
            .put("key", &Value::Int64(42))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::Int64(42)));
    }

    #[tokio::test]
    async fn test_float_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        #[allow(clippy::approx_constant)]
        let test_value = 3.14;
        engine
            .put("key", &Value::Float64(test_value))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::Float64(test_value)));
    }

    #[tokio::test]
    async fn test_boolean_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        engine
            .put("key", &Value::Bool(true))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::Bool(true)));
    }

    #[tokio::test]
    async fn test_null_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        engine.put("key", &Value::Null).await.expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::Null));
    }

    #[tokio::test]
    async fn test_bytes_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        engine
            .put("key", &Value::Bytes(b"binary data".to_vec()))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::Bytes(b"binary data".to_vec())));
    }
}

#[cfg(test)]
mod concurrent_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;

    #[tokio::test]
    async fn test_concurrent_puts() {
        let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));
        let mut handles = vec![];

        for i in 0..100 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                engine_clone
                    .put(&format!("key{}", i), &Value::Int64(i as i64))
                    .await
                    .expect("Put failed");
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task failed");
        }

        assert_eq!(engine.count().await.expect("Count failed"), 100);
    }

    #[tokio::test]
    async fn test_concurrent_gets() {
        let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));

        // Pre-populate
        for i in 0..50 {
            engine
                .put(&format!("key{}", i), &Value::Int64(i as i64))
                .await
                .expect("Put failed");
        }

        let mut handles = vec![];
        for i in 0..50 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                let value = engine_clone
                    .get(&format!("key{}", i))
                    .await
                    .expect("Get failed");
                assert_eq!(value, Some(Value::Int64(i)));
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task failed");
        }
    }

    #[tokio::test]
    async fn test_concurrent_mixed_operations() {
        let engine = Arc::new(F4KVSCore::new().expect("Failed to create engine"));
        let mut handles = vec![];

        // Mix of put, get, delete
        for i in 0..50 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                engine_clone
                    .put(&format!("key{}", i), &Value::Int64(i as i64))
                    .await
                    .expect("Put failed");
                let _ = engine_clone
                    .get(&format!("key{}", i))
                    .await
                    .expect("Get failed");
                if i % 2 == 0 {
                    engine_clone
                        .delete(&format!("key{}", i))
                        .await
                        .expect("Delete failed");
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Task failed");
        }
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_key() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        // Empty keys are not allowed - should return an error
        let result = engine
            .put("", &Value::String("empty key".to_string()))
            .await;
        assert!(result.is_err());
        // Verify the error is InvalidKey
        if let Err(e) = result {
            assert!(
                format!("{:?}", e).contains("InvalidKey") || format!("{:?}", e).contains("empty")
            );
        }
    }

    #[tokio::test]
    async fn test_very_long_key() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let long_key = "a".repeat(1000);
        engine
            .put(&long_key, &Value::String("value".to_string()))
            .await
            .expect("Put failed");
        let value = engine.get(&long_key).await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value".to_string())));
    }

    #[tokio::test]
    async fn test_very_long_value() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let long_value = "a".repeat(10000);
        engine
            .put("key", &Value::String(long_value.clone()))
            .await
            .expect("Put failed");
        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::String(long_value)));
    }

    #[tokio::test]
    async fn test_special_characters_in_key() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let special_key = "key:with:colons:and/slashes";
        engine
            .put(special_key, &Value::String("value".to_string()))
            .await
            .expect("Put failed");
        let value = engine.get(special_key).await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value".to_string())));
    }

    #[tokio::test]
    async fn test_unicode_keys() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let unicode_key = "key_测试_キー_тест";
        engine
            .put(unicode_key, &Value::String("value".to_string()))
            .await
            .expect("Put failed");
        let value = engine.get(unicode_key).await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value".to_string())));
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let engine = F4KVSCore::new().expect("Failed to create engine");

        engine
            .put("key", &Value::String("value1".to_string()))
            .await
            .expect("Put failed");

        engine
            .put("key", &Value::String("value2".to_string()))
            .await
            .expect("Put failed");

        let value = engine.get("key").await.expect("Get failed");
        assert_eq!(value, Some(Value::String("value2".to_string())));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_key() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        // Should not error
        engine.delete("nonexistent").await.expect("Delete failed");
    }

    #[tokio::test]
    async fn test_scan_empty_prefix() {
        let engine = F4KVSCore::new().expect("Failed to create engine");
        let keys = engine.scan_prefix("").await.expect("Scan prefix failed");
        // Should return all keys (empty in this case)
        assert!(keys.is_empty());
    }
}
