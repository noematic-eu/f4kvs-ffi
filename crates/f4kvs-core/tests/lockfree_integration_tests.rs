//! Lock-free data structures integration tests
//!
//! This module provides comprehensive integration tests for lock-free data structures
//! including SafeLockFreeHashMap under heavy concurrent load.

use f4kvs_core::lockfree::{LockFreeHashMap, LockFreeHashMapConfig, LockFreeQueue, LockFreeStack};
use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::time::timeout;

/// Lock-free integration test suite
pub struct LockFreeIntegrationTestSuite;

impl LockFreeIntegrationTestSuite {
    /// Run all lock-free integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running Lock-Free Integration Tests");
        println!("=====================================");
        println!();

        // Test basic lock-free operations
        Self::test_basic_lockfree_operations().await?;
        println!("✅ Basic lock-free operations tests passed");

        // Test concurrent access patterns
        Self::test_concurrent_access_patterns().await?;
        println!("✅ Concurrent access patterns tests passed");

        // Test high-load scenarios
        Self::test_high_load_scenarios().await?;
        println!("✅ High-load scenarios tests passed");

        // Test memory consistency
        Self::test_memory_consistency().await?;
        println!("✅ Memory consistency tests passed");

        // Test edge cases
        Self::test_edge_cases().await?;
        println!("✅ Edge cases tests passed");

        println!();
        println!("🎉 All lock-free integration tests passed!");

        Ok(())
    }

    /// Test basic lock-free operations
    async fn test_basic_lockfree_operations() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test basic put/get operations
        let value = Value::String("lockfree_test".to_string());
        engine.put("key1", &value).await?;

        let retrieved = engine.get("key1").await?;
        assert_eq!(retrieved, Some(value.clone()));

        // Test multiple operations
        for i in 0..100 {
            let key = format!("key_{}", i);
            let val = Value::String(format!("value_{}", i));
            engine.put(&key, &val).await?;
        }

        // Verify all values
        for i in 0..100 {
            let key = format!("key_{}", i);
            let expected = Value::String(format!("value_{}", i));
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(expected));
        }

        Ok(())
    }

    /// Test concurrent access patterns with multiple threads
    async fn test_concurrent_access_patterns() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = Arc::new(F4KVSCore::with_config(config)?);
        let barrier = Arc::new(Barrier::new(8));

        let mut handles = vec![];

        for thread_id in 0..8 {
            let engine_clone = engine.clone();
            let barrier_clone = barrier.clone();

            let handle = tokio::spawn(async move {
                // Wait for all threads to start
                barrier_clone.wait().await;

                // Each thread performs different operations
                for i in 0..50 {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = Value::String(format!("thread_{}_value_{}", thread_id, i));

                    // Put operation
                    engine_clone.put(&key, &value).await.unwrap();

                    // Get operation
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(value));

                    // Update operation
                    let updated_value =
                        Value::String(format!("thread_{}_updated_{}", thread_id, i));
                    engine_clone.put(&key, &updated_value).await.unwrap();

                    // Verify update
                    let final_retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(final_retrieved, Some(updated_value));
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        for thread_id in 0..8 {
            for i in 0..50 {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let expected = Value::String(format!("thread_{}_updated_{}", thread_id, i));
                let retrieved = engine.get(&key).await?;
                assert_eq!(retrieved, Some(expected));
            }
        }

        Ok(())
    }

    /// Test high-load scenarios with many concurrent operations
    async fn test_high_load_scenarios() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = Arc::new(F4KVSCore::with_config(config)?);
        let barrier = Arc::new(Barrier::new(16));

        let mut handles = vec![];

        for thread_id in 0..16 {
            let engine_clone = engine.clone();
            let barrier_clone = barrier.clone();

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;

                // High-frequency operations
                for i in 0..1000 {
                    let key = format!("load_{}_{}", thread_id, i);
                    let value = Value::String(format!("load_value_{}_{}", thread_id, i));

                    // Rapid put/get cycles
                    engine_clone.put(&key, &value).await.unwrap();
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(value));

                    // Delete operation
                    engine_clone.delete(&key).await.unwrap();
                    let after_delete = engine_clone.get(&key).await.unwrap();
                    assert_eq!(after_delete, None);
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete with timeout
        let result = timeout(Duration::from_secs(30), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(result.is_ok(), "High-load test timed out");

        Ok(())
    }

    /// Test memory consistency under concurrent access
    async fn test_memory_consistency() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = Arc::new(F4KVSCore::with_config(config)?);
        let barrier = Arc::new(Barrier::new(4));

        let mut handles = vec![];

        for thread_id in 0..4 {
            let engine_clone = engine.clone();
            let barrier_clone = barrier.clone();

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;

                // Test memory consistency with overlapping keys
                for i in 0..100 {
                    let key = format!("shared_key_{}", i);
                    let value = Value::String(format!("thread_{}_value_{}", thread_id, i));

                    // Put operation
                    engine_clone.put(&key, &value).await.unwrap();

                    // Immediate get to check consistency
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert!(retrieved.is_some());

                    // Small delay to allow other threads to interfere
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify that all keys exist (last writer wins)
        for i in 0..100 {
            let key = format!("shared_key_{}", i);
            let retrieved = engine.get(&key).await?;
            assert!(retrieved.is_some());
        }

        Ok(())
    }

    /// Test edge cases for lock-free structures
    async fn test_edge_cases() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test single character key (minimum valid key)
        let single_key = "a";
        let value = Value::String("single_key_value".to_string());
        engine.put(single_key, &value).await?;
        let retrieved = engine.get(single_key).await?;
        assert_eq!(retrieved, Some(value));

        // Test long key (within limits)
        let long_key = "a".repeat(1000);
        let long_value = Value::String("long_key_value".to_string());
        engine.put(&long_key, &long_value).await?;
        let retrieved = engine.get(&long_key).await?;
        assert_eq!(retrieved, Some(long_value));

        // Test special characters in key
        let special_key = "key!@#$%^&*()_+-=[]{}|;':\",./<>?";
        let special_value = Value::String("special_key_value".to_string());
        engine.put(special_key, &special_value).await?;
        let retrieved = engine.get(special_key).await?;
        assert_eq!(retrieved, Some(special_value));

        // Test very large value
        let large_value = Value::String("x".repeat(100000));
        engine.put("large_value_key", &large_value).await?;
        let retrieved = engine.get("large_value_key").await?;
        assert_eq!(retrieved, Some(large_value));

        // Test rapid put/delete cycles
        for i in 0..1000 {
            let key = format!("rapid_{}", i);
            let value = Value::String(format!("rapid_value_{}", i));

            engine.put(&key, &value).await?;
            engine.delete(&key).await?;

            // Verify deletion
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, None);
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_lockfree_integration() {
    LockFreeIntegrationTestSuite::run_all_tests().await.unwrap();
}

// ============================================================================
// COMPREHENSIVE LOCK-FREE DATA STRUCTURE TESTS
// ============================================================================

/// Test suite for LockFreeHashMap basic operations
#[cfg(test)]
mod lockfree_hashmap_tests {
    use super::*;

    #[test]
    fn test_hashmap_creation() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_insert_and_get() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Test basic insert and get
        let old_value = map.insert("key1".to_string(), "value1".to_string());
        assert_eq!(old_value, None);

        let value = map.get(&"key1".to_string());
        assert_eq!(value, Some("value1".to_string()));

        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_hashmap_insert_update() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert initial value
        let old_value = map.insert("key1".to_string(), "value1".to_string());
        assert_eq!(old_value, None);

        // Update value
        let old_value = map.insert("key1".to_string(), "value2".to_string());
        assert_eq!(old_value, Some("value1".to_string()));

        // Verify new value
        let value = map.get(&"key1".to_string());
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_hashmap_remove() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert value
        map.insert("key1".to_string(), "value1".to_string());
        assert_eq!(map.len(), 1);

        // Remove value
        let removed = map.remove(&"key1".to_string());
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());

        // Try to remove non-existent key
        let removed = map.remove(&"nonexistent".to_string());
        assert_eq!(removed, None);
    }

    #[test]
    fn test_hashmap_contains_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        assert!(!map.contains_key(&"key1".to_string()));

        map.insert("key1".to_string(), "value1".to_string());
        assert!(map.contains_key(&"key1".to_string()));
        assert!(!map.contains_key(&"key2".to_string()));
    }

    #[test]
    fn test_hashmap_multiple_operations() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert multiple values
        for i in 0..100 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i);
            map.insert(key, value);
        }

        assert_eq!(map.len(), 100);

        // Verify all values
        for i in 0..100 {
            let key = format!("key_{}", i);
            let expected_value = format!("value_{}", i);
            let value = map.get(&key);
            assert_eq!(value, Some(expected_value));
        }
    }

    #[test]
    fn test_hashmap_clear() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert values
        for i in 0..50 {
            let key = format!("key_{}", i);
            map.insert(key, i.to_string());
        }

        assert_eq!(map.len(), 50);

        // Clear map
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_hash_collision_handling() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys that might hash to same bucket
        let keys = vec!["a", "b", "c", "d", "e", "f", "g", "h"];
        for (i, key) in keys.iter().enumerate() {
            map.insert(key.to_string(), i.to_string());
        }

        // Verify all values are retrievable
        for (i, key) in keys.iter().enumerate() {
            let value = map.get(&key.to_string());
            assert_eq!(value, Some(i.to_string()));
        }
    }

    #[test]
    fn test_hashmap_edge_cases() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Test empty string key
        map.insert("".to_string(), "empty_key".to_string());
        assert_eq!(map.get(&"".to_string()), Some("empty_key".to_string()));

        // Test very long key
        let long_key = "a".repeat(1000);
        map.insert(long_key.clone(), "long_key_value".to_string());
        assert_eq!(map.get(&long_key), Some("long_key_value".to_string()));

        // Test special characters
        let special_key = "!@#$%^&*()_+-=[]{}|;':\",./<>?";
        map.insert(special_key.to_string(), "special_value".to_string());
        assert_eq!(
            map.get(&special_key.to_string()),
            Some("special_value".to_string())
        );
    }
}

/// Test suite for LockFreeStack basic operations
#[cfg(test)]
mod lockfree_stack_tests {
    use super::*;

    #[test]
    fn test_stack_creation() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push_and_pop() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Test empty stack
        assert!(stack.is_empty());
        assert_eq!(stack.pop(), None);

        // Push values
        stack.push(1);
        assert!(!stack.is_empty());
        assert_eq!(stack.len(), 1);

        stack.push(2);
        assert_eq!(stack.len(), 2);

        // Pop values (LIFO order)
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.len(), 1);

        assert_eq!(stack.pop(), Some(1));
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);

        // Pop from empty stack
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_stack_multiple_operations() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Push multiple values
        for i in 0..100 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 100);

        // Pop all values (should be in reverse order)
        for i in (0..100).rev() {
            assert_eq!(stack.pop(), Some(i));
        }

        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_interleaved_operations() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Push some values
        stack.push(1);
        stack.push(2);
        stack.push(3);

        // Pop one
        assert_eq!(stack.pop(), Some(3));

        // Push more
        stack.push(4);
        stack.push(5);

        // Pop all remaining
        assert_eq!(stack.pop(), Some(5));
        assert_eq!(stack.pop(), Some(4));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_edge_cases() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Test with different data types
        let string_stack = LockFreeStack::new();
        string_stack.push("hello".to_string());
        string_stack.push("world".to_string());

        assert_eq!(string_stack.pop(), Some("world".to_string()));
        assert_eq!(string_stack.pop(), Some("hello".to_string()));

        // Test with large values (push many individual values)
        for i in 0..1000 {
            stack.push(i);
        }
        for i in (0..1000).rev() {
            assert_eq!(stack.pop(), Some(i));
        }
    }
}

/// Test suite for LockFreeQueue basic operations
#[cfg(test)]
mod lockfree_queue_tests {
    use super::*;

    #[test]
    fn test_queue_creation() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_enqueue_and_dequeue() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Test empty queue
        assert!(queue.is_empty());
        assert_eq!(queue.dequeue(), None);

        // Enqueue values
        queue.enqueue(1);
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        queue.enqueue(2);
        assert_eq!(queue.len(), 2);

        // Dequeue values (FIFO order)
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.len(), 1);

        assert_eq!(queue.dequeue(), Some(2));
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        // Dequeue from empty queue
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_queue_multiple_operations() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Enqueue multiple values
        for i in 0..100 {
            queue.enqueue(i);
        }

        assert_eq!(queue.len(), 100);

        // Dequeue all values (should be in order)
        for i in 0..100 {
            assert_eq!(queue.dequeue(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_interleaved_operations() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Enqueue some values
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        // Dequeue one
        assert_eq!(queue.dequeue(), Some(1));

        // Enqueue more
        queue.enqueue(4);
        queue.enqueue(5);

        // Dequeue all remaining
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), Some(4));
        assert_eq!(queue.dequeue(), Some(5));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_edge_cases() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Test with different data types
        let string_queue = LockFreeQueue::new();
        string_queue.enqueue("hello".to_string());
        string_queue.enqueue("world".to_string());

        assert_eq!(string_queue.dequeue(), Some("hello".to_string()));
        assert_eq!(string_queue.dequeue(), Some("world".to_string()));

        // Test with large values (enqueue many individual values)
        for i in 0..1000 {
            queue.enqueue(i);
        }
        for i in 0..1000 {
            assert_eq!(queue.dequeue(), Some(i));
        }
    }
}

/// Test suite for concurrent access patterns
#[cfg(test)]
mod concurrent_access_tests {
    use super::*;

    #[test]
    fn test_hashmap_concurrent_insert() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("thread_{}_value_{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all values were inserted
        assert_eq!(map.len(), num_threads * operations_per_thread);

        for thread_id in 0..num_threads {
            for i in 0..operations_per_thread {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let expected_value = format!("thread_{}_value_{}", thread_id, i);
                let value = map.get(&key);
                assert_eq!(value, Some(expected_value));
            }
        }
    }

    #[test]
    fn test_hashmap_concurrent_read_write() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let num_threads = 4;
        let operations_per_thread = 50;

        // Pre-populate with some data
        for i in 0..100 {
            let key = format!("key_{}", i);
            map.insert(key, i.to_string());
        }

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("thread_{}_value_{}", thread_id, i);

                    // Insert new value
                    map_clone.insert(key.clone(), value.clone());

                    // Read it back
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Read some pre-existing values
                    let existing_key = format!("key_{}", i % 100);
                    let existing_value = map_clone.get(&existing_key);
                    assert!(existing_value.is_some());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_stack_concurrent_push_pop() {
        let stack = Arc::new(LockFreeStack::new());
        let num_threads = 4;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let stack_clone = stack.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let value = thread_id * operations_per_thread + i;
                    stack_clone.push(value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Collect all values by popping
        let mut collected_values = vec![];
        while let Some(value) = stack.pop() {
            collected_values.push(value);
        }

        // Verify we got all expected values
        assert_eq!(collected_values.len(), num_threads * operations_per_thread);
    }

    #[test]
    fn test_queue_concurrent_enqueue_dequeue() {
        let queue = Arc::new(LockFreeQueue::new());
        let num_threads = 4;
        let operations_per_thread = 100;

        let mut handles = vec![];

        // Spawn enqueue threads
        for thread_id in 0..num_threads {
            let queue_clone = queue.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let value = thread_id * operations_per_thread + i;
                    queue_clone.enqueue(value);
                }
            });
            handles.push(handle);
        }

        // Spawn dequeue threads
        let dequeue_count = Arc::new(AtomicUsize::new(0));
        for _ in 0..num_threads {
            let queue_clone = queue.clone();
            let count_clone = dequeue_count.clone();
            let handle = thread::spawn(move || {
                while count_clone.load(Ordering::Relaxed) < num_threads * operations_per_thread {
                    if let Some(_) = queue_clone.dequeue() {
                        count_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify queue is empty
        assert!(queue.is_empty());
    }

    #[test]
    fn test_memory_safety_under_load() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let num_threads = 8;
        let operations_per_thread = 1000;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("thread_{}_value_{}", thread_id, i);

                    // Insert
                    map_clone.insert(key.clone(), value.clone());

                    // Read
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Remove
                    let removed = map_clone.remove(&key);
                    assert!(removed.is_some());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Map should be empty after all operations
        assert!(map.is_empty());
    }
}

/// Test suite for error handling and edge cases
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_hashmap_capacity_limits() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 2, // Small initial capacity
            load_factor: 0.75,
            max_buckets: 1024 * 1024,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert many values to test resize behavior
        for i in 0..1000 {
            let key = format!("key_{}", i);
            map.insert(key, i.to_string());
        }

        // Verify all values are still accessible
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let value = map.get(&key);
            assert_eq!(value, Some(i.to_string()));
        }
    }

    #[test]
    fn test_stack_rapid_operations() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Rapid push/pop cycles
        for _ in 0..1000 {
            stack.push(42);
            assert_eq!(stack.pop(), Some(42));
        }

        assert!(stack.is_empty());
    }

    #[test]
    fn test_queue_rapid_operations() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Rapid enqueue/dequeue cycles
        for i in 0..1000 {
            queue.enqueue(i);
            assert_eq!(queue.dequeue(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_mixed_operations() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);
        let stack: LockFreeStack<i32> = LockFreeStack::new();
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Perform mixed operations across all structures
        for i in 0..100 {
            // HashMap operations
            let key = format!("key_{}", i);
            map.insert(key, i.to_string());

            // Stack operations
            stack.push(i);

            // Queue operations
            queue.enqueue(i);
        }

        // Verify all structures have correct state
        assert_eq!(map.len(), 100);
        assert_eq!(stack.len(), 100);
        assert_eq!(queue.len(), 100);

        // Pop from stack (LIFO)
        for i in (0..100).rev() {
            assert_eq!(stack.pop(), Some(i));
        }

        // Dequeue from queue (FIFO)
        for i in 0..100 {
            assert_eq!(queue.dequeue(), Some(i));
        }

        // Verify map still has all values
        for i in 0..100 {
            let key = format!("key_{}", i);
            assert_eq!(map.get(&key), Some(i.to_string()));
        }
    }
}
