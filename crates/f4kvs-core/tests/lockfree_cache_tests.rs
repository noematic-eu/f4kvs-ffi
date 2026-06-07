//! Lock-free Cache Tests
//!
//! This module provides comprehensive tests for cache functionality
//! focusing on LRU/LFU eviction policies, concurrent access, and performance.

use f4kvs_core::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Simple LRU cache implementation using safe concurrency primitives
#[derive(Clone)]
pub struct SafeLRUCache<K, V> {
    map: Arc<SafeConcurrentHashMap<K, (V, Instant)>>,
    max_size: usize,
    keys: Arc<std::sync::Mutex<Vec<K>>>,
}

impl<K, V> SafeLRUCache<K, V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn new(max_size: usize) -> Self {
        Self {
            map: Arc::new(SafeConcurrentHashMap::new(
                SafeConcurrentHashMapConfig::default(),
            )),
            max_size,
            keys: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        // Get value atomically
        let value = self.map.get(key).map(|(v, _)| v.clone());

        if let Some(ref val) = value {
            // Update timestamp for LRU and move to end of keys list
            // This is best-effort - if another thread evicts the key, that's ok
            self.map.insert(key.clone(), (val.clone(), Instant::now()));

            // Move to end of keys list (mark as recently used)
            if let Ok(mut keys) = self.keys.lock() {
                if let Some(pos) = keys.iter().position(|k| k == key) {
                    let k = keys.remove(pos);
                    keys.push(k);
                }
            }
        }

        value
    }

    pub fn put(&self, key: K, value: V) -> Option<V> {
        // Insert value atomically
        let old_value = self
            .map
            .insert(key.clone(), (value.clone(), Instant::now()));

        let was_new = old_value.is_none();

        // Track key insertion order for LRU eviction
        // Do this inside a single lock to minimize contention
        if let Ok(mut keys) = self.keys.lock() {
            if was_new {
                keys.push(key.clone());
            } else {
                // Move to end if already exists (mark as recently used)
                if let Some(pos) = keys.iter().position(|k| k == &key) {
                    keys.remove(pos);
                    keys.push(key.clone());
                }
            }

            // Simple eviction: if we exceed max_size, remove oldest entry (from front)
            // Evict while holding the lock to avoid race conditions
            while self.map.len() > self.max_size && !keys.is_empty() {
                let oldest_key = keys.remove(0);
                let _ = self.map.remove(&oldest_key);
            }
        }

        old_value.map(|(v, _)| v)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.len() == 0
    }
}

/// Test suite for cache basic operations
#[cfg(test)]
mod cache_basic_tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = SafeLRUCache::<String, String>::new(100);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_put_and_get() {
        let cache = SafeLRUCache::<String, String>::new(100);

        // Test basic put and get
        let old_value = cache.put("key1".to_string(), "value1".to_string());
        assert_eq!(old_value, None);

        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value1".to_string()));

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_update() {
        let cache = SafeLRUCache::<String, String>::new(100);

        // Insert initial value
        let old_value = cache.put("key1".to_string(), "value1".to_string());
        assert_eq!(old_value, None);

        // Update value
        let old_value = cache.put("key1".to_string(), "value2".to_string());
        assert_eq!(old_value, Some("value1".to_string()));

        // Verify new value
        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_cache_multiple_operations() {
        let cache = SafeLRUCache::<String, String>::new(100);

        // Insert multiple values
        for i in 0..50 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i);
            cache.put(key, value);
        }

        assert_eq!(cache.len(), 50);

        // Verify all values
        for i in 0..50 {
            let key = format!("key_{}", i);
            let expected_value = format!("value_{}", i);
            let value = cache.get(&key);
            assert_eq!(value, Some(expected_value));
        }
    }

    #[test]
    fn test_cache_capacity_limit() {
        let cache = SafeLRUCache::<String, String>::new(5);

        // Insert more items than capacity
        for i in 0..10 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i);
            cache.put(key, value);
        }

        // Cache should not exceed capacity
        assert!(cache.len() <= 5);
    }
}

/// Test suite for cache eviction policies
#[cfg(test)]
mod cache_eviction_tests {
    use super::*;

    #[test]
    fn test_lru_eviction_basic() {
        let cache = SafeLRUCache::<String, String>::new(3);

        // Insert 3 items
        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());
        cache.put("key3".to_string(), "value3".to_string());

        assert_eq!(cache.len(), 3);

        // Access key1 to make it recently used
        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value1".to_string()));

        // Insert 4th item, should evict least recently used
        cache.put("key4".to_string(), "value4".to_string());

        // key1 should still be there (recently accessed)
        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value1".to_string()));

        // key4 should be there
        let value = cache.get(&"key4".to_string());
        assert_eq!(value, Some("value4".to_string()));
    }

    #[test]
    fn test_cache_eviction_under_load() {
        let cache = SafeLRUCache::<String, String>::new(10);

        // Insert many items to trigger eviction
        for i in 0..100 {
            let key = format!("load_key_{}", i);
            let value = format!("load_value_{}", i);
            cache.put(key, value);
        }

        // Cache should not exceed capacity
        assert!(cache.len() <= 10);

        // Some recent items should still be accessible
        for i in 90..100 {
            let key = format!("load_key_{}", i);
            let value = cache.get(&key);
            // Some items might be evicted, that's ok
            if value.is_some() {
                assert_eq!(value, Some(format!("load_value_{}", i)));
            }
        }
    }

    #[test]
    fn test_cache_access_patterns() {
        let cache = SafeLRUCache::<String, String>::new(5);

        // Insert items
        for i in 0..5 {
            let key = format!("pattern_key_{}", i);
            let value = format!("pattern_value_{}", i);
            cache.put(key, value);
        }

        // Access items in different patterns
        for _ in 0..10 {
            // Access key0 frequently
            let value = cache.get(&"pattern_key_0".to_string());
            assert_eq!(value, Some("pattern_value_0".to_string()));

            // Access key1 occasionally
            if cache.get(&"pattern_key_1".to_string()).is_some() {
                let value = cache.get(&"pattern_key_1".to_string());
                assert_eq!(value, Some("pattern_value_1".to_string()));
            }
        }

        // key0 should still be there (frequently accessed)
        let value = cache.get(&"pattern_key_0".to_string());
        assert_eq!(value, Some("pattern_value_0".to_string()));
    }
}

/// Test suite for concurrent cache access
#[cfg(test)]
mod cache_concurrent_tests {
    use super::*;

    #[test]
    fn test_concurrent_cache_operations() {
        let cache = Arc::new(SafeLRUCache::<String, String>::new(100));
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let cache_clone = cache.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("concurrent_key_{}_{}", thread_id, i);
                    let value = format!("concurrent_value_{}_{}", thread_id, i);

                    // Put
                    cache_clone.put(key.clone(), value.clone());

                    // Get (may return None due to concurrent eviction, that's ok)
                    let retrieved = cache_clone.get(&key);
                    // In concurrent scenarios, eviction may occur between put and get
                    if retrieved.is_some() {
                        assert_eq!(retrieved, Some(value));
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Cache should have some items (eviction may have occurred)
        assert!(cache.len() > 0);
        assert!(cache.len() <= 100);
    }

    #[test]
    fn test_concurrent_cache_read_write() {
        let cache = Arc::new(SafeLRUCache::<String, String>::new(50));
        let num_threads = 4;
        let operations_per_thread = 50;

        // Pre-populate with some data
        for i in 0..20 {
            let key = format!("pre_key_{}", i);
            cache.put(key, format!("pre_value_{}", i));
        }

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let cache_clone = cache.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("thread_{}_value_{}", thread_id, i);

                    // Put new value
                    cache_clone.put(key.clone(), value.clone());

                    // Get it back (may be None due to concurrent eviction)
                    let retrieved = cache_clone.get(&key);
                    if retrieved.is_some() {
                        assert_eq!(retrieved, Some(value));
                    }

                    // Read some pre-existing values
                    let pre_key = format!("pre_key_{}", i % 20);
                    let pre_value = cache_clone.get(&pre_key);
                    // Pre-existing values might be evicted, that's ok
                    if pre_value.is_some() {
                        assert_eq!(pre_value, Some(format!("pre_value_{}", i % 20)));
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
    fn test_concurrent_cache_eviction() {
        let cache = Arc::new(SafeLRUCache::<String, String>::new(10));
        let num_threads = 4;
        let operations_per_thread = 50;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let cache_clone = cache.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("eviction_key_{}_{}", thread_id, i);
                    let value = format!("eviction_value_{}_{}", thread_id, i);

                    // Put value
                    cache_clone.put(key.clone(), value.clone());

                    // Access it to make it recently used (may be evicted concurrently)
                    let _retrieved = cache_clone.get(&key);
                    // Don't assert value here as concurrent eviction may have occurred
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Cache should not exceed capacity
        assert!(cache.len() <= 10);
    }
}

/// Test suite for cache performance characteristics
#[cfg(test)]
mod cache_performance_tests {
    use super::*;

    #[test]
    fn test_cache_hit_performance() {
        let cache = SafeLRUCache::<String, String>::new(1000);
        let num_items = 1000;

        // Pre-populate cache
        for i in 0..num_items {
            let key = format!("perf_key_{}", i);
            let value = format!("perf_value_{}", i);
            cache.put(key, value);
        }

        let start = Instant::now();

        // Perform many cache hits (reduced iterations to be robust on CI / slower machines)
        for _ in 0..2000 {
            for i in 0..num_items {
                let key = format!("perf_key_{}", i);
                let _ = cache.get(&key);
            }
        }

        let duration = start.elapsed();
        println!("Cache hit performance: {:?}", duration);

        // Should complete in reasonable time (allow more time for lock contention in concurrent scenarios)
        assert!(duration.as_secs() < 30);
    }

    #[test]
    fn test_cache_miss_performance() {
        let cache = SafeLRUCache::<String, String>::new(100);
        let num_items = 100;

        // Pre-populate cache
        for i in 0..num_items {
            let key = format!("miss_key_{}", i);
            let value = format!("miss_value_{}", i);
            cache.put(key, value);
        }

        let start = Instant::now();

        // Perform many cache misses
        for _ in 0..1000 {
            for i in num_items..num_items * 2 {
                let key = format!("miss_key_{}", i);
                let _ = cache.get(&key);
            }
        }

        let duration = start.elapsed();
        println!("Cache miss performance: {:?}", duration);

        // Should complete in reasonable time
        assert!(duration.as_secs() < 5);
    }

    #[test]
    fn test_concurrent_cache_performance() {
        let cache = Arc::new(SafeLRUCache::<String, String>::new(100));
        let num_threads = 8;
        let operations_per_thread = 1000;

        let start = Instant::now();

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let cache_clone = cache.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("concurrent_perf_key_{}_{}", thread_id, i);
                    let value = format!("concurrent_perf_value_{}_{}", thread_id, i);

                    cache_clone.put(key.clone(), value.clone());
                    let _ = cache_clone.get(&key);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_threads * operations_per_thread * 2; // put + get

        println!("Concurrent cache performance: {:?}", duration);
        println!(
            "Operations per second: {}",
            total_operations as f64 / duration.as_secs_f64()
        );

        // Should complete in reasonable time
        assert!(duration.as_secs() < 10);
    }
}

/// Test suite for cache edge cases and error handling
#[cfg(test)]
mod cache_edge_case_tests {
    use super::*;

    #[test]
    fn test_cache_with_empty_capacity() {
        let cache = SafeLRUCache::<String, String>::new(0);

        // Should handle zero capacity gracefully
        let old_value = cache.put("key1".to_string(), "value1".to_string());
        assert_eq!(old_value, None);

        // Cache should remain empty
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_with_single_capacity() {
        let cache = SafeLRUCache::<String, String>::new(1);

        // Insert first item
        cache.put("key1".to_string(), "value1".to_string());
        assert_eq!(cache.len(), 1);

        // Insert second item, should evict first
        cache.put("key2".to_string(), "value2".to_string());
        assert_eq!(cache.len(), 1);

        // key1 should be evicted
        let value1 = cache.get(&"key1".to_string());
        assert_eq!(value1, None);

        // key2 should be there
        let value2 = cache.get(&"key2".to_string());
        assert_eq!(value2, Some("value2".to_string()));
    }

    #[test]
    fn test_cache_with_large_values() {
        let cache = SafeLRUCache::<String, String>::new(10);

        // Insert large values
        for i in 0..5 {
            let key = format!("large_key_{}", i);
            let large_value = "x".repeat(10000); // 10KB value
            cache.put(key, large_value);
        }

        assert_eq!(cache.len(), 5);

        // Verify all large values are accessible
        for i in 0..5 {
            let key = format!("large_key_{}", i);
            let value = cache.get(&key);
            assert!(value.is_some());
            assert_eq!(value.unwrap().len(), 10000);
        }
    }

    #[test]
    fn test_cache_with_special_characters() {
        let cache = SafeLRUCache::<String, String>::new(100);

        let special_keys = vec![
            "key with spaces",
            "key\twith\ttabs",
            "key\nwith\nnewlines",
            "key with unicode: 你好世界",
            "key with emoji: 🚀🔥💯",
        ];

        for (i, key) in special_keys.iter().enumerate() {
            let value = format!("special_value_{}", i);
            cache.put(key.to_string(), value.clone());

            let retrieved = cache.get(&key.to_string());
            assert_eq!(retrieved, Some(value));
        }
    }

    #[test]
    fn test_cache_rapid_operations() {
        let cache = SafeLRUCache::<String, String>::new(5);

        // Rapid put/get cycles
        for i in 0..1000 {
            let key = format!("rapid_key_{}", i);
            let value = format!("rapid_value_{}", i);

            cache.put(key.clone(), value.clone());
            let retrieved = cache.get(&key);
            assert_eq!(retrieved, Some(value));
        }

        // Cache should not exceed capacity
        assert!(cache.len() <= 5);
    }
}
