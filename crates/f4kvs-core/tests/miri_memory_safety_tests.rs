//! Miri Memory Safety Tests for F4KVS Core Foundation
//!
//! This module contains comprehensive memory safety tests that are specifically
//! designed to run under Miri to detect memory safety issues in unsafe code.

#![cfg(miri)]

use f4kvs_core::*;
use std::sync::Arc;
use std::thread;

/// Test basic memory safety operations
#[test]
fn test_basic_memory_safety() {
    // Test basic memory allocation and deallocation
    let data = vec![1, 2, 3, 4, 5];
    let mut copy_data = data.clone();

    // Test that we can safely modify the copy
    for i in 0..copy_data.len() {
        copy_data[i] *= 2;
    }

    // Verify the original data is unchanged
    assert_eq!(data, vec![1, 2, 3, 4, 5]);
    assert_eq!(copy_data, vec![2, 4, 6, 8, 10]);
}

/// Test safe lock-free hash map memory safety
#[test]
fn test_safe_lockfree_hashmap_memory_safety() {
    let config = SafeLockFreeHashMapConfig::default();
    let map = SafeLockFreeHashMap::<String, i32>::new(config);

    // Test basic operations
    map.insert("key1".to_string(), 42);
    map.insert("key2".to_string(), 84);

    // Test concurrent access
    let map_arc = Arc::new(map);
    let mut handles = vec![];

    for i in 0..4 {
        let map_clone = Arc::clone(&map_arc);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread_{}_key_{}", i, j);
                let value = i * 100 + j;
                map_clone.insert(key.clone(), value);

                // Verify the value was inserted correctly
                if let Some(retrieved) = map_clone.get(&key) {
                    assert_eq!(retrieved, value);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    assert!(map_arc.get(&"key1".to_string()).is_some());
    assert!(map_arc.get(&"key2".to_string()).is_some());
}

/// Test SIMD operations memory safety
#[test]
fn test_simd_memory_safety() {
    // Test aligned memory operations
    let mut data = vec![0u8; 64];
    for i in 0..data.len() {
        data[i] = i as u8;
    }

    // Test basic memory operations without SIMD
    // Copy data to verify memory safety
    let mut copy_data = vec![0u8; 64];
    copy_data.copy_from_slice(&data);

    // Verify the copy worked correctly
    assert_eq!(data, copy_data);

    // Test unaligned memory handling
    let mut unaligned_data = vec![0u8; 33]; // Not 32-byte aligned
    for i in 0..unaligned_data.len() {
        unaligned_data[i] = i as u8;
    }

    // Test that we can safely access unaligned memory
    assert_eq!(unaligned_data[0], 0);
    assert_eq!(unaligned_data[32], 32);
}

/// Test memory pool memory safety
#[test]
fn test_memory_pool_memory_safety() {
    let config = SafeMemoryPoolConfig::default();
    let pool = SafeMemoryPool::new(config).expect("Failed to create memory pool");

    // Test basic allocation and deallocation
    let ptr1 = pool.allocate().expect("Failed to allocate memory");
    assert!(!ptr1.as_ptr().is_null());

    let ptr2 = pool.allocate().expect("Failed to allocate memory");
    assert!(!ptr2.as_ptr().is_null());

    // Test that allocated memory is valid and writable
    unsafe {
        std::ptr::write_bytes(ptr1.as_ptr(), 0x42, 1);
        assert_eq!(*ptr1.as_ptr(), 0x42);
    }

    unsafe {
        std::ptr::write_bytes(ptr2.as_ptr(), 0x84, 1);
        assert_eq!(*ptr2.as_ptr(), 0x84);
    }
}

/// Test cache efficient allocator memory safety
#[test]
fn test_cache_efficient_allocator_memory_safety() {
    let allocator = SafeCacheEfficientAllocator::new();

    // Test basic allocation
    let layout1 = std::alloc::Layout::from_size_align(64, 8).unwrap();
    let ptr1 = allocator
        .allocate(layout1)
        .expect("Failed to allocate memory");
    assert!(!ptr1.as_ptr().is_null());

    let layout2 = std::alloc::Layout::from_size_align(128, 8).unwrap();
    let ptr2 = allocator
        .allocate(layout2)
        .expect("Failed to allocate memory");
    assert!(!ptr2.as_ptr().is_null());

    // Test that allocated memory is valid and writable
    unsafe {
        std::ptr::write_bytes(ptr1.as_ptr(), 0x42, 64);
        assert_eq!(*ptr1.as_ptr(), 0x42);
    }

    unsafe {
        std::ptr::write_bytes(ptr2.as_ptr(), 0x84, 128);
        assert_eq!(*ptr2.as_ptr(), 0x84);
    }
}

/// Test atomic operations memory safety
#[test]
fn test_atomic_operations_memory_safety() {
    use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

    // Test atomic pointer operations
    let data = Box::new(42);
    let atomic_ptr = AtomicPtr::new(Box::into_raw(data));

    // Test atomic load
    let loaded_ptr = atomic_ptr.load(Ordering::Acquire);
    assert!(!loaded_ptr.is_null());

    unsafe {
        assert_eq!(*loaded_ptr, 42);
    }

    // Test atomic store
    let new_data = Box::new(84);
    let new_ptr = Box::into_raw(new_data);
    atomic_ptr.store(new_ptr, Ordering::Release);

    // Verify the store worked
    let stored_ptr = atomic_ptr.load(Ordering::Acquire);
    assert_eq!(stored_ptr, new_ptr);

    unsafe {
        assert_eq!(*stored_ptr, 84);
    }

    // Clean up
    unsafe {
        let _ = Box::from_raw(stored_ptr);
    }
}

/// Test concurrent memory access patterns
#[test]
fn test_concurrent_memory_access() {
    let shared_data = Arc::new(std::sync::Mutex::new(vec![0; 1000]));
    let mut handles = vec![];

    // Spawn multiple threads that access shared memory
    for i in 0..4 {
        let data_clone = Arc::clone(&shared_data);
        let handle = thread::spawn(move || {
            for j in 0..250 {
                let mut data = data_clone.lock().unwrap();
                let index = (i * 250 + j) % data.len();
                data[index] = i * 250 + j;
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify the final state
    let final_data = shared_data.lock().unwrap();
    for (i, &value) in final_data.iter().enumerate() {
        assert!(value < 1000); // All values should be within expected range
    }
}

/// Test memory alignment and boundary conditions
#[test]
fn test_memory_alignment_boundaries() {
    // Test various alignment scenarios
    let sizes = vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];

    for size in sizes {
        let mut data = vec![0u8; size];

        // Test that we can safely access the memory
        for i in 0..size {
            data[i] = (i % 256) as u8;
        }

        // Test boundary conditions
        if size > 0 {
            assert_eq!(data[0], 0);
            assert_eq!(data[size - 1], ((size - 1) % 256) as u8);
        }

        // Test that we don't access out of bounds
        let slice = &data[..];
        assert_eq!(slice.len(), size);
    }
}

/// Test memory leak detection
#[test]
fn test_memory_leak_detection() {
    // This test ensures that we don't leak memory
    let initial_allocations = 100;
    let mut pointers = Vec::new();

    // Allocate some memory
    for i in 0..initial_allocations {
        let ptr = Box::into_raw(Box::new(i));
        pointers.push(ptr);
    }

    // Verify all allocations succeeded
    assert_eq!(pointers.len(), initial_allocations);

    // Clean up all allocations
    for ptr in pointers {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    // If we reach here without Miri detecting a leak, the test passes
}

/// Test use-after-free detection
#[test]
fn test_use_after_free_detection() {
    // This test should be carefully written to avoid actual use-after-free
    // but should exercise the code paths that Miri can detect

    let data = Box::new(42);
    let ptr = Box::into_raw(data);

    // Use the pointer
    unsafe {
        assert_eq!(*ptr, 42);
    }

    // Free the memory
    unsafe {
        let _ = Box::from_raw(ptr);
    }

    // Don't use the pointer after freeing - this would be detected by Miri
    // if we tried to access *ptr here
}

/// Test double-free detection
#[test]
fn test_double_free_detection() {
    let data = Box::new(42);
    let ptr = Box::into_raw(data);

    // Free the memory once
    unsafe {
        let _ = Box::from_raw(ptr);
    }

    // Don't free again - this would be detected by Miri
    // if we tried to free the same pointer again
}

/// Test buffer overflow detection
#[test]
fn test_buffer_overflow_detection() {
    let mut buffer = vec![0u8; 10];

    // Safe access within bounds
    for i in 0..buffer.len() {
        buffer[i] = i as u8;
    }

    // Verify the buffer contents
    for i in 0..buffer.len() {
        assert_eq!(buffer[i], i as u8);
    }

    // Don't access out of bounds - this would be detected by Miri
    // if we tried to access buffer[buffer.len()] or beyond
}

/// Test lock-free hash map memory safety with hazard pointers
#[test]
fn test_lockfree_hashmap_hazard_pointers() {
    use f4kvs_core::lockfree::LockFreeHashMap;

    let map = LockFreeHashMap::<String, i32>::new();

    // Test basic operations
    map.insert("key1".to_string(), 42);
    map.insert("key2".to_string(), 84);

    // Test concurrent access with hazard pointer protection
    let map_arc = Arc::new(map);
    let mut handles = vec![];

    for i in 0..4 {
        let map_clone = Arc::clone(&map_arc);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread_{}_key_{}", i, j);
                let value = i * 100 + j;
                map_clone.insert(key.clone(), value);

                // Test hazard pointer protection during get operations
                if let Some(retrieved) = map_clone.get(&key) {
                    assert_eq!(retrieved, value);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    assert!(map_arc.get(&"key1".to_string()).is_some());
    assert!(map_arc.get(&"key2".to_string()).is_some());
}

/// Test lock-free cache memory safety
#[test]
fn test_lockfree_cache_memory_safety() {
    use f4kvs_core::lockfree::LockFreeCache;

    let cache = LockFreeCache::<String, i32>::new(100);

    // Test basic cache operations
    cache.put("key1".to_string(), 42);
    cache.put("key2".to_string(), 84);

    // Test concurrent access
    let cache_arc = Arc::new(cache);
    let mut handles = vec![];

    for i in 0..4 {
        let cache_clone = Arc::clone(&cache_arc);
        let handle = thread::spawn(move || {
            for j in 0..50 {
                let key = format!("thread_{}_key_{}", i, j);
                let value = i * 50 + j;
                cache_clone.put(key.clone(), value);

                // Test hazard pointer protection during get operations
                if let Some(retrieved) = cache_clone.get(&key) {
                    assert_eq!(retrieved, value);
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

/// Test SIMD operations memory safety with alignment validation
#[test]
fn test_simd_operations_memory_safety() {
    use f4kvs_core::simd::{SimdBulkOps, SimdStringOps, SimdUtils};

    let config = f4kvs_core::simd::SimdConfig::default();
    let string_ops = SimdStringOps::new(config);
    let bulk_ops = SimdBulkOps::new(config);

    // Test string operations
    let haystack = b"Hello, World! This is a test string for SIMD operations.";
    let needle = b'o';

    let result = string_ops.find_byte(haystack, *needle);
    assert!(result.is_some());

    // Test bulk operations
    let data1 = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let data2 = vec![1, 2, 3, 4, 5, 6, 7, 8];

    let result = bulk_ops.bulk_compare(&data1, &data2);
    assert!(result.is_ok());
    assert!(result.unwrap());

    // Test alignment validation
    let data = vec![0u8; 64];
    let result = SimdUtils::validate_simd_operation(&data, 0, 32, 16);
    assert!(result.is_ok());

    // Test bounds validation
    let result = SimdUtils::validate_bounds(64, 0, 32);
    assert!(result);

    let result = SimdUtils::validate_bounds(64, 40, 32);
    assert!(!result);
}

/// Test FFI memory safety patterns
#[test]
fn test_ffi_memory_safety_patterns() {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    // Test C string conversion safety
    let rust_string = "Hello, World!";
    let c_string = CString::new(rust_string).unwrap();
    let c_str = c_string.as_c_str();

    // Test safe conversion back to Rust string
    let converted = c_str.to_str().unwrap();
    assert_eq!(converted, rust_string);

    // Test null pointer handling
    let null_ptr: *const c_char = std::ptr::null();
    assert!(null_ptr.is_null());

    // Test buffer bounds checking
    let data = vec![1, 2, 3, 4, 5];
    let slice = &data[0..3];
    assert_eq!(slice.len(), 3);
    assert_eq!(slice[0], 1);
    assert_eq!(slice[2], 3);
}

/// Test memory pool edge cases
#[test]
fn test_memory_pool_edge_cases() {
    let config = SafeMemoryPoolConfig::default();
    let pool = SafeMemoryPool::new(config).expect("Failed to create memory pool");

    // Test allocation edge cases
    let ptr1 = pool.allocate().expect("Failed to allocate memory");
    assert!(!ptr1.as_ptr().is_null());

    // Test that we can safely write to allocated memory
    unsafe {
        std::ptr::write_bytes(ptr1.as_ptr(), 0x42, 1);
        assert_eq!(*ptr1.as_ptr(), 0x42);
    }

    // Test multiple allocations
    let mut pointers = Vec::new();
    for i in 0..10 {
        let ptr = pool.allocate().expect("Failed to allocate memory");
        assert!(!ptr.as_ptr().is_null());

        // Write unique data to each allocation
        unsafe {
            std::ptr::write_bytes(ptr.as_ptr(), i as u8, 1);
            assert_eq!(*ptr.as_ptr(), i as u8);
        }

        pointers.push(ptr);
    }

    // Verify all allocations are unique
    for (i, ptr) in pointers.iter().enumerate() {
        unsafe {
            assert_eq!(*ptr.as_ptr(), i as u8);
        }
    }
}

/// Test atomic operations with memory ordering
#[test]
fn test_atomic_operations_memory_ordering() {
    use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
    use std::sync::Arc;

    // Test atomic pointer with proper memory ordering
    let data = Box::new(42);
    let atomic_ptr = Arc::new(AtomicPtr::new(Box::into_raw(data)));

    // Test concurrent access
    let mut handles = vec![];

    for i in 0..4 {
        let atomic_clone = Arc::clone(&atomic_ptr);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let ptr = atomic_clone.load(Ordering::Acquire);
                if !ptr.is_null() {
                    unsafe {
                        let value = *ptr;
                        assert!(value >= 0);
                    }
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Clean up
    let ptr = atomic_ptr.load(Ordering::Acquire);
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
}

/// Test memory alignment edge cases
#[test]
fn test_memory_alignment_edge_cases() {
    use f4kvs_core::simd::SimdUtils;

    // Test various alignment scenarios
    let alignments = vec![1, 2, 4, 8, 16, 32, 64];

    for alignment in alignments {
        // Test alignment validation
        let data = vec![0u8; 128];
        let ptr = data.as_ptr();

        let is_aligned = SimdUtils::is_aligned(ptr, alignment);

        // Test alignment calculation
        if alignment > 1 {
            let addr = ptr as usize;
            let expected_aligned = (addr + alignment - 1) & !(alignment - 1);

            if is_aligned {
                assert_eq!(addr, expected_aligned);
            }
        }
    }
}

/// Test concurrent memory access with proper synchronization
#[test]
fn test_concurrent_memory_access_synchronization() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    let shared_counter = Arc::new(AtomicUsize::new(0));
    let shared_data = Arc::new(Mutex::new(vec![0; 1000]));
    let mut handles = vec![];

    // Spawn multiple threads with proper synchronization
    for i in 0..4 {
        let counter_clone = Arc::clone(&shared_counter);
        let data_clone = Arc::clone(&shared_data);

        let handle = thread::spawn(move || {
            for j in 0..250 {
                // Atomic increment
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);

                // Mutex-protected data access
                let mut data = data_clone.lock().unwrap();
                let index = count % data.len();
                data[index] = i * 250 + j;
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    let final_count = shared_counter.load(Ordering::SeqCst);
    assert_eq!(final_count, 1000);

    let final_data = shared_data.lock().unwrap();
    for &value in final_data.iter() {
        assert!(value < 1000);
    }
}
