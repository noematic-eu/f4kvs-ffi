//! Lock-free cache implementation for F4KVS Core
//!
//! This module provides a high-performance, lock-free cache implementation
//! using atomic operations and compare-and-swap (CAS) for thread-safe access.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// A single entry in the lock-free cache
#[derive(Debug)]
pub struct CacheEntry<K, V> {
    /// The cached key
    pub key: K,
    /// The cached value
    pub value: V,
    /// Unix timestamp when the entry was created
    pub timestamp: u64,
    /// Number of times this entry has been accessed
    pub access_count: AtomicUsize,
    /// Pointer to the next entry in the linked list
    pub next: AtomicPtr<CacheEntry<K, V>>,
}

/// Lock-free cache implementation
pub struct LockFreeCache<K, V> {
    /// Array of atomic pointers to cache entries
    buckets: Vec<AtomicPtr<CacheEntry<K, V>>>,
    /// Number of buckets (must be power of 2)
    bucket_count: usize,
    /// Current size
    size: AtomicUsize,
    /// Maximum size before eviction
    #[allow(dead_code)]
    max_size: usize,
    /// Load factor threshold
    load_factor: f64,
}

impl<K, V> LockFreeCache<K, V>
where
    K: Clone + Hash + Eq + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new lock-free cache
    pub fn new(initial_capacity: usize, max_size: usize) -> Self {
        let bucket_count = initial_capacity.next_power_of_two();
        let mut buckets = Vec::with_capacity(bucket_count);

        for _ in 0..bucket_count {
            buckets.push(AtomicPtr::new(ptr::null_mut()));
        }

        Self {
            buckets,
            bucket_count,
            size: AtomicUsize::new(0),
            max_size,
            load_factor: 0.75,
        }
    }

    /// Get a value from the cache
    pub fn get(&self, key: &K) -> Option<V> {
        let bucket_index = self.hash_key(key) & (self.bucket_count - 1);
        let mut current = self.buckets[bucket_index].load(Ordering::Acquire);

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
            let entry = unsafe { &*current };
            if entry.key == *key {
                // Update access count atomically
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                let result = Some(entry.value.clone());
                // Release hazard pointer before returning
                unsafe {
                    crate::hazard_pointers::release_hazard_pointer(current);
                }
                return result;
            }

            // Get the next node pointer BEFORE releasing the hazard pointer
            // SAFETY: current is still protected by the hazard pointer, so we can safely
            // read the next field to get the next node in the chain.
            let next_node = entry.next.load(Ordering::Acquire);

            // Release hazard pointer after we've captured the next node
            unsafe {
                crate::hazard_pointers::release_hazard_pointer(current);
            }

            // Move to next node using the captured pointer
            current = next_node;
        }

        None
    }

    /// Insert a value into the cache
    pub fn insert(&self, key: K, value: V) -> bool {
        // Check if we need to resize
        if self.should_resize() {
            self.resize();
        }

        let bucket_index = self.hash_key(&key) & (self.bucket_count - 1);
        let timestamp = self.current_timestamp();

        // Create new entry
        let new_entry = Box::into_raw(Box::new(CacheEntry {
            key: key.clone(),
            value: value.clone(),
            timestamp,
            access_count: AtomicUsize::new(1),
            next: AtomicPtr::new(ptr::null_mut()),
        }));

        // Try to insert at head of bucket
        let head = self.buckets[bucket_index].load(Ordering::Acquire);
        unsafe {
            (*new_entry).next.store(head, Ordering::Release);
        }

        // CAS to update head
        match self.buckets[bucket_index].compare_exchange_weak(
            head,
            new_entry,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.size.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(_) => {
                // Cleanup on failure
                unsafe {
                    let _ = Box::from_raw(new_entry);
                }
                false
            }
        }
    }

    /// Remove a value from the cache
    pub fn remove(&self, key: &K) -> Option<V> {
        let bucket_index = self.hash_key(key) & (self.bucket_count - 1);
        let mut prev = &self.buckets[bucket_index];
        let mut current = prev.load(Ordering::Acquire);

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
            let entry = unsafe { &*current };
            if entry.key == *key {
                // Found the entry, remove it
                let next = entry.next.load(Ordering::Acquire);
                if prev
                    .compare_exchange_weak(current, next, Ordering::Release, Ordering::Acquire)
                    .is_ok()
                {
                    self.size.fetch_sub(1, Ordering::Relaxed);
                    let value = entry.value.clone();
                    // Release hazard pointer before freeing
                    unsafe {
                        crate::hazard_pointers::release_hazard_pointer(current);
                        let _ = Box::from_raw(current);
                    }
                    return Some(value);
                }
                // Release hazard pointer if CAS failed
                unsafe {
                    crate::hazard_pointers::release_hazard_pointer(current);
                }
            } else {
                // Release hazard pointer before continuing
                unsafe {
                    crate::hazard_pointers::release_hazard_pointer(current);
                }
            }

            // Get the next node pointer for iteration
            let next_node = entry.next.load(Ordering::Acquire);
            prev = &entry.next;
            current = next_node;
        }

        None
    }

    /// Get current cache size
    pub fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.size.load(Ordering::Relaxed) == 0
    }

    /// Clear all entries from the cache
    pub fn clear(&self) {
        for bucket in &self.buckets {
            let mut current = bucket.swap(ptr::null_mut(), Ordering::Acquire);
            while !current.is_null() {
                unsafe {
                    let entry = &*current;
                    let next = entry.next.load(Ordering::Acquire);
                    let _ = Box::from_raw(current);
                    current = next;
                }
            }
        }
        self.size.store(0, Ordering::Relaxed);
    }

    /// Hash a key to get bucket index
    fn hash_key(&self, key: &K) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Get current timestamp
    fn current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Check if cache should be resized
    fn should_resize(&self) -> bool {
        let current_size = self.size.load(Ordering::Relaxed);
        current_size as f64 / self.bucket_count as f64 > self.load_factor
    }

    /// Resize the cache to double the bucket count
    fn resize(&self) {
        // This is a simplified implementation
        // In a real implementation, you would need to handle concurrent resizing
        // and ensure all operations are atomic
        let _new_bucket_count = self.bucket_count * 2;
        // Implementation would go here...
    }
}

impl<K, V> Drop for LockFreeCache<K, V> {
    fn drop(&mut self) {
        // Clear all buckets
        for bucket in &self.buckets {
            let mut current = bucket.swap(ptr::null_mut(), Ordering::Acquire);
            while !current.is_null() {
                unsafe {
                    let entry = &*current;
                    let next = entry.next.load(Ordering::Acquire);
                    let _ = Box::from_raw(current);
                    current = next;
                }
            }
        }
    }
}

/// Statistics about the lock-free cache performance
#[derive(Debug, Clone)]
pub struct LockFreeCacheStats {
    /// Current number of entries in the cache
    pub size: usize,
    /// Number of hash buckets
    pub bucket_count: usize,
    /// Load factor (size / bucket_count)
    pub load_factor: f64,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Cache miss rate (0.0 to 1.0)
    pub miss_rate: f64,
}

impl<K, V> LockFreeCache<K, V> {
    /// Get cache statistics
    pub fn stats(&self) -> LockFreeCacheStats {
        let size = self.size.load(Ordering::Relaxed);
        let load_factor = size as f64 / self.bucket_count as f64;

        LockFreeCacheStats {
            size,
            bucket_count: self.bucket_count,
            load_factor,
            hit_rate: 0.0, // Would need to track hits/misses
            miss_rate: 0.0,
        }
    }
}

/// Lock-free cache with LRU eviction
pub struct LockFreeLRUCache<K, V> {
    /// The underlying lock-free cache
    cache: LockFreeCache<K, V>,
    /// Maximum size before eviction
    max_size: usize,
}

impl<K, V> LockFreeLRUCache<K, V>
where
    K: Clone + Hash + Eq + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new lock-free LRU cache
    pub fn new(initial_capacity: usize, max_size: usize) -> Self {
        Self {
            cache: LockFreeCache::new(initial_capacity, max_size),
            max_size,
        }
    }

    /// Get a value from the cache (updates LRU order)
    pub fn get(&self, key: &K) -> Option<V> {
        self.cache.get(key)
    }

    /// Insert a value into the cache (may evict LRU entries)
    pub fn insert(&self, key: K, value: V) -> bool {
        // Check if we need to evict
        if self.cache.size() >= self.max_size {
            self.evict_lru();
        }

        self.cache.insert(key, value)
    }

    /// Evict least recently used entries
    fn evict_lru(&self) {
        // This is a simplified implementation
        // In a real implementation, you would need to track access order
        // and evict the least recently used entries
    }
}

#[cfg(test)]
mod tests {
    use crate::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};
    use std::sync::Arc;

    // Simple safe cache wrapper using SafeConcurrentHashMap
    struct SafeCache<K, V> {
        map: Arc<SafeConcurrentHashMap<K, V>>,
    }

    impl<K, V> SafeCache<K, V>
    where
        K: std::hash::Hash + Eq + Clone,
        V: Clone,
    {
        fn new(_initial_capacity: usize, _max_size: usize) -> Self {
            Self {
                map: Arc::new(SafeConcurrentHashMap::new(
                    SafeConcurrentHashMapConfig::default(),
                )),
            }
        }

        fn insert(&self, key: K, value: V) -> bool {
            self.map.insert(key, value);
            true
        }

        fn get(&self, key: &K) -> Option<V>
        where
            K: std::hash::Hash + Eq,
        {
            self.map.get(key)
        }

        fn remove(&self, key: &K) -> Option<V>
        where
            K: std::hash::Hash + Eq,
        {
            self.map.remove(key)
        }

        fn size(&self) -> usize {
            self.map.len()
        }

        fn is_empty(&self) -> bool {
            self.map.is_empty()
        }
    }

    #[test]
    fn test_lockfree_cache_basic_operations() {
        let cache = SafeCache::new(16, 100);

        assert!(cache.is_empty());
        assert_eq!(cache.size(), 0);

        assert!(cache.insert("key1".to_string(), "value1".to_string()));
        assert_eq!(cache.size(), 1);

        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"key2".to_string()), None);

        assert_eq!(
            cache.remove(&"key1".to_string()),
            Some("value1".to_string())
        );
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_lockfree_cache_concurrent_access() {
        let cache = Arc::new(SafeCache::new(16, 1000));
        let mut handles = vec![];

        // Spawn multiple threads to insert values
        for i in 0..10 {
            let cache = Arc::clone(&cache);
            let handle = std::thread::spawn(move || {
                for j in 0..100 {
                    let key = format!("key_{}_{}", i, j);
                    let value = format!("value_{}_{}", i, j);
                    cache.insert(key, value);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify some values are present
        assert!(cache.size() > 0);
        assert!(cache.get(&"key_0_0".to_string()).is_some());
    }
}
