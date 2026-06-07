//! Hazard Pointers and Safe Concurrency Tests
//!
//! This module provides comprehensive tests for hazard pointer implementation
//! and safe concurrency primitives, focusing on memory safety and thread safety.

use f4kvs_core::safe_concurrency_wrappers::{
    SafeConcurrentHashMap, SafeConcurrentHashMapConfig, SafeConcurrentQueue, SafeConcurrentStack,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Test suite for hazard pointer memory safety
#[cfg(test)]
mod hazard_pointer_tests {
    use super::*;

    #[test]
    fn test_memory_safety_under_concurrent_access() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 8;
        let operations_per_thread = 1000;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("hazard_key_{}_{}", thread_id, i);
                    let value = format!("hazard_value_{}_{}", thread_id, i);

                    // Insert
                    map_clone.insert(key.clone(), value.clone());

                    // Read immediately
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Update
                    let new_value = format!("updated_hazard_value_{}_{}", thread_id, i);
                    map_clone.insert(key.clone(), new_value.clone());

                    // Verify update
                    let final_retrieved = map_clone.get(&key);
                    assert_eq!(final_retrieved, Some(new_value));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state
        assert_eq!(map.len(), (num_threads * operations_per_thread) as usize);
    }

    #[test]
    fn test_memory_safety_with_rapid_allocations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 500;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("rapid_key_{}_{}", thread_id, i);
                    let value = format!("rapid_value_{}_{}", thread_id, i);

                    // Rapid insert/delete cycles
                    map_clone.insert(key.clone(), value.clone());
                    let _ = map_clone.remove(&key);

                    // Insert again
                    let new_value = format!("new_rapid_value_{}_{}", thread_id, i);
                    map_clone.insert(key.clone(), new_value.clone());

                    // Verify
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(new_value));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_memory_safety_with_large_values() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("large_key_{}_{}", thread_id, i);
                    let large_value = "x".repeat(10000); // 10KB value

                    map_clone.insert(key.clone(), large_value.clone());

                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(large_value));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

/// Test suite for safe concurrency primitives
#[cfg(test)]
mod safe_concurrency_tests {
    use super::*;

    #[test]
    fn test_safe_concurrent_stack_operations() {
        let stack = Arc::new(SafeConcurrentStack::<i32>::new());
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = vec![];

        // Spawn push threads
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

        // Wait for all push operations
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
    fn test_safe_concurrent_queue_operations() {
        let queue = Arc::new(SafeConcurrentQueue::<i32>::new());
        let num_threads = 8;
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

        // Wait for all enqueue operations
        for handle in handles {
            handle.join().unwrap();
        }

        // Collect all values by dequeueing
        let mut collected_values = vec![];
        while let Some(value) = queue.dequeue() {
            collected_values.push(value);
        }

        // Verify we got all expected values
        assert_eq!(
            collected_values.len(),
            (num_threads * operations_per_thread) as usize
        );
    }

    #[test]
    fn test_safe_concurrent_mixed_operations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let stack = Arc::new(SafeConcurrentStack::<i32>::new());
        let queue = Arc::new(SafeConcurrentQueue::<i32>::new());

        let num_threads = 4;
        let operations_per_thread = 50;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let stack_clone = stack.clone();
            let queue_clone = queue.clone();

            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let value = thread_id * operations_per_thread + i;

                    // HashMap operations
                    let key = format!("mixed_key_{}_{}", thread_id, i);
                    map_clone.insert(key, value);

                    // Stack operations
                    stack_clone.push(value);

                    // Queue operations
                    queue_clone.enqueue(value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all structures have correct state
        assert_eq!(map.len(), (num_threads * operations_per_thread) as usize);

        // Collect from stack and queue
        let mut stack_values = vec![];
        while let Some(value) = stack.pop() {
            stack_values.push(value);
        }

        let mut queue_values = vec![];
        while let Some(value) = queue.dequeue() {
            queue_values.push(value);
        }

        assert_eq!(
            stack_values.len(),
            (num_threads * operations_per_thread) as usize
        );
        assert_eq!(
            queue_values.len(),
            (num_threads * operations_per_thread) as usize
        );
    }
}

/// Test suite for memory pressure and stress testing
#[cfg(test)]
mod memory_pressure_tests {
    use super::*;

    #[test]
    fn test_memory_pressure_with_rapid_allocations() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 6;
        let operations_per_thread = 200;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("pressure_key_{}_{}", thread_id, i);
                    let value = format!("pressure_value_{}_{}", thread_id, i);

                    // Insert
                    map_clone.insert(key.clone(), value.clone());

                    // Small delay to allow other threads to interfere
                    thread::sleep(Duration::from_micros(1));

                    // Read
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Remove
                    let removed = map_clone.remove(&key);
                    assert_eq!(removed, Some(format!("pressure_value_{}_{}", thread_id, i)));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Map should be empty after all operations
        assert!(map.is_empty());
    }

    #[test]
    fn test_memory_pressure_with_large_values() {
        let map = Arc::new(SafeConcurrentHashMap::<String, String>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 50;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("large_pressure_key_{}_{}", thread_id, i);
                    let large_value = "x".repeat(5000); // 5KB value

                    map_clone.insert(key.clone(), large_value.clone());

                    // Verify
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(large_value));

                    // Remove
                    let removed = map_clone.remove(&key);
                    assert!(removed.is_some());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Map should be empty
        assert!(map.is_empty());
    }

    #[test]
    fn test_memory_pressure_with_concurrent_structures() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let stack = Arc::new(SafeConcurrentStack::<i32>::new());
        let queue = Arc::new(SafeConcurrentQueue::<i32>::new());

        let num_threads = 4;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let stack_clone = stack.clone();
            let queue_clone = queue.clone();

            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let value = thread_id * operations_per_thread + i;

                    // All three operations
                    let key = format!("concurrent_key_{}_{}", thread_id, i);
                    map_clone.insert(key, value);
                    stack_clone.push(value);
                    queue_clone.enqueue(value);

                    // Small delay
                    thread::sleep(Duration::from_micros(1));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all structures have correct state
        assert_eq!(map.len(), (num_threads * operations_per_thread) as usize);

        // Collect from stack and queue
        let mut stack_count = 0;
        while stack.pop().is_some() {
            stack_count += 1;
        }

        let mut queue_count = 0;
        while queue.dequeue().is_some() {
            queue_count += 1;
        }

        assert_eq!(stack_count, num_threads * operations_per_thread);
        assert_eq!(queue_count, num_threads * operations_per_thread);
    }
}

/// Test suite for thread safety and race condition prevention
#[cfg(test)]
mod thread_safety_tests {
    use super::*;

    #[test]
    fn test_no_data_races_in_concurrent_access() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("race_key_{}_{}", thread_id, i);
                    let value = thread_id * operations_per_thread + i;

                    // Insert
                    map_clone.insert(key.clone(), value);

                    // Read
                    let retrieved = map_clone.get(&key);
                    assert_eq!(retrieved, Some(value));

                    // Update
                    let new_value = value + 1000;
                    map_clone.insert(key.clone(), new_value);

                    // Verify update
                    let final_retrieved = map_clone.get(&key);
                    assert_eq!(final_retrieved, Some(new_value));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state
        assert_eq!(map.len(), (num_threads * operations_per_thread) as usize);
    }

    #[test]
    fn test_atomic_operations_consistency() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 200;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("atomic_key_{}_{}", thread_id, i);
                    let value = thread_id * operations_per_thread + i;

                    // Multiple atomic operations
                    map_clone.insert(key.clone(), value);
                    let retrieved1 = map_clone.get(&key);
                    assert_eq!(retrieved1, Some(value));

                    // Update
                    let new_value = value + 5000;
                    map_clone.insert(key.clone(), new_value);
                    let retrieved2 = map_clone.get(&key);
                    assert_eq!(retrieved2, Some(new_value));

                    // Remove
                    let removed = map_clone.remove(&key);
                    assert_eq!(removed, Some(new_value));

                    // Verify removal
                    let final_retrieved = map_clone.get(&key);
                    assert_eq!(final_retrieved, None);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Map should be empty
        assert!(map.is_empty());
    }

    #[test]
    fn test_concurrent_size_consistency() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let num_threads = 4;
        let operations_per_thread = 100;

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("size_key_{}_{}", thread_id, i);
                    let value = thread_id * operations_per_thread + i;

                    // Insert
                    map_clone.insert(key.clone(), value);

                    // Check size is consistent
                    let size = map_clone.len();
                    assert!(size > 0);

                    // Remove
                    let _ = map_clone.remove(&key);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Map should be empty
        assert!(map.is_empty());
    }
}

/// Test suite for performance under concurrent load
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

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
                    let key = format!("perf_key_{}_{}", thread_id, i);
                    let value = format!("perf_value_{}_{}", thread_id, i);
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

    #[test]
    fn test_mixed_operations_performance() {
        let map = Arc::new(SafeConcurrentHashMap::<String, i32>::new(
            SafeConcurrentHashMapConfig::default(),
        ));
        let stack = Arc::new(SafeConcurrentStack::<i32>::new());
        let queue = Arc::new(SafeConcurrentQueue::<i32>::new());

        let num_threads = 4;
        let operations_per_thread = 500;

        let start = Instant::now();

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = map.clone();
            let stack_clone = stack.clone();
            let queue_clone = queue.clone();

            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let value = thread_id * operations_per_thread + i;

                    // All three operations
                    let key = format!("mixed_perf_key_{}_{}", thread_id, i);
                    map_clone.insert(key, value);
                    stack_clone.push(value);
                    queue_clone.enqueue(value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        let total_operations = num_threads * operations_per_thread * 3; // 3 operations per iteration

        println!(
            "Completed {} mixed operations in {:?}",
            total_operations, duration
        );
        println!(
            "Operations per second: {}",
            total_operations as f64 / duration.as_secs_f64()
        );

        // Should complete in reasonable time
        assert!(duration.as_secs() < 5);
        assert_eq!(map.len(), (num_threads * operations_per_thread) as usize);
    }
}
