//! Basic lock-free data structure tests
//!
//! This module provides comprehensive unit tests for lock-free data structures
//! focusing on basic operations, edge cases, and concurrent access patterns.

use f4kvs_core::safe_concurrency_wrappers::{
    SafeConcurrentHashMap, SafeConcurrentHashMapConfig, SafeConcurrentQueue, SafeConcurrentStack,
};
use std::sync::Arc;
use std::thread;

/// Test suite for LockFreeHashMap basic operations
#[cfg(test)]
mod hashmap_basic_tests {
    use super::*;

    #[test]
    fn test_hashmap_creation() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_insert_and_get() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        assert!(!map.contains_key(&"key1".to_string()));

        map.insert("key1".to_string(), "value1".to_string());
        assert!(map.contains_key(&"key1".to_string()));
        assert!(!map.contains_key(&"key2".to_string()));
    }

    #[test]
    fn test_hashmap_multiple_operations() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
    fn test_hashmap_edge_cases() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test empty string key
        map.insert("".to_string(), "empty_key".to_string());
        assert_eq!(map.get(&"".to_string()), Some("empty_key".to_string()));

        // Test very long key
        let long_key = "a".repeat(1000);
        map.insert(long_key.clone(), "long_key_value".to_string());
        assert_eq!(map.get(&long_key), Some("long_key_value".to_string()));

        // Test special characters
        let special_key = "!@#$%^&*()_+-=[]{}|;':\",./<>?".to_string();
        map.insert(special_key.clone(), "special_value".to_string());
        assert_eq!(map.get(&special_key), Some("special_value".to_string()));
    }
}

/// Test suite for LockFreeStack basic operations
#[cfg(test)]
mod stack_basic_tests {
    use super::*;

    #[test]
    fn test_stack_creation() {
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push_and_pop() {
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();

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
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();

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
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();

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
}

/// Test suite for LockFreeQueue basic operations
#[cfg(test)]
mod queue_basic_tests {
    use super::*;

    #[test]
    fn test_queue_creation() {
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_enqueue_and_dequeue() {
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();

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
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();

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
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();

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
}

/// Test suite for concurrent access patterns
#[cfg(test)]
mod concurrent_tests {
    use super::*;

    #[test]
    fn test_hashmap_concurrent_insert() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
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
    fn test_stack_concurrent_push_pop() {
        let stack = Arc::new(SafeConcurrentStack::<i32>::new());
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
        assert_eq!(
            collected_values.len(),
            (num_threads * operations_per_thread) as usize
        );
    }

    #[test]
    fn test_queue_concurrent_enqueue_dequeue() {
        let queue = Arc::new(SafeConcurrentQueue::<i32>::new());
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
        for _ in 0..num_threads {
            let queue_clone = queue.clone();
            let handle = thread::spawn(move || {
                let mut count = 0;
                while count < operations_per_thread {
                    if let Some(_) = queue_clone.dequeue() {
                        count += 1;
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
}

/// Test suite for error handling and edge cases
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_hashmap_capacity_limits() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

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
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();

        // Rapid push/pop cycles
        for _ in 0..1000 {
            stack.push(42);
            assert_eq!(stack.pop(), Some(42));
        }

        assert!(stack.is_empty());
    }

    #[test]
    fn test_queue_rapid_operations() {
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();

        // Rapid enqueue/dequeue cycles
        for i in 0..1000 {
            queue.enqueue(i);
            assert_eq!(queue.dequeue(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_mixed_operations() {
        let map: SafeConcurrentHashMap<String, i32> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let stack: SafeConcurrentStack<i32> = SafeConcurrentStack::new();
        let queue: SafeConcurrentQueue<i32> = SafeConcurrentQueue::new();

        // Perform mixed operations across all structures
        for i in 0..100 {
            // HashMap operations
            let key = format!("key_{}", i);
            map.insert(key, i);

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
            assert_eq!(map.get(&key), Some(i));
        }
    }
}
