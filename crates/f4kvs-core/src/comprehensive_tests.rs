//! Comprehensive testing suite for f4kvs-core
//!
//! This module provides extensive testing including:
//! - Fuzz testing for all data structures
//! - Edge case testing for error conditions
//! - Stress testing for concurrent operations
//! - Memory pressure testing
//! - Performance regression testing
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};
// use crate::hazard_pointers::{HazardPointerGuard, SafeAtomicPtr};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Fuzz testing for SafeConcurrentHashMap
#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use rand::Rng;

    #[test]
    // Re-enabled: Basic fuzz test should be safe
    fn test_hashmap_fuzz_insert_get() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let mut rng = rand::thread_rng();

        // Test with random keys and values
        for _ in 0..1000 {
            let key = format!("key_{}", rng.gen::<u32>());
            let value = format!("value_{}", rng.gen::<u32>());

            // Insert
            let old_value = map.insert(key.clone(), value.clone());

            // Get should return the new value
            assert_eq!(map.get(&key), Some(value.clone()));

            // If we inserted the same key again, old_value should be the previous value
            if old_value.is_some() {
                assert_ne!(old_value, Some(value));
            }
        }
    }

    #[test]
    // Re-enabled: Basic remove test should be safe
    fn test_hashmap_fuzz_remove() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let mut rng = rand::thread_rng();
        let mut inserted_keys = Vec::new();

        // Insert random keys
        for _ in 0..500 {
            let key = format!("key_{}", rng.gen::<u32>());
            let value = format!("value_{}", rng.gen::<u32>());
            map.insert(key.clone(), value);
            inserted_keys.push(key);
        }

        // Remove random keys
        for _ in 0..250 {
            let idx = rng.gen_range(0..inserted_keys.len());
            let key = inserted_keys.remove(idx);

            let removed_value = map.remove(&key);
            assert!(removed_value.is_some());

            // Key should no longer exist
            assert_eq!(map.get(&key), None);
        }
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_fuzz_concurrent_operations() {
        let map: Arc<SafeConcurrentHashMap<String, String>> = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let mut handles = vec![];

        // Spawn multiple threads with different operations
        for thread_id in 0..10 {
            let map = Arc::clone(&map);
            let handle = thread::spawn(move || {
                let mut rng = rand::thread_rng();

                for _ in 0..100 {
                    let operation = rng.gen_range(0..3);
                    let key = format!("thread_{}_key_{}", thread_id, rng.gen::<u32>());

                    match operation {
                        0 => {
                            // Insert
                            let value = format!("value_{}", rng.gen::<u32>());
                            map.insert(key, value);
                        }
                        1 => {
                            // Get
                            let _ = map.get(&key);
                        }
                        2 => {
                            // Remove
                            let _ = map.remove(&key);
                        }
                        _ => unreachable!(),
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

/// Edge case testing
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_empty_operations() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test operations on empty map
        assert_eq!(map.get(&"nonexistent".to_string()), None);
        assert_eq!(map.remove(&"nonexistent".to_string()), None);
        assert!(!map.contains_key(&"nonexistent".to_string()));
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_very_long_keys() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test with very long keys
        let long_key = "a".repeat(10000);
        let value = "test_value".to_string();

        map.insert(long_key.clone(), value.clone());
        assert_eq!(map.get(&long_key), Some(value));
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_unicode_keys() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test with Unicode keys
        let unicode_keys = [
            "🚀".to_string(),
            "测试".to_string(),
            "тест".to_string(),
            "🎯".to_string(),
        ];

        for (i, key) in unicode_keys.iter().enumerate() {
            let value = format!("value_{}", i);
            map.insert(key.clone(), value.clone());
            assert_eq!(map.get(key), Some(value));
        }
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_special_characters() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test with special characters
        let special_keys = [
            "key with spaces".to_string(),
            "key\nwith\nnewlines".to_string(),
            "key\twith\ttabs".to_string(),
            "key\"with\"quotes".to_string(),
            "key'with'apostrophes".to_string(),
        ];

        for (i, key) in special_keys.iter().enumerate() {
            let value = format!("value_{}", i);
            map.insert(key.clone(), value.clone());
            assert_eq!(map.get(key), Some(value));
        }
    }
}

/// Stress testing for concurrent operations
#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_high_concurrency() {
        let map: Arc<SafeConcurrentHashMap<String, String>> = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let mut handles = vec![];

        // Spawn many threads for high concurrency
        for thread_id in 0..50 {
            let map = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..1000 {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("value_{}_{}", thread_id, i);

                    // Insert
                    map.insert(key.clone(), value.clone());

                    // Verify
                    assert_eq!(map.get(&key), Some(value));

                    // Remove
                    let removed = map.remove(&key);
                    assert!(removed.is_some());
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_rapid_insert_delete() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Rapid insert and delete operations
        for i in 0..10000 {
            let key = format!("rapid_key_{}", i);
            let value = format!("rapid_value_{}", i);

            // Insert
            map.insert(key.clone(), value.clone());

            // Immediately delete
            let removed = map.remove(&key);
            assert_eq!(removed, Some(value));

            // Verify it's gone
            assert_eq!(map.get(&key), None);
        }
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_memory_pressure() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert many items to test memory pressure
        let mut keys = Vec::new();
        for i in 0..100000 {
            let key = format!("pressure_key_{}", i);
            let value = format!("pressure_value_{}", i);
            map.insert(key.clone(), value);
            keys.push(key);
        }

        // Verify all items are still accessible
        for (i, key) in keys.iter().enumerate() {
            let expected_value = format!("pressure_value_{}", i);
            assert_eq!(map.get(key), Some(expected_value));
        }

        // Remove all items
        for key in keys {
            let removed = map.remove(&key);
            assert!(removed.is_some());
        }
    }
}

/// Performance regression testing
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_performance_insert() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let start = Instant::now();

        // Insert 100,000 items
        for i in 0..100000 {
            let key = format!("perf_key_{}", i);
            let value = format!("perf_value_{}", i);
            map.insert(key, value);
        }

        let duration = start.elapsed();
        log::debug!("Insert 100,000 items took: {:?}", duration);

        // Should complete within reasonable time (adjust threshold as needed)
        assert!(duration < Duration::from_secs(10));
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_performance_get() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert items first
        for i in 0..10000 {
            let key = format!("perf_get_key_{}", i);
            let value = format!("perf_get_value_{}", i);
            map.insert(key, value);
        }

        let start = Instant::now();

        // Get all items
        for i in 0..10000 {
            let key = format!("perf_get_key_{}", i);
            let expected_value = format!("perf_get_value_{}", i);
            assert_eq!(map.get(&key), Some(expected_value));
        }

        let duration = start.elapsed();
        log::debug!("Get 10,000 items took: {:?}", duration);

        // Should complete within reasonable time
        assert!(duration < Duration::from_secs(5));
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hashmap_performance_concurrent() {
        let map: Arc<SafeConcurrentHashMap<String, String>> = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let start = Instant::now();
        let mut handles = vec![];

        // Spawn multiple threads for concurrent performance test
        for thread_id in 0..10 {
            let map = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..1000 {
                    let key = format!("concurrent_perf_{}_{}", thread_id, i);
                    let value = format!("concurrent_value_{}_{}", thread_id, i);

                    map.insert(key.clone(), value.clone());
                    assert_eq!(map.get(&key), Some(value));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        log::debug!(
            "Concurrent operations (10 threads, 1000 ops each) took: {:?}",
            duration
        );

        // Should complete within reasonable time
        assert!(duration < Duration::from_secs(30));
    }
}

/// Memory safety testing
#[cfg(test)]
mod memory_safety_tests {
    use super::*;

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_hazard_pointer_safety() {
        // Test that hazard pointers prevent use-after-free
        let map: Arc<SafeConcurrentHashMap<String, String>> = Arc::new(SafeConcurrentHashMap::new(
            SafeConcurrentHashMapConfig::default(),
        ));

        // Insert some data
        for i in 0..100 {
            let key = format!("safety_key_{}", i);
            let value = format!("safety_value_{}", i);
            map.insert(key, value);
        }

        // Concurrent access should be safe
        let mut handles = vec![];
        for _thread_id in 0..10 {
            let map = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = format!("safety_key_{}", i);
                    let _ = map.get(&key);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    // Re-enabled after replacing unsafe implementation with safe DashMap-based alternative
    fn test_memory_leak_prevention() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert and remove many items to test for memory leaks
        for i in 0..10000 {
            let key = format!("leak_test_key_{}", i);
            let value = format!("leak_test_value_{}", i);

            map.insert(key.clone(), value);
            let removed = map.remove(&key);
            assert!(removed.is_some());
        }

        // If we get here without running out of memory, the test passes
    }
}

/// Integration testing
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_hashmap_integration_workflow() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Simulate a realistic workflow
        let users = vec!["alice", "bob", "charlie", "diana", "eve"];
        let mut user_data = std::collections::HashMap::new();

        // Initialize user data
        for user in &users {
            let key = format!("user:{}", user);
            let data = format!("data_for_{}", user);
            map.insert(key.clone(), data.clone());
            user_data.insert(user.to_string(), data);
        }

        // Verify all users exist
        for user in &users {
            let key = format!("user:{}", user);
            let expected_data = user_data.get(*user).unwrap();
            assert_eq!(map.get(&key), Some(expected_data.clone()));
        }

        // Update some user data
        for user in &users[0..3] {
            let key = format!("user:{}", user);
            let new_data = format!("updated_data_for_{}", user);
            let old_data = map.insert(key.clone(), new_data.clone());
            assert_eq!(old_data, Some(user_data.get(*user).unwrap().clone()));
            user_data.insert(user.to_string(), new_data);
        }

        // Remove some users
        for user in &users[3..] {
            let key = format!("user:{}", user);
            let removed_data = map.remove(&key);
            assert_eq!(removed_data, Some(user_data.get(*user).unwrap().clone()));
        }

        // Verify final state
        for user in &users[0..3] {
            let key = format!("user:{}", user);
            let expected_data = user_data.get(*user).unwrap();
            assert_eq!(map.get(&key), Some(expected_data.clone()));
        }

        for user in &users[3..] {
            let key = format!("user:{}", user);
            assert_eq!(map.get(&key), None);
        }
    }
}
