//! Safe Concurrency Wrappers
//!
//! This module provides safe, production-tested alternatives to the unsafe
//! lock-free implementations. These wrappers use battle-tested libraries
//! like DashMap and crossbeam to provide thread-safe concurrent data structures
//! without the memory safety issues of custom unsafe implementations.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use std::sync::{Arc, Mutex};

/// Safe concurrent hash map using DashMap
///
/// This is a drop-in replacement for the unsafe LockFreeHashMap that uses
/// DashMap internally, which is a production-tested, memory-safe concurrent
/// hash map implementation.
pub struct SafeConcurrentHashMap<K, V> {
    inner: Arc<DashMap<K, V>>,
}

impl<K, V> SafeConcurrentHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new safe concurrent hash map
    pub fn new(_config: SafeConcurrentHashMapConfig) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Get a value by key
    pub fn get(&self, key: &K) -> Option<V>
    where
        K: std::hash::Hash + Eq,
    {
        self.inner.get(key).map(|entry| entry.value().clone())
    }

    /// Remove a key-value pair
    pub fn remove(&self, key: &K) -> Option<V>
    where
        K: std::hash::Hash + Eq,
    {
        self.inner.remove(key).map(|(_, value)| value)
    }

    /// Check if the map contains a key
    pub fn contains_key(&self, key: &K) -> bool
    where
        K: std::hash::Hash + Eq,
    {
        self.inner.contains_key(key)
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear all elements
    pub fn clear(&self) {
        self.inner.clear();
    }
}

/// Configuration for safe concurrent hash map
#[derive(Debug, Clone)]
pub struct SafeConcurrentHashMapConfig {
    /// Initial capacity (ignored for DashMap)
    pub initial_capacity: usize,
    /// Load factor (ignored for DashMap)
    pub load_factor: f64,
}

impl Default for SafeConcurrentHashMapConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 16,
            load_factor: 0.75,
        }
    }
}

/// Safe concurrent queue using crossbeam SegQueue
///
/// This is a drop-in replacement for the unsafe LockFreeQueue that uses
/// crossbeam's SegQueue, which is a production-tested, memory-safe concurrent
/// queue implementation.
pub struct SafeConcurrentQueue<T> {
    inner: Arc<SegQueue<T>>,
}

impl<T> SafeConcurrentQueue<T> {
    /// Create a new safe concurrent queue
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
        }
    }

    /// Enqueue an item
    pub fn enqueue(&self, item: T) {
        self.inner.push(item);
    }

    /// Dequeue an item
    pub fn dequeue(&self) -> Option<T> {
        self.inner.pop()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of elements (approximate)
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T> Default for SafeConcurrentQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe concurrent stack using Vec with Mutex for LIFO behavior
///
/// This is a drop-in replacement for the unsafe LockFreeStack that uses
/// a Vec with Mutex for proper LIFO (Last In, First Out) behavior.
pub struct SafeConcurrentStack<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> SafeConcurrentStack<T> {
    /// Create a new safe concurrent stack
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Push an item onto the stack
    pub fn push(&self, item: T) {
        if let Ok(mut vec) = self.inner.lock() {
            vec.push(item);
        }
    }

    /// Pop an item from the stack
    pub fn pop(&self) -> Option<T> {
        if let Ok(mut vec) = self.inner.lock() {
            vec.pop()
        } else {
            None
        }
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        if let Ok(vec) = self.inner.lock() {
            vec.is_empty()
        } else {
            true
        }
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        if let Ok(vec) = self.inner.lock() {
            vec.len()
        } else {
            0
        }
    }
}

impl<T> Default for SafeConcurrentStack<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_safe_concurrent_hashmap_basic_operations() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test insert
        assert_eq!(map.insert("key1", "value1"), None);
        assert_eq!(map.get(&"key1"), Some("value1"));

        // Test overwrite
        assert_eq!(map.insert("key1", "value2"), Some("value1"));
        assert_eq!(map.get(&"key1"), Some("value2"));

        // Test remove
        assert_eq!(map.remove(&"key1"), Some("value2"));
        assert_eq!(map.get(&"key1"), None);
    }

    #[test]
    fn test_safe_concurrent_hashmap_concurrent_access() {
        let map = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let mut handles = vec![];

        // Spawn multiple threads to insert data
        for i in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let key = format!("key_{}_{}", i, j);
                    let value = format!("value_{}_{}", i, j);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all data is accessible
        assert_eq!(map.len(), 1000);
    }

    #[test]
    fn test_safe_concurrent_queue_basic_operations() {
        let queue = SafeConcurrentQueue::new();

        assert!(queue.is_empty());

        // Test enqueue
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert!(!queue.is_empty());

        // Test dequeue
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_safe_concurrent_stack_basic_operations() {
        let stack = SafeConcurrentStack::new();

        assert!(stack.is_empty());

        // Test push
        stack.push(1);
        stack.push(2);
        stack.push(3);

        assert!(!stack.is_empty());

        // Test pop (LIFO order)
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
    }
}
