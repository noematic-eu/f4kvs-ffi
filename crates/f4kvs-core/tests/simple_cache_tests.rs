//! Simple Cache Tests
//!
//! This module provides comprehensive tests for cache-like functionality
//! using safe concurrency primitives, focusing on basic operations and concurrent access.

use f4kvs_core::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Test suite for cache-like operations using SafeConcurrentHashMap
#[cfg(test)]
mod cache_like_tests {
    use super::*;

    #[test]
    fn test_cache_like_operations() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Test basic put and get operations
        cache.insert("key1".to_string(), "value1".to_string());
        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value1".to_string()));

        // Test update
        let old_value = cache.insert("key1".to_string(), "value2".to_string());
        assert_eq!(old_value, Some("value1".to_string()));

        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("value2".to_string()));

        // Test remove
        let removed = cache.remove(&"key1".to_string());
        assert_eq!(removed, Some("value2".to_string()));

        let value = cache.get(&"key1".to_string());
        assert_eq!(value, None);
    }

    #[test]
    fn test_cache_like_multiple_operations() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Insert multiple values
        for i in 0..100 {
            let key = format!("cache_key_{}", i);
            let value = format!("cache_value_{}", i);
            cache.insert(key, value);
        }

        assert_eq!(cache.len(), 100);

        // Verify all values
        for i in 0..100 {
            let key = format!("cache_key_{}", i);
            let expected_value = format!("cache_value_{}", i);
            let value = cache.get(&key);
            assert_eq!(value, Some(expected_value));
        }
    }

    #[test]
    fn test_cache_like_concurrent_operations() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let cache_clone = cache.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("concurrent_cache_key_{}_{}", thread_id, i);
                    let value = format!("concurrent_cache_value_{}_{}", thread_id, i);

                    // Insert
                    cache_clone.insert(key.clone(), value.clone());

                    // Read
                    let retrieved = cache_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Update
                    let new_value = format!("updated_concurrent_cache_value_{}_{}", thread_id, i);
                    cache_clone.insert(key.clone(), new_value.clone());

                    // Verify update
                    let final_retrieved = cache_clone.get(&key);
                    assert_eq!(final_retrieved, Some(new_value));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all values were inserted
        assert_eq!(cache.len(), num_threads * operations_per_thread);
    }

    #[test]
    fn test_cache_like_performance() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_items = 1000;

        // Pre-populate cache
        for i in 0..num_items {
            let key = format!("perf_key_{}", i);
            let value = format!("perf_value_{}", i);
            cache.insert(key, value);
        }

        let start = Instant::now();

        // Perform many operations
        for _ in 0..1000 {
            for i in 0..num_items {
                let key = format!("perf_key_{}", i);
                let _ = cache.get(&key);
            }
        }

        let duration = start.elapsed();
        println!("Cache-like performance: {:?}", duration);

        // Should complete in reasonable time
        assert!(duration.as_secs() < 10);
    }

    #[test]
    fn test_cache_like_edge_cases() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Test empty string key
        cache.insert("".to_string(), "empty_key_value".to_string());
        let value = cache.get(&"".to_string());
        assert_eq!(value, Some("empty_key_value".to_string()));

        // Test very long key
        let long_key = "a".repeat(1000);
        cache.insert(long_key.clone(), "long_key_value".to_string());
        let value = cache.get(&long_key);
        assert_eq!(value, Some("long_key_value".to_string()));

        // Test special characters
        let special_key = "!@#$%^&*()_+-=[]{}|;':\",./<>?".to_string();
        cache.insert(special_key.clone(), "special_value".to_string());
        let value = cache.get(&special_key);
        assert_eq!(value, Some("special_value".to_string()));
    }

    #[test]
    fn test_cache_like_large_values() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Test with large values
        for i in 0..10 {
            let key = format!("large_key_{}", i);
            let large_value = "x".repeat(10000); // 10KB value
            cache.insert(key.clone(), large_value.clone());

            let retrieved = cache.get(&key);
            assert_eq!(retrieved, Some(large_value));
        }

        assert_eq!(cache.len(), 10);
    }

    #[test]
    fn test_cache_like_rapid_operations() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Rapid insert/delete cycles
        for i in 0..1000 {
            let key = format!("rapid_key_{}", i);
            let value = format!("rapid_value_{}", i);

            // Insert
            cache.insert(key.clone(), value.clone());

            // Verify
            let retrieved = cache.get(&key);
            assert_eq!(retrieved, Some(value));

            // Remove
            let removed = cache.remove(&key);
            assert_eq!(removed, Some(format!("rapid_value_{}", i)));

            // Verify removal
            let after_delete = cache.get(&key);
            assert_eq!(after_delete, None);
        }

        // Cache should be empty
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_like_memory_pressure() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Insert many items to test memory pressure
        for i in 0..10000 {
            let key = format!("pressure_key_{}", i);
            let value = format!("pressure_value_{}", i);
            cache.insert(key, value);
        }

        // Verify all items are still accessible
        for i in 0..10000 {
            let key = format!("pressure_key_{}", i);
            let expected_value = format!("pressure_value_{}", i);
            let value = cache.get(&key);
            assert_eq!(value, Some(expected_value));
        }

        // Remove half the items
        for i in 0..5000 {
            let key = format!("pressure_key_{}", i);
            let removed = cache.remove(&key);
            assert_eq!(removed, Some(format!("pressure_value_{}", i)));
        }

        // Verify remaining items
        for i in 5000..10000 {
            let key = format!("pressure_key_{}", i);
            let expected_value = format!("pressure_value_{}", i);
            let value = cache.get(&key);
            assert_eq!(value, Some(expected_value));
        }

        assert_eq!(cache.len(), 5000);
    }

    #[test]
    fn test_cache_like_concurrent_performance() {
        let cache = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
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

                    cache_clone.insert(key.clone(), value.clone());
                    let _ = cache_clone.get(&key);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_threads * operations_per_thread * 2; // insert + get

        println!("Concurrent cache-like performance: {:?}", duration);
        println!(
            "Operations per second: {}",
            total_operations as f64 / duration.as_secs_f64()
        );

        // Should complete in reasonable time
        assert!(duration.as_secs() < 10);
        assert_eq!(cache.len(), total_operations / 2);
    }
}
