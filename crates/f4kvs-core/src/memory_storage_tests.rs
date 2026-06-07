//! Tests for memory storage implementation
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;
use crate::Value;
use std::sync::Arc;
use tokio::task;

#[tokio::test]
async fn test_memory_storage_basic_operations() {
    let storage = MemoryStorage::new();

    // Test put and get
    storage
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    let value = storage.get("key1").await.unwrap();
    assert_eq!(value, Some(Value::String("value1".to_string())));

    // Test exists
    assert!(storage.exists("key1").await.unwrap());
    assert!(!storage.exists("nonexistent").await.unwrap());

    // Test delete
    storage.delete("key1").await.unwrap();
    assert!(!storage.exists("key1").await.unwrap());
    assert!(storage.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_memory_storage_stats() {
    let storage = MemoryStorage::new();

    assert_eq!(storage.count().await.unwrap(), 0);

    storage
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    storage.put("key2", &Value::Int64(42)).await.unwrap();

    assert_eq!(storage.count().await.unwrap(), 2);

    let stats = storage.stats().await.unwrap();
    assert_eq!(stats.key_count, 2);
    assert!(stats.memory_usage > 0);
    assert!(stats.total_operations > 0);
}

#[tokio::test]
async fn test_memory_storage_keys() {
    let storage = MemoryStorage::new();

    storage
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    storage.put("key2", &Value::Int64(42)).await.unwrap();
    storage.put("key3", &Value::Bool(true)).await.unwrap();

    let keys = storage.keys().await.unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"key1".to_string()));
    assert!(keys.contains(&"key2".to_string()));
    assert!(keys.contains(&"key3".to_string()));
}

#[tokio::test]
async fn test_memory_storage_clear() {
    let storage = MemoryStorage::new();

    storage
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();
    storage.put("key2", &Value::Int64(42)).await.unwrap();

    assert_eq!(storage.count().await.unwrap(), 2);

    storage.clear().await.unwrap();
    assert_eq!(storage.count().await.unwrap(), 0);
    let keys = storage.keys().await.unwrap();
    assert!(keys.is_empty());
}

#[tokio::test]
async fn test_memory_storage_incremental_memory_tracking() {
    let storage = MemoryStorage::new();

    // Test that memory usage is tracked incrementally
    let initial_memory = storage.get_current_memory_usage().await;
    assert_eq!(initial_memory, 0);

    // Add a key-value pair
    storage
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();

    let memory_after_put = storage.get_current_memory_usage().await;
    assert!(memory_after_put > 0);

    // Verify the memory calculation is correct
    let expected_memory = "key1".len() + Value::String("value1".to_string()).memory_size();
    assert_eq!(memory_after_put as usize, expected_memory);

    // Update the same key with a larger value
    storage
        .put(
            "key1",
            &Value::String("much_longer_value_for_testing".to_string()),
        )
        .await
        .unwrap();

    let memory_after_update = storage.get_current_memory_usage().await;
    assert!(memory_after_update > memory_after_put);

    // Verify the memory calculation is still correct
    let expected_memory_after_update =
        "key1".len() + Value::String("much_longer_value_for_testing".to_string()).memory_size();
    assert_eq!(memory_after_update as usize, expected_memory_after_update);

    // Delete the key
    storage.delete("key1").await.unwrap();

    let memory_after_delete = storage.get_current_memory_usage().await;
    assert_eq!(memory_after_delete, 0);
}

#[tokio::test]
async fn test_memory_storage_batch_operations_memory_tracking() {
    let storage = MemoryStorage::new();

    // Test batch put with memory tracking
    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("key2".to_string(), Value::Int64(42)),
        ("key3".to_string(), Value::Bool(true)),
    ];

    storage.batch_put(items).await.unwrap();

    let memory_after_batch_put = storage.get_current_memory_usage().await;
    assert!(memory_after_batch_put > 0);

    // Test batch delete with memory tracking
    let keys_to_delete = vec!["key1".to_string(), "key3".to_string()];
    storage.batch_delete(keys_to_delete).await.unwrap();

    let memory_after_batch_delete = storage.get_current_memory_usage().await;
    assert!(memory_after_batch_delete < memory_after_batch_put);

    // Verify only key2 remains
    assert_eq!(storage.count().await.unwrap(), 1);
    assert!(storage.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_concurrent_storage_operations() {
    let storage = Arc::new(MemoryStorage::new());

    // Spawn concurrent operations
    let mut handles = vec![];
    for task_id in 0..20 {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            for i in 0..50 {
                let key = format!("concurrent_key{}_task{}", i, task_id);
                let value = Value::String(format!("concurrent_value{}_task{}", i, task_id));

                // Put operation
                storage_clone.put(&key, &value).await.unwrap();

                // Get operation
                let retrieved = storage_clone.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));

                // Exists operation
                assert!(storage_clone.exists(&key).await.unwrap());
            }
            format!("Storage task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify final state
    assert_eq!(storage.count().await.unwrap(), 1000); // 20 tasks * 50 keys each
}

#[tokio::test]
async fn test_concurrent_storage_stats() {
    let storage = Arc::new(MemoryStorage::new());

    // Pre-populate with data
    for i in 0..100 {
        storage
            .put(
                &format!("stats_key{}", i),
                &Value::String(format!("stats_value{}", i)),
            )
            .await
            .unwrap();
    }

    // Spawn concurrent stats access
    let mut handles = vec![];
    for task_id in 0..30 {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            for _ in 0..10 {
                let stats = storage_clone.stats().await.unwrap();
                assert_eq!(stats.key_count, 100);
                assert!(stats.memory_usage > 0);
                assert!(stats.total_operations > 0);

                let count = storage_clone.count().await.unwrap();
                assert_eq!(count, 100);

                let keys = storage_clone.keys().await.unwrap();
                assert_eq!(keys.len(), 100);
            }
            format!("Stats task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_storage_deletes() {
    let storage = Arc::new(MemoryStorage::new());

    // Pre-populate with data
    for i in 0..200 {
        storage
            .put(
                &format!("delete_key{}", i),
                &Value::String(format!("delete_value{}", i)),
            )
            .await
            .unwrap();
    }

    assert_eq!(storage.count().await.unwrap(), 200);

    // Spawn concurrent delete operations
    let mut handles = vec![];
    for task_id in 0..4 {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            for i in 0..50 {
                let key = format!("delete_key{}", task_id * 50 + i);
                storage_clone.delete(&key).await.unwrap();
            }
            format!("Delete task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify all keys were deleted
    assert_eq!(storage.count().await.unwrap(), 0);
    let keys = storage.keys().await.unwrap();
    assert!(keys.is_empty());
}

#[tokio::test]
async fn test_concurrent_storage_clear() {
    let storage = Arc::new(MemoryStorage::new());

    // Pre-populate with data
    for i in 0..100 {
        storage
            .put(
                &format!("clear_key{}", i),
                &Value::String(format!("clear_value{}", i)),
            )
            .await
            .unwrap();
    }

    assert_eq!(storage.count().await.unwrap(), 100);

    // Spawn concurrent clear operations
    let mut handles = vec![];
    for task_id in 0..5 {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            storage_clone.clear().await.unwrap();
            format!("Clear task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify the storage is empty
    assert_eq!(storage.count().await.unwrap(), 0);
    let keys = storage.keys().await.unwrap();
    assert!(keys.is_empty());
}

#[tokio::test]
async fn test_concurrent_storage_mixed_operations() {
    let storage = Arc::new(MemoryStorage::new());

    // Spawn mixed operation tasks
    let mut handles = vec![];
    let num_tasks = 25;
    let operations_per_task = 80;

    for task_id in 0..num_tasks {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            for i in 0..operations_per_task {
                let operation = i % 5; // 0: put, 1: get, 2: delete, 3: exists, 4: count
                let key = format!("mixed_key{}_task{}", i, task_id);

                match operation {
                    0 => {
                        let value = Value::String(format!("mixed_value{}_task{}", i, task_id));
                        storage_clone.put(&key, &value).await.unwrap();
                    }
                    1 => {
                        let _ = storage_clone.get(&key).await;
                    }
                    2 => {
                        let _ = storage_clone.delete(&key).await;
                    }
                    3 => {
                        let _ = storage_clone.exists(&key).await;
                    }
                    4 => {
                        let _ = storage_clone.count().await;
                    }
                    _ => unreachable!(),
                }
            }
            format!("Mixed operation task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify the storage is still functional
    let stats = storage.stats().await.unwrap();
    assert!(stats.total_operations > 0);
}

#[tokio::test]
async fn test_concurrent_storage_large_values() {
    let storage = Arc::new(MemoryStorage::new());

    // Create large values
    let large_string = "x".repeat(10000); // Large but reasonable size
    let large_value = Value::String(large_string);

    // Spawn concurrent operations with large values
    let mut handles = vec![];
    for task_id in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let large_value_clone = large_value.clone();
        let handle = task::spawn(async move {
            for i in 0..20 {
                let key = format!("large_key{}_task{}", i, task_id);

                // Put large value
                storage_clone.put(&key, &large_value_clone).await.unwrap();

                // Get large value
                let retrieved = storage_clone.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(large_value_clone.clone()));

                // Delete large value
                storage_clone.delete(&key).await.unwrap();
            }
            format!("Large value task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify final state
    assert_eq!(storage.count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_concurrent_storage_memory_pressure() {
    let storage = Arc::new(MemoryStorage::new());

    // Spawn many tasks that create and destroy data rapidly
    let mut handles = vec![];
    for task_id in 0..50 {
        let storage_clone = Arc::clone(&storage);
        let handle = task::spawn(async move {
            for i in 0..100 {
                let key = format!("pressure_key{}_task{}", i, task_id);
                let value = Value::String(format!("pressure_value{}_task{}", i, task_id));

                // Create data
                storage_clone.put(&key, &value).await.unwrap();

                // Verify it exists
                assert!(storage_clone.exists(&key).await.unwrap());

                // Delete it immediately
                storage_clone.delete(&key).await.unwrap();

                // Verify it's gone
                assert!(!storage_clone.exists(&key).await.unwrap());
            }
            format!("Memory pressure task {} completed", task_id)
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
    }

    // Verify final state
    assert_eq!(storage.count().await.unwrap(), 0);

    // Check memory usage is reasonable
    let stats = storage.stats().await.unwrap();
    assert_eq!(stats.memory_usage, 0);
}
