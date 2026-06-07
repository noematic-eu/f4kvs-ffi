//! Lock-free HashMap specific tests
//!
//! This module provides comprehensive tests for the lock-free HashMap implementation
//! focusing on hash collision handling, concurrent access, and memory safety.

use f4kvs_core::safe_concurrency_wrappers::{SafeConcurrentHashMap, SafeConcurrentHashMapConfig};
use std::sync::Arc;
use std::thread;

/// Test suite for LockFreeHashMap hash collision handling
#[cfg(test)]
mod hash_collision_tests {
    use super::*;

    #[test]
    fn test_hash_collision_handling() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert keys that might hash to same bucket
        let keys = vec![
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
            "r", "s", "t",
        ];

        for (i, key) in keys.iter().enumerate() {
            let value = format!("value_{}", i);
            map.insert(key.to_string(), value);
        }

        // Verify all values are retrievable
        for (i, key) in keys.iter().enumerate() {
            let expected_value = format!("value_{}", i);
            let value = map.get(&key.to_string());
            assert_eq!(value, Some(expected_value));
        }
    }

    #[test]
    fn test_hash_collision_with_updates() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert keys that might collide
        let keys = vec!["key1", "key2", "key3", "key4", "key5"];

        // Initial insertions
        for (i, key) in keys.iter().enumerate() {
            let value = format!("initial_{}", i);
            map.insert(key.to_string(), value);
        }

        // Update all values
        for (i, key) in keys.iter().enumerate() {
            let value = format!("updated_{}", i);
            let old_value = map.insert(key.to_string(), value.clone());
            assert_eq!(old_value, Some(format!("initial_{}", i)));
        }

        // Verify all updates
        for (i, key) in keys.iter().enumerate() {
            let expected_value = format!("updated_{}", i);
            let value = map.get(&key.to_string());
            assert_eq!(value, Some(expected_value));
        }
    }

    #[test]
    fn test_hash_collision_with_removals() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert keys
        let keys = vec!["key1", "key2", "key3", "key4", "key5"];
        for (i, key) in keys.iter().enumerate() {
            let value = format!("value_{}", i);
            map.insert(key.to_string(), value);
        }

        // Remove some keys
        let removed_keys = vec!["key2", "key4"];
        for key in &removed_keys {
            let removed = map.remove(&key.to_string());
            assert!(removed.is_some());
        }

        // Verify remaining keys
        let remaining_keys = vec!["key1", "key3", "key5"];
        for (i, key) in remaining_keys.iter().enumerate() {
            let expected_value = format!("value_{}", i * 2); // key1=0, key3=2, key5=4
            let value = map.get(&key.to_string());
            assert_eq!(value, Some(expected_value));
        }

        // Verify removed keys are gone
        for key in &removed_keys {
            let value = map.get(&key.to_string());
            assert_eq!(value, None);
        }
    }
}

/// Test suite for LockFreeHashMap concurrent access patterns
#[cfg(test)]
mod concurrent_access_tests {
    use super::*;

    #[test]
    fn test_concurrent_insert_operations() {
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
    fn test_concurrent_read_write_operations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 50;

        // Pre-populate with some data
        for i in 0..100 {
            let key = format!("pre_key_{}", i);
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
                    let existing_key = format!("pre_key_{}", i % 100);
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
    fn test_concurrent_remove_operations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 25;

        // Pre-populate with data
        for i in 0..200 {
            let key = format!("key_{}", i);
            map.insert(key, format!("value_{}", i));
        }

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("key_{}", thread_id * operations_per_thread + i);
                    let removed = map_clone.remove(&key);
                    assert!(removed.is_some());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify correct number of remaining items
        assert_eq!(map.len(), 200 - (num_threads * operations_per_thread));
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let _num_threads = 6;
        let operations_per_thread = 50;

        let mut handles = vec![];

        // Spawn insert threads
        for thread_id in 0..2 {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("insert_{}_key_{}", thread_id, i);
                    let value = format!("insert_{}_value_{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        // Spawn read threads
        for thread_id in 0..2 {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("insert_{}_key_{}", thread_id, i);
                    let value = map_clone.get(&key);
                    // Value might not exist yet, that's ok
                    if let Some(v) = value {
                        assert!(v.starts_with("insert_"));
                    }
                }
            });
            handles.push(handle);
        }

        // Spawn remove threads
        for thread_id in 0..2 {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("insert_{}_key_{}", thread_id, i);
                    let _ = map_clone.remove(&key);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

/// Test suite for LockFreeHashMap memory safety and edge cases
#[cfg(test)]
mod memory_safety_tests {
    use super::*;

    #[test]
    fn test_large_key_values() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test very large key
        let large_key = "a".repeat(10000);
        let large_value = "b".repeat(10000);
        map.insert(large_key.clone(), large_value.clone());

        let retrieved = map.get(&large_key);
        assert_eq!(retrieved, Some(large_value));

        // Test very large value
        let normal_key = "normal_key";
        let huge_value = "x".repeat(100000);
        map.insert(normal_key.to_string(), huge_value.clone());

        let retrieved = map.get(&normal_key.to_string());
        assert_eq!(retrieved, Some(huge_value));
    }

    #[test]
    fn test_special_characters_in_keys() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        let special_keys = vec![
            "!@#$%^&*()_+-=[]{}|;':\",./<>?",
            "key with spaces",
            "key\twith\ttabs",
            "key\nwith\nnewlines",
            "key\rwith\rcarriage",
            "key with unicode: 你好世界",
            "key with emoji: 🚀🔥💯",
        ];

        for (i, key) in special_keys.iter().enumerate() {
            let value = format!("special_value_{}", i);
            map.insert(key.to_string(), value.clone());

            let retrieved = map.get(&key.to_string());
            assert_eq!(retrieved, Some(value));
        }
    }

    #[test]
    fn test_empty_strings() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Test empty key
        map.insert("".to_string(), "empty_key_value".to_string());
        let retrieved = map.get(&"".to_string());
        assert_eq!(retrieved, Some("empty_key_value".to_string()));

        // Test empty value
        map.insert("empty_value_key".to_string(), "".to_string());
        let retrieved = map.get(&"empty_value_key".to_string());
        assert_eq!(retrieved, Some("".to_string()));

        // Test both empty
        map.insert("".to_string(), "".to_string());
        let retrieved = map.get(&"".to_string());
        assert_eq!(retrieved, Some("".to_string()));
    }

    #[test]
    fn test_rapid_insert_delete_cycles() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Rapid insert/delete cycles
        for i in 0..1000 {
            let key = format!("rapid_key_{}", i);
            let value = format!("rapid_value_{}", i);

            // Insert
            map.insert(key.clone(), value.clone());

            // Verify
            let retrieved = map.get(&key);
            assert_eq!(retrieved, Some(value));

            // Delete
            let removed = map.remove(&key);
            assert_eq!(removed, Some(format!("rapid_value_{}", i)));

            // Verify deletion
            let after_delete = map.get(&key);
            assert_eq!(after_delete, None);
        }

        // Map should be empty
        assert!(map.is_empty());
    }

    #[test]
    fn test_memory_pressure_scenario() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());

        // Insert many items to test memory pressure
        for i in 0..10000 {
            let key = format!("pressure_key_{}", i);
            let value = format!("pressure_value_{}", i);
            map.insert(key, value);
        }

        // Verify all items are still accessible
        for i in 0..10000 {
            let key = format!("pressure_key_{}", i);
            let expected_value = format!("pressure_value_{}", i);
            let value = map.get(&key);
            assert_eq!(value, Some(expected_value));
        }

        // Remove half the items
        for i in 0..5000 {
            let key = format!("pressure_key_{}", i);
            let removed = map.remove(&key);
            assert_eq!(removed, Some(format!("pressure_value_{}", i)));
        }

        // Verify remaining items
        for i in 5000..10000 {
            let key = format!("pressure_key_{}", i);
            let expected_value = format!("pressure_value_{}", i);
            let value = map.get(&key);
            assert_eq!(value, Some(expected_value));
        }

        assert_eq!(map.len(), 5000);
    }
}

/// Test suite for LockFreeHashMap performance characteristics
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_insert_performance() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let num_items = 10000;

        let start = Instant::now();

        for i in 0..num_items {
            let key = format!("perf_key_{}", i);
            let value = format!("perf_value_{}", i);
            map.insert(key, value);
        }

        let duration = start.elapsed();
        println!("Inserted {} items in {:?}", num_items, duration);

        // Should complete in reasonable time (less than 1 second)
        assert!(duration.as_secs() < 1);
        assert_eq!(map.len(), num_items);
    }

    #[test]
    fn test_lookup_performance() {
        let map: SafeConcurrentHashMap<String, String> =
            SafeConcurrentHashMap::new(SafeConcurrentHashMapConfig::default());
        let num_items = 10000;

        // Insert items
        for i in 0..num_items {
            let key = format!("lookup_key_{}", i);
            let value = format!("lookup_value_{}", i);
            map.insert(key, value);
        }

        let start = Instant::now();

        // Lookup all items
        for i in 0..num_items {
            let key = format!("lookup_key_{}", i);
            let value = map.get(&key);
            assert_eq!(value, Some(format!("lookup_value_{}", i)));
        }

        let duration = start.elapsed();
        println!("Looked up {} items in {:?}", num_items, duration);

        // Should complete in reasonable time (less than 1 second)
        assert!(duration.as_secs() < 1);
    }

    #[test]
    fn test_concurrent_performance() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 8;
        let operations_per_thread = 1000;

        let start = Instant::now();

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("concurrent_key_{}_{}", thread_id, i);
                    let value = format!("concurrent_value_{}_{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_threads * operations_per_thread;

        println!(
            "Completed {} concurrent operations in {:?}",
            total_operations, duration
        );
        println!(
            "Operations per second: {}",
            total_operations as f64 / duration.as_secs_f64()
        );

        // Should complete in reasonable time
        assert!(duration.as_secs() < 5);
        assert_eq!(map.len(), total_operations);
    }
}
