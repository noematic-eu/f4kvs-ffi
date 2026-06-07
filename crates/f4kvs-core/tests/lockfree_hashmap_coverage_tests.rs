//! Comprehensive coverage tests for LockFreeHashMap
//!
//! This module provides extensive tests for LockFreeHashMap to increase code coverage,
//! including hash collisions, concurrent access, resizing, and edge cases.

use f4kvs_core::lockfree::{LockFreeHashMap, LockFreeHashMapConfig};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Test suite for LockFreeHashMap basic operations
#[cfg(test)]
mod hashmap_basic_tests {
    use super::*;

    #[test]
    fn test_hashmap_creation() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_hashmap_creation_custom_config() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 32,
            load_factor: 0.8,
            max_buckets: 2048,
        };
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

        map.insert("key1".to_string(), "value1".to_string());
        assert_eq!(map.len(), 1);

        let removed = map.remove(&"key1".to_string());
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());

        // Remove non-existent key
        let removed = map.remove(&"nonexistent".to_string());
        assert_eq!(removed, None);
    }

    #[test]
    fn test_hashmap_contains_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        map.insert("key1".to_string(), "value1".to_string());

        assert!(map.contains_key(&"key1".to_string()));
        assert!(!map.contains_key(&"nonexistent".to_string()));
    }

    #[test]
    fn test_hashmap_clear() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert multiple items
        for i in 0..100 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        assert_eq!(map.len(), 100);
        map.clear();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());

        // Verify all keys are gone
        for i in 0..100 {
            assert!(!map.contains_key(&format!("key_{}", i)));
        }
    }

    #[test]
    fn test_hashmap_multiple_keys() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert many keys
        for i in 0..1000 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        assert_eq!(map.len(), 1000);

        // Verify all keys are retrievable
        for i in 0..1000 {
            let value = map.get(&format!("key_{}", i));
            assert_eq!(value, Some(format!("value_{}", i)));
        }
    }
}

/// Test suite for hash collision handling
#[cfg(test)]
mod hash_collision_tests {
    use super::*;

    #[test]
    fn test_hash_collision_handling() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 4, // Small bucket count to force collisions
            load_factor: 0.75,
            max_buckets: 1024,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys that will likely hash to same bucket
        let keys = vec![
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
            "r", "s", "t", "u", "v", "w", "x", "y", "z",
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

        assert_eq!(map.len(), keys.len());
    }

    #[test]
    fn test_hash_collision_with_updates() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 4,
            load_factor: 0.75,
            max_buckets: 1024,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys that may collide
        for i in 0..50 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        // Update colliding keys
        for i in 0..50 {
            let old_value = map.insert(format!("key_{}", i), format!("updated_{}", i));
            assert_eq!(old_value, Some(format!("value_{}", i)));
        }

        // Verify all updates are correct
        for i in 0..50 {
            let value = map.get(&format!("key_{}", i));
            assert_eq!(value, Some(format!("updated_{}", i)));
        }
    }

    #[test]
    fn test_hash_collision_with_removes() {
        let config = LockFreeHashMapConfig {
            initial_buckets: 4,
            load_factor: 0.75,
            max_buckets: 1024,
        };
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Insert keys that may collide
        for i in 0..30 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        // Remove some keys
        for i in 0..10 {
            let removed = map.remove(&format!("key_{}", i));
            assert_eq!(removed, Some(format!("value_{}", i)));
        }

        // Verify remaining keys are still accessible
        for i in 10..30 {
            let value = map.get(&format!("key_{}", i));
            assert_eq!(value, Some(format!("value_{}", i)));
        }

        // Verify removed keys are gone
        for i in 0..10 {
            assert!(!map.contains_key(&format!("key_{}", i)));
        }

        assert_eq!(map.len(), 20);
    }
}

/// Test suite for concurrent access
#[cfg(test)]
mod concurrent_access_tests {
    use super::*;

    #[test]
    fn test_concurrent_inserts() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let mut handles = vec![];

        // Spawn 10 threads, each inserting 100 items
        for thread_id in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = format!("thread_{}_value_{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were inserted
        assert_eq!(map.len(), 1000);

        // Verify items are retrievable
        for thread_id in 0..10 {
            for i in 0..100 {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let expected_value = format!("thread_{}_value_{}", thread_id, i);
                let value = map.get(&key);
                assert_eq!(value, Some(expected_value));
            }
        }
    }

    #[test]
    fn test_concurrent_reads() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));

        // Pre-populate map
        for i in 0..100 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        let mut handles = vec![];

        // Spawn 10 reader threads
        for _ in 0..10 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    for i in 0..100 {
                        let key = format!("key_{}", i);
                        let value = map_clone.get(&key);
                        assert_eq!(value, Some(format!("value_{}", i)));
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_insert_and_get() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let mut handles = vec![];

        // Spawn writer threads
        for thread_id in 0..5 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = format!("key_{}_{}", thread_id, i);
                    let value = format!("value_{}_{}", thread_id, i);
                    map_clone.insert(key, value);
                }
            });
            handles.push(handle);
        }

        // Spawn reader threads
        for _ in 0..5 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    thread::sleep(Duration::from_micros(10));
                    // Try to read some keys
                    for i in 0..10 {
                        let key = format!("key_0_{}", i);
                        let _ = map_clone.get(&key);
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify writer data is present
        assert_eq!(map.len(), 250);
    }

    #[test]
    fn test_concurrent_insert_and_remove() {
        let config = LockFreeHashMapConfig::default();
        let map = Arc::new(LockFreeHashMap::new(config));
        let mut handles = vec![];

        // Pre-populate
        for i in 0..100 {
            map.insert(format!("key_{}", i), format!("value_{}", i));
        }

        // Spawn inserter threads
        for thread_id in 0..3 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = format!("new_key_{}_{}", thread_id, i);
                    map_clone.insert(key, "new_value".to_string());
                }
            });
            handles.push(handle);
        }

        // Spawn remover threads
        for _ in 0..3 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..30 {
                    let key = format!("key_{}", i);
                    let _ = map_clone.remove(&key);
                    thread::sleep(Duration::from_micros(1));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Map should have some items remaining
        assert!(map.len() > 0);
    }
}

/// Test suite for edge cases
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_hashmap_empty_string_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        map.insert(String::new(), "empty_key_value".to_string());
        let value = map.get(&String::new());
        assert_eq!(value, Some("empty_key_value".to_string()));
    }

    #[test]
    fn test_hashmap_large_string_key() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        let large_key = "a".repeat(10000);
        map.insert(large_key.clone(), "large_key_value".to_string());
        let value = map.get(&large_key);
        assert_eq!(value, Some("large_key_value".to_string()));
    }

    #[test]
    fn test_hashmap_integer_keys() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<i32, String> = LockFreeHashMap::new(config);

        for i in 0..100 {
            map.insert(i, format!("value_{}", i));
        }

        for i in 0..100 {
            let value = map.get(&i);
            assert_eq!(value, Some(format!("value_{}", i)));
        }
    }

    #[test]
    fn test_hashmap_large_values() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, Vec<u8>> = LockFreeHashMap::new(config);

        let large_value = vec![0u8; 1024 * 1024]; // 1MB
        map.insert("large_key".to_string(), large_value.clone());
        let retrieved = map.get(&"large_key".to_string());
        assert_eq!(retrieved, Some(large_value));
    }

    #[test]
    fn test_hashmap_rapid_insert_remove() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, String> = LockFreeHashMap::new(config);

        // Rapid insert/remove cycles
        for i in 0..1000 {
            let key = format!("key_{}", i);
            map.insert(key.clone(), "value".to_string());
            let removed = map.remove(&key);
            assert_eq!(removed, Some("value".to_string()));
        }

        assert!(map.is_empty());
    }

    #[test]
    fn test_hashmap_update_chain() {
        let config = LockFreeHashMapConfig::default();
        let map: LockFreeHashMap<String, i32> = LockFreeHashMap::new(config);

        let key = "test_key".to_string();

        // Multiple updates
        for i in 0..100 {
            let old_value = map.insert(key.clone(), i);
            if i > 0 {
                assert_eq!(old_value, Some(i - 1));
            } else {
                assert_eq!(old_value, None);
            }
        }

        let final_value = map.get(&key);
        assert_eq!(final_value, Some(99));
    }
}
