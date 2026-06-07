//! Comprehensive tests for lock-free data structures
//!
//! This module provides extensive tests to increase code coverage for lock-free
//! structures, focusing on edge cases, error handling, and concurrent scenarios.

use f4kvs_core::lockfree::{LockFreeHashMap, LockFreeHashMapConfig, LockFreeQueue, LockFreeStack};
use std::sync::Arc;
use std::thread;

#[cfg(test)]
mod hashmap_comprehensive_tests {
    use super::*;

    #[test]
    fn test_hashmap_with_custom_config() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 16,
            load_factor: 0.5,
            max_buckets: 1024,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);
        assert!(map.is_empty());
    }

    #[test]
    fn test_hashmap_insert_many_keys() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert many keys to test resizing
        for i in 0..1000 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        assert_eq!(map.len(), 1000);

        // Verify all keys are present
        for i in 0..1000 {
            assert_eq!(map.get(&format!("key{}", i)), Some(format!("value{}", i)));
        }
    }

    #[test]
    fn test_hashmap_remove_all() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys
        for i in 0..100 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        // Remove all keys
        for i in 0..100 {
            assert_eq!(
                map.remove(&format!("key{}", i)),
                Some(format!("value{}", i))
            );
        }

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_contains_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        map.insert("key1".to_string(), "value1".to_string());
        assert!(map.contains_key(&"key1".to_string()));
        assert!(!map.contains_key(&"key2".to_string()));
    }

    #[test]
    fn test_hashmap_clear() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys
        for i in 0..50 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        assert_eq!(map.len(), 50);
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        // Verify all keys are gone
        for i in 0..50 {
            assert_eq!(map.get(&format!("key{}", i)), None);
        }
    }

    #[test]
    fn test_hashmap_concurrent_insert() {
        let map = Arc::new(LockFreeHashMap::new(LockFreeHashMapConfig::default()));
        let mut handles = vec![];

        // Spawn threads that insert different keys
        for thread_id in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = format!("thread{}_key{}", thread_id, i);
                    let value = format!("thread{}_value{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(map.len(), 1000);
    }

    #[test]
    fn test_hashmap_concurrent_get() {
        let map = Arc::new(LockFreeHashMap::new(LockFreeHashMapConfig::default()));

        // Pre-populate
        for i in 0..100 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        let mut handles = vec![];
        for _ in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = format!("key{}", i);
                    let value = map_clone.get(&key);
                    assert_eq!(value, Some(format!("value{}", i)));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_hashmap_concurrent_remove() {
        let map = Arc::new(LockFreeHashMap::new(LockFreeHashMapConfig::default()));

        // Pre-populate
        for i in 0..100 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        let mut handles = vec![];
        for thread_id in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..10 {
                    let key = format!("key{}", thread_id * 10 + i);
                    let _ = map_clone.remove(&key);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_concurrent_mixed_operations() {
        let map = Arc::new(LockFreeHashMap::new(LockFreeHashMapConfig::default()));
        let mut handles = vec![];

        // Mix of insert, get, remove operations
        for thread_id in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = format!("thread{}_key{}", thread_id, i);
                    map_clone.insert(key.clone(), format!("value{}", i));
                    let _ = map_clone.get(&key);
                    if i % 2 == 0 {
                        let _ = map_clone.remove(&key);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_hashmap_hash_collisions() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 4, // Small bucket count to force collisions
            load_factor: 0.75,
            max_buckets: 16,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys that may hash to same bucket
        for i in 0..20 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        // Verify all keys are still accessible
        for i in 0..20 {
            assert_eq!(map.get(&format!("key{}", i)), Some(format!("value{}", i)));
        }
    }

    #[test]
    fn test_hashmap_update_existing_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        map.insert("key1".to_string(), "value1".to_string());
        assert_eq!(map.get(&"key1".to_string()), Some("value1".to_string()));

        let old = map.insert("key1".to_string(), "value2".to_string());
        assert_eq!(old, Some("value1".to_string()));
        assert_eq!(map.get(&"key1".to_string()), Some("value2".to_string()));
    }

    #[test]
    fn test_hashmap_remove_nonexistent() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        assert_eq!(map.remove(&"nonexistent".to_string()), None);
    }

    #[test]
    fn test_hashmap_get_nonexistent() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        assert_eq!(map.get(&"nonexistent".to_string()), None);
    }
}

#[cfg(test)]
mod queue_comprehensive_tests {
    use super::*;

    #[test]
    fn test_queue_enqueue_dequeue_sequence() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        // Enqueue sequence
        for i in 0..100 {
            queue.enqueue(i);
        }

        assert_eq!(queue.len(), 100);

        // Dequeue sequence (FIFO)
        for i in 0..100 {
            assert_eq!(queue.dequeue(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_interleaved_enqueue_dequeue() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        queue.enqueue(1);
        queue.enqueue(2);
        assert_eq!(queue.dequeue(), Some(1));
        queue.enqueue(3);
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_concurrent_enqueue() {
        let queue = Arc::new(LockFreeQueue::<i32>::new());
        let mut handles = vec![];

        for thread_id in 0..10 {
            let queue_clone = Arc::clone(&queue);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    queue_clone.enqueue(thread_id * 100 + i);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(queue.len(), 1000);
    }

    #[test]
    fn test_queue_concurrent_dequeue() {
        let queue = Arc::new(LockFreeQueue::<i32>::new());

        // Pre-populate
        for i in 0..1000 {
            queue.enqueue(i);
        }

        let mut handles = vec![];
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));

        for _ in 0..10 {
            let queue_clone = Arc::clone(&queue);
            let results_clone = Arc::clone(&results);
            let handle = thread::spawn(move || {
                let mut local_results = Vec::new();
                for _ in 0..100 {
                    if let Some(value) = queue_clone.dequeue() {
                        local_results.push(value);
                    }
                }
                results_clone.lock().unwrap().extend(local_results);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(results.lock().unwrap().len(), 1000);
    }

    #[test]
    fn test_queue_concurrent_mixed() {
        let queue = Arc::new(LockFreeQueue::<i32>::new());
        let mut handles = vec![];

        // Mix of enqueue and dequeue
        for thread_id in 0..10 {
            let queue_clone = Arc::clone(&queue);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    queue_clone.enqueue(thread_id * 50 + i);
                    if i % 2 == 0 {
                        let _ = queue_clone.dequeue();
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_queue_empty_dequeue() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_queue_len_accuracy() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();

        assert_eq!(queue.len(), 0);
        queue.enqueue(1);
        assert_eq!(queue.len(), 1);
        queue.enqueue(2);
        assert_eq!(queue.len(), 2);
        queue.dequeue();
        assert_eq!(queue.len(), 1);
        queue.dequeue();
        assert_eq!(queue.len(), 0);
    }
}

#[cfg(test)]
mod stack_comprehensive_tests {
    use super::*;

    #[test]
    fn test_stack_push_pop_sequence() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        // Push sequence
        for i in 0..100 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 100);

        // Pop sequence (LIFO)
        for i in (0..100).rev() {
            assert_eq!(stack.pop(), Some(i));
        }

        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_interleaved_push_pop() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        stack.push(1);
        stack.push(2);
        assert_eq!(stack.pop(), Some(2));
        stack.push(3);
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_concurrent_push() {
        let stack = Arc::new(LockFreeStack::<i32>::new());
        let mut handles = vec![];

        for thread_id in 0..10 {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    stack_clone.push(thread_id * 100 + i);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(stack.len(), 1000);
    }

    #[test]
    fn test_stack_concurrent_pop() {
        let stack = Arc::new(LockFreeStack::<i32>::new());

        // Pre-populate
        for i in 0..1000 {
            stack.push(i);
        }

        let mut handles = vec![];
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));

        for _ in 0..10 {
            let stack_clone = Arc::clone(&stack);
            let results_clone = Arc::clone(&results);
            let handle = thread::spawn(move || {
                let mut local_results = Vec::new();
                for _ in 0..100 {
                    if let Some(value) = stack_clone.pop() {
                        local_results.push(value);
                    }
                }
                results_clone.lock().unwrap().extend(local_results);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(results.lock().unwrap().len(), 1000);
    }

    #[test]
    fn test_stack_concurrent_mixed() {
        let stack = Arc::new(LockFreeStack::<i32>::new());
        let mut handles = vec![];

        // Mix of push and pop
        for thread_id in 0..10 {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    stack_clone.push(thread_id * 50 + i);
                    if i % 2 == 0 {
                        let _ = stack_clone.pop();
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_stack_empty_pop() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_stack_len_accuracy() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();

        assert_eq!(stack.len(), 0);
        stack.push(1);
        assert_eq!(stack.len(), 1);
        stack.push(2);
        assert_eq!(stack.len(), 2);
        stack.pop();
        assert_eq!(stack.len(), 1);
        stack.pop();
        assert_eq!(stack.len(), 0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_hashmap_queue_integration() {
        let map = LockFreeHashMap::new(LockFreeHashMapConfig::default());
        let queue: LockFreeQueue<String> = LockFreeQueue::new();

        // Use queue to track insertion order
        for i in 0..10 {
            let key = format!("key{}", i);
            map.insert(key.clone(), format!("value{}", i));
            queue.enqueue(key);
        }

        // Process in order
        while let Some(key) = queue.dequeue() {
            assert!(map.contains_key(&key));
        }
    }

    #[test]
    fn test_all_structures_together() {
        let map = LockFreeHashMap::new(LockFreeHashMapConfig::default());
        let queue: LockFreeQueue<String> = LockFreeQueue::new();
        let stack: LockFreeStack<String> = LockFreeStack::new();

        // Insert into map
        for i in 0..10 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        // Queue keys for processing
        for i in 0..10 {
            queue.enqueue(format!("key{}", i));
        }

        // Process from queue and push to stack
        while let Some(key) = queue.dequeue() {
            if let Some(value) = map.get(&key) {
                stack.push(value);
            }
        }

        // Verify stack has all values
        assert_eq!(stack.len(), 10);
    }
}
