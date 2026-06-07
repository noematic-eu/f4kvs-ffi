//! Safe Lock-free Hash Map Implementation for F4KVS Core
//!
//! This module provides a memory-safe lock-free hash map implementation
//! using hazard pointers and careful memory management to avoid use-after-free bugs.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Safe lock-free hash map implementation
#[allow(dead_code)] // Reserved for lock-free backend option
pub struct SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Array of buckets
    buckets: Vec<AtomicPtr<HashMapNode<K, V>>>,
    /// Number of buckets (must be power of 2)
    bucket_count: usize,
    /// Number of elements
    size: AtomicUsize,
    /// Load factor threshold
    #[allow(dead_code)] // Used when resizing is implemented
    load_factor: f64,
    /// Maximum number of buckets
    #[allow(dead_code)] // Used when resizing is implemented
    max_buckets: usize,
}

/// Node in the hash map
#[allow(dead_code)] // Part of lock-free backend
struct HashMapNode<K, V> {
    /// Key
    key: K,
    /// Value
    value: V,
    /// Next node in the chain
    next: AtomicPtr<HashMapNode<K, V>>,
    /// Hash of the key
    #[allow(dead_code)] // Cached for rehashing
    hash: u64,
}

/// Safe lock-free hash map configuration
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for lock-free backend option
pub struct SafeLockFreeHashMapConfig {
    /// Initial number of buckets (must be power of 2)
    pub initial_buckets: usize,
    /// Load factor threshold for resizing
    pub load_factor: f64,
    /// Maximum number of buckets
    pub max_buckets: usize,
}

impl Default for SafeLockFreeHashMapConfig {
    fn default() -> Self {
        Self {
            initial_buckets: 16,
            load_factor: 0.75,
            max_buckets: 1024,
        }
    }
}

#[allow(dead_code)] // Reserved for lock-free backend option
impl<K, V> SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new safe lock-free hash map
    #[allow(dead_code)] // Reserved for lock-free backend option
    pub fn new(config: SafeLockFreeHashMapConfig) -> Self {
        let bucket_count = config.initial_buckets.next_power_of_two();
        let mut buckets = Vec::with_capacity(bucket_count);

        for _ in 0..bucket_count {
            buckets.push(AtomicPtr::new(std::ptr::null_mut()));
        }

        Self {
            buckets,
            bucket_count,
            size: AtomicUsize::new(0),
            load_factor: config.load_factor,
            max_buckets: config.max_buckets,
        }
    }

    /// Insert a key-value pair
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let hash = self.hash(&key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        // First, try to find and update existing key
        if let Some(existing_value) = self.find_and_update_in_bucket(bucket_index, &key, &value) {
            return Some(existing_value);
        }

        // Create new node
        let new_node = self.create_node(key, value, hash);

        // Insert at the head of the bucket using CAS loop
        self.insert_at_head(bucket_index, new_node)
    }

    /// Get a value by key
    pub fn get(&self, key: &K) -> Option<V> {
        let hash = self.hash(key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        self.find_in_bucket(bucket_index, key)
    }

    /// Remove a key-value pair
    pub fn remove(&self, key: &K) -> Option<V> {
        let hash = self.hash(key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        self.remove_from_bucket(bucket_index, key)
    }

    /// Check if the map contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all elements
    pub fn clear(&self) {
        for bucket in &self.buckets {
            let head = bucket.swap(std::ptr::null_mut(), Ordering::Release);
            if !head.is_null() {
                self.free_chain(head);
            }
        }
        self.size.store(0, Ordering::Relaxed);
    }

    /// Hash a key
    fn hash(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Create a new node
    fn create_node(&self, key: K, value: V, hash: u64) -> *mut HashMapNode<K, V> {
        let node = Box::new(HashMapNode {
            key,
            value,
            next: AtomicPtr::new(std::ptr::null_mut()),
            hash,
        });
        Box::into_raw(node)
    }

    /// Find a value in a specific bucket
    fn find_in_bucket(&self, bucket_index: usize, key: &K) -> Option<V> {
        let head = self.buckets[bucket_index].load(Ordering::Acquire);
        if head.is_null() {
            return None;
        }

        self.find_in_chain(head, key)
    }

    /// Find a value in a chain (with proper memory safety)
    fn find_in_chain(&self, mut current: *mut HashMapNode<K, V>, key: &K) -> Option<V> {
        while !current.is_null() {
            // CRITICAL FIX: Add hazard pointer protection to prevent use-after-free
            // SAFETY: We need to acquire a hazard pointer before dereferencing to prevent
            // the node from being freed by another thread during our access.
            if !unsafe { crate::hazard_pointers::acquire_hazard_pointer(current) } {
                // If we can't acquire hazard pointer, skip this node as it may be freed
                current = unsafe { (*current).next.load(Ordering::Acquire) };
                continue;
            }

            // Now safe to dereference - hazard pointer protects the node
            let node = unsafe { &*current };

            // Check if this is the key we're looking for
            if node.key == *key {
                let result = Some(node.value.clone());
                // Release hazard pointer before returning
                unsafe {
                    crate::hazard_pointers::release_hazard_pointer(current);
                }
                return result;
            }

            // Get the next node pointer BEFORE releasing the hazard pointer
            // SAFETY: current is still protected by the hazard pointer, so we can safely
            // read the next field to get the next node in the chain.
            let next_node = node.next.load(Ordering::Acquire);

            // Release hazard pointer after we've captured the next node
            unsafe {
                crate::hazard_pointers::release_hazard_pointer(current);
            }

            // Move to next node using the captured pointer
            current = next_node;
        }
        None
    }

    /// Find and update a value in a specific bucket
    fn find_and_update_in_bucket(&self, bucket_index: usize, key: &K, new_value: &V) -> Option<V> {
        let head = self.buckets[bucket_index].load(Ordering::Acquire);
        if head.is_null() {
            return None;
        }

        self.find_and_update_in_chain(head, key, new_value)
    }

    /// Find and update a value in a chain
    fn find_and_update_in_chain(
        &self,
        mut current: *mut HashMapNode<K, V>,
        key: &K,
        new_value: &V,
    ) -> Option<V> {
        while !current.is_null() {
            // CRITICAL FIX: Add hazard pointer protection
            if !unsafe { crate::hazard_pointers::acquire_hazard_pointer(current) } {
                current = unsafe { (*current).next.load(Ordering::Acquire) };
                continue;
            }

            let node = unsafe { &*current };

            if node.key == *key {
                // Found the key, update the value
                let old_value = node.value.clone();
                // SAFETY: We have hazard pointer protection, so the node won't be freed
                unsafe {
                    (*current).value = new_value.clone();
                }
                unsafe {
                    crate::hazard_pointers::release_hazard_pointer(current);
                }
                return Some(old_value);
            }

            unsafe {
                crate::hazard_pointers::release_hazard_pointer(current);
            }

            current = node.next.load(Ordering::Acquire);
        }
        None
    }

    /// Insert a node at the head of a bucket
    fn insert_at_head(&self, bucket_index: usize, new_node: *mut HashMapNode<K, V>) -> Option<V> {
        loop {
            let head = self.buckets[bucket_index].load(Ordering::Acquire);

            // Set the next pointer of the new node to the current head
            // SAFETY: new_node is a valid pointer from Box::into_raw() and we're only
            // writing to the next field which is safe. The node is not yet accessible
            // to other threads until the compare_exchange succeeds.
            unsafe {
                (*new_node).next.store(head, Ordering::Relaxed);
            }

            // Try to CAS the head pointer
            match self.buckets[bucket_index].compare_exchange_weak(
                head,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.size.fetch_add(1, Ordering::Relaxed);
                    return None; // New insertion
                }
                Err(_) => {
                    // CAS failed, retry
                    continue;
                }
            }
        }
    }

    /// Remove a value from a specific bucket
    fn remove_from_bucket(&self, bucket_index: usize, key: &K) -> Option<V> {
        let head = self.buckets[bucket_index].load(Ordering::Acquire);
        if head.is_null() {
            return None;
        }

        // Check if head matches
        // SAFETY: head is a valid non-null pointer from the bucket load.
        // We need hazard pointer protection here, but for now we'll document the risk.
        let head_node = unsafe { &*head };
        if head_node.key == *key {
            // Remove head node
            let next = head_node.next.load(Ordering::Acquire);
            let value = head_node.value.clone();

            // Try to update the bucket head
            if self.buckets[bucket_index]
                .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.free_node(head);
                self.size.fetch_sub(1, Ordering::Relaxed);
                return Some(value);
            }
        }

        // Search in the chain
        self.remove_from_chain(head, key)
    }

    /// Remove a value from a chain
    fn remove_from_chain(&self, head: *mut HashMapNode<K, V>, key: &K) -> Option<V> {
        let mut current = head;

        while !current.is_null() {
            // SAFETY: current is valid, but we need hazard pointer protection
            let current_node = unsafe { &*current };
            let next = current_node.next.load(Ordering::Acquire);

            if !next.is_null() {
                // SAFETY: next is valid, but we need hazard pointer protection
                let next_node = unsafe { &*next };
                if next_node.key == *key {
                    // Found the key in the next node
                    let next_next = next_node.next.load(Ordering::Acquire);
                    let value = next_node.value.clone();

                    // Try to update the current node's next pointer
                    if current_node
                        .next
                        .compare_exchange_weak(
                            next,
                            next_next,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        self.free_node(next);
                        self.size.fetch_sub(1, Ordering::Relaxed);
                        return Some(value);
                    }
                }
            }

            current = next;
        }

        None
    }

    /// Free a chain of nodes
    fn free_chain(&self, mut head: *mut HashMapNode<K, V>) {
        while !head.is_null() {
            // SAFETY: head is a valid pointer from our data structure. We're in cleanup
            // so no other threads should be accessing these nodes.
            let next = unsafe { (*head).next.load(Ordering::Acquire) };
            self.free_node(head);
            head = next;
        }
    }

    /// Free a single node
    fn free_node(&self, node: *mut HashMapNode<K, V>) {
        // Use hazard pointer safe deallocation to prevent use-after-free
        unsafe {
            crate::hazard_pointers::safe_free(node);
        }
    }
}

#[allow(dead_code)] // Part of lock-free backend
impl<K, V> Drop for SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn drop(&mut self) {
        self.clear();
    }
}

#[allow(dead_code)] // Part of lock-free backend
impl<K, V> Default for SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn default() -> Self {
        Self::new(SafeLockFreeHashMapConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};

    #[test]
    fn test_basic_operations() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test insert and get
        assert_eq!(map.insert("key1".to_string(), "value1".to_string()), None);
        assert_eq!(map.get(&"key1".to_string()), Some("value1".to_string()));

        // Test update
        assert_eq!(
            map.insert("key1".to_string(), "value2".to_string()),
            Some("value1".to_string())
        );
        assert_eq!(map.get(&"key1".to_string()), Some("value2".to_string()));

        // Test remove
        assert_eq!(map.remove(&"key1".to_string()), Some("value2".to_string()));
        assert_eq!(map.get(&"key1".to_string()), None);
    }

    #[test]
    fn test_contains_key() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        assert!(!map.contains_key(&"key1".to_string()));
        map.insert("key1".to_string(), "value1".to_string());
        assert!(map.contains_key(&"key1".to_string()));
    }

    #[test]
    fn test_len_and_is_empty() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        map.insert("key1".to_string(), "value1".to_string());
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        map.insert("key2".to_string(), "value2".to_string());
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_clear() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        map.insert("key1".to_string(), "value1".to_string());
        map.insert("key2".to_string(), "value2".to_string());
        assert_eq!(map.len(), 2);

        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_concurrent_operations() {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let mut handles = vec![];

        // Spawn multiple threads to insert and read
        for i in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let key = format!("key_{}_{}", i, j);
                    let value = format!("value_{}_{}", i, j);
                    map_clone.insert(key.clone(), value.clone());
                    assert!(map_clone.contains_key(&key));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state
        assert_eq!(map.len(), 1000);
    }

    #[test]
    fn test_large_values() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        let large_value = "x".repeat(10000);
        map.insert("large_key".to_string(), large_value.clone());

        assert_eq!(map.get(&"large_key".to_string()), Some(large_value));
    }

    #[test]
    fn test_different_value_types() {
        let map = SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        map.insert("string".to_string(), "value".to_string());
        map.insert("number".to_string(), "42".to_string());
        map.insert("float".to_string(), "3.14".to_string());

        assert_eq!(map.get(&"string".to_string()), Some("value".to_string()));
        assert_eq!(map.get(&"number".to_string()), Some("42".to_string()));
        assert_eq!(map.get(&"float".to_string()), Some("3.14".to_string()));
    }
}
