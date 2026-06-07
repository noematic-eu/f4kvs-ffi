//! Memory safety tests for f4kvs-core unsafe code
//!
//! These tests are designed to be run with Miri to detect memory safety issues.
//! Run with: cargo miri test --test memory_safety_tests

use f4kvs_core::cache_efficient_allocator::CacheEfficientAllocator;
use f4kvs_core::lockfree::{LockFreeHashMap, LockFreeHashMapConfig};
use f4kvs_core::memory_pool::{MemoryPool, MemoryPoolConfig};
use f4kvs_core::simd::{SimdConfig, SimdStringOps, SimdUtils};
use std::alloc::Layout;

#[test]
fn test_lockfree_hashmap_memory_safety() {
    // Test basic operations don't cause memory issues
    let map = LockFreeHashMap::new(LockFreeHashMapConfig::default());

    // Test insert and get operations
    assert_eq!(map.insert("key1".to_string(), "value1".to_string()), None);
    assert_eq!(map.get(&"key1".to_string()), Some("value1".to_string()));

    // Test concurrent access (basic)
    let map = std::sync::Arc::new(map);
    let map_clone = std::sync::Arc::clone(&map);

    let handle = std::thread::spawn(move || {
        for i in 0..100 {
            let key = format!("thread_key_{}", i);
            let value = format!("thread_value_{}", i);
            map_clone.insert(key, value);
        }
    });

    // Main thread also does operations
    for i in 0..100 {
        let key = format!("main_key_{}", i);
        let value = format!("main_value_{}", i);
        map.insert(key, value);
    }

    handle.join().unwrap();

    // Verify no crashes occurred
    assert!(!map.is_empty());
}

#[test]
fn test_simd_operations_memory_safety() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test with various input sizes
    let test_cases = vec![
        b"Hello, World!".to_vec(),
        vec![0u8; 1000], // Large buffer
        vec![0u8; 1],    // Single byte
        vec![],          // Empty buffer
    ];

    for haystack in test_cases {
        // Test find_byte with different needles
        for needle in [b'a', b'z', 0u8, 255u8] {
            let _result = ops.find_byte(&haystack, needle);
            // Just ensure no crash occurs
        }

        // Test find_substring
        let needle = b"test";
        let _result = ops.find_substring(&haystack, needle);
    }
}

#[test]
fn test_simd_utils_memory_safety() {
    // Test pointer alignment function
    let test_cases = vec![
        (16, 16), // Already aligned
        (15, 16), // Needs alignment
        (32, 32), // Large alignment
        (0, 16),  // Edge case
    ];

    for (addr, alignment) in test_cases {
        let ptr = addr as *mut u8;
        let result = unsafe { SimdUtils::align_pointer(ptr, alignment) };

        // Should either succeed or return an error, but not crash
        match result {
            Ok(aligned_ptr) => {
                // Verify alignment
                assert!(SimdUtils::is_aligned(aligned_ptr, alignment));
            }
            Err(_) => {
                // Error is acceptable for invalid inputs
            }
        }
    }
}

#[test]
fn test_memory_pool_safety() {
    let config = MemoryPoolConfig {
        block_size: 1024,
        max_pool_size: 10,
        initial_blocks: 2,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config.clone()).unwrap();

    // Test allocation and deallocation
    let mut blocks = Vec::new();
    for _ in 0..5 {
        if let Ok(block) = pool.allocate() {
            blocks.push(block);
        }
    }

    // Deallocate all blocks
    for block in blocks {
        pool.deallocate(block).unwrap();
    }

    // Verify stats are reasonable
    let stats = pool.get_stats();
    // Invariants: utilization within [0,100], and in-use cannot exceed total allocated
    assert!(stats.utilization_percent >= 0.0 && stats.utilization_percent <= 100.0);
    assert!(stats.total_allocated >= stats.blocks_in_use);
    assert_eq!(
        stats.total_memory_bytes,
        stats.total_allocated * config.block_size
    );
    // Available + in-use should never exceed total allocated
    assert!(stats.blocks_available + stats.blocks_in_use <= stats.total_allocated);
}

#[test]
fn test_cache_efficient_allocator_safety() {
    let allocator = CacheEfficientAllocator::new();

    // Test various allocation sizes
    let sizes = vec![8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

    let mut allocations = Vec::new();
    for size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        if let Ok(ptr) = allocator.allocate(layout) {
            allocations.push((ptr, layout));
        }
    }

    // Deallocate all
    for (ptr, layout) in allocations {
        allocator.deallocate(ptr, layout);
    }

    // Verify stats
    let stats = allocator.stats();
    assert!(stats.allocation_count > 0);
    assert!(stats.deallocation_count > 0);
}

#[test]
fn test_concurrent_memory_operations() {
    // Test concurrent access to memory pool
    let config = MemoryPoolConfig {
        block_size: 512,
        max_pool_size: 20,
        initial_blocks: 5,
        enable_stats: true,
    };

    let pool = std::sync::Arc::new(MemoryPool::new(config).unwrap());
    let mut handles = Vec::new();

    // Spawn multiple threads doing allocations
    for _ in 0..4 {
        let pool_clone = std::sync::Arc::clone(&pool);
        let handle = std::thread::spawn(move || {
            let mut blocks = Vec::new();
            for _ in 0..10 {
                if let Ok(block) = pool_clone.allocate() {
                    blocks.push(block);
                }
            }

            // Deallocate all blocks
            for block in blocks {
                let _ = pool_clone.deallocate(block);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    let stats = pool.get_stats();
    assert!(stats.blocks_in_use == 0); // All blocks should be returned
}

#[test]
fn test_edge_cases_and_boundary_conditions() {
    // Test with edge case inputs
    let map = LockFreeHashMap::new(LockFreeHashMapConfig::default());

    // Test with empty strings
    map.insert("".to_string(), "".to_string());
    assert_eq!(map.get(&"".to_string()), Some("".to_string()));

    // Test with very long strings
    let long_key = "a".repeat(10000);
    let long_value = "b".repeat(10000);
    map.insert(long_key.clone(), long_value.clone());
    assert_eq!(map.get(&long_key), Some(long_value));

    // Test with special characters
    let special_key = "key with spaces and symbols!@#$%^&*()".to_string();
    map.insert(special_key.clone(), "special_value".to_string());
    assert_eq!(map.get(&special_key), Some("special_value".to_string()));
}

// Stress test for memory safety
#[test]
fn test_memory_stress_test() {
    let map = std::sync::Arc::new(LockFreeHashMap::new(LockFreeHashMapConfig::default()));
    let mut handles = Vec::new();

    // Spawn many threads doing various operations
    for thread_id in 0..10 {
        let map_clone = std::sync::Arc::clone(&map);
        let handle = std::thread::spawn(move || {
            for i in 0..1000 {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let value = format!("thread_{}_value_{}", thread_id, i);

                // Insert
                map_clone.insert(key.clone(), value.clone());

                // Get
                let retrieved = map_clone.get(&key);
                assert_eq!(retrieved, Some(value));

                // Remove occasionally
                if i % 100 == 0 {
                    map_clone.remove(&key);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state is consistent
    let final_len = map.len();
    assert!(final_len > 0);

    // Clear and verify cleanup
    map.clear();
    assert!(map.is_empty());
}
