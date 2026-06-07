//! Comprehensive safe SIMD tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for safe SIMD scenarios including:
//! - Safety property validation
//! - Miri memory safety tests
//! - Concurrent SIMD operations
//! - Error propagation testing

#![cfg(feature = "safe-simd")]

use f4kvs_core::safe_simd::{SafeSimdConfig, SafeSimdManager};
use std::sync::Arc;

#[test]
fn test_safety_property_validation() {
    let manager = SafeSimdManager::default();

    // Test that all operations are safe (no panics, no UB)
    let data = b"test data for safety validation";

    // All operations should complete without panicking
    let _ = manager.find_byte(data, b't');
    let _ = manager.find_substring(data, b"test");
    let _ = manager.compare_bytes(data, data);
    let _ = manager.memory_compare(data, data);
    let _ = manager.memory_search(data, b"test");

    let mut dst = vec![0u8; data.len()];
    let _ = manager.bulk_copy(data, &mut dst);
    manager.bulk_zero(&mut dst);
}

#[test]
fn test_miri_memory_safety() {
    // These tests are designed to be run with Miri for memory safety validation
    let manager = SafeSimdManager::default();

    // Test with various memory layouts
    let mut data = Vec::with_capacity(200);
    data.extend_from_slice(&[1u8; 100]);
    data.extend_from_slice(&[2u8; 100]);

    // Operations should be memory-safe
    let _ = manager.find_byte(&data, b'1');
    let _ = manager.find_substring(&data, &[1u8; 10]);

    let mut dst = vec![0u8; data.len()];
    assert!(manager.bulk_copy(&data, &mut dst).is_ok());
}

#[test]
fn test_concurrent_simd_operations() {
    let manager = Arc::new(SafeSimdManager::default());

    // Spawn multiple concurrent operations
    let mut handles = Vec::new();
    for i in 0..50 {
        let manager_clone: Arc<SafeSimdManager> = Arc::clone(&manager);
        let handle = std::thread::spawn(move || {
            let data = format!("test_data_{}", i).into_bytes();
            let needle = b'_';

            // Perform various operations concurrently
            let find_result = manager_clone.find_byte(&data, needle);
            let search_result = manager_clone.memory_search(&data, b"test");
            let mut dst = vec![0u8; data.len()];
            let copy_result = manager_clone.bulk_copy(&data, &mut dst);

            (find_result, search_result, copy_result)
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        let (find_result, search_result, copy_result) = handle.join().unwrap();
        assert!(find_result.is_some());
        assert!(search_result.is_some());
        assert!(copy_result.is_ok());
    }
}

#[test]
fn test_error_propagation_testing() {
    let manager = SafeSimdManager::default();

    // Test error propagation for bulk_copy
    let test_cases = vec![
        (b"hello".as_slice(), vec![0u8; 3], true),  // Too small
        (b"hello".as_slice(), vec![0u8; 10], true), // Too large
        (b"hello".as_slice(), vec![0u8; 5], false), // Correct size
    ];

    for (src, mut dst, should_error) in test_cases {
        let result = manager.bulk_copy(src, &mut dst);
        if should_error {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn test_alignment_guarantees_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test with various alignments and offsets
    for alignment in [1, 2, 4, 8, 16, 32] {
        for offset in 0..alignment {
            let total = 256 + offset;
            let mut data = vec![0u8; total];
            for i in 0..total {
                data[i] = (i % 256) as u8;
            }

            // Create unaligned slice
            let unaligned = &data[offset..];
            let mut dst = vec![0u8; unaligned.len()];

            // Should work safely regardless of alignment
            assert!(manager.bulk_copy(unaligned, &mut dst).is_ok());
            assert_eq!(unaligned, dst.as_slice());
        }
    }
}

#[test]
fn test_bounds_checking_enforcement_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test with various buffer sizes
    let sizes = vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 4096];

    for size in sizes {
        let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; size];

        // Should work safely
        assert!(manager.bulk_copy(&src, &mut dst).is_ok());
        assert_eq!(src, dst);

        // Test bulk_zero
        manager.bulk_zero(&mut dst);
        assert!(dst.iter().all(|&b| b == 0));

        // Test memory operations
        assert!(manager.memory_compare(&src, &src));
        let _ = manager.memory_search(&src, &src[0..size.min(16)]);
    }
}

#[test]
fn test_safe_simd_with_disabled_config() {
    let config = SafeSimdConfig {
        enabled: false,
        preferred_width: 16,
    };
    let manager = SafeSimdManager::new(config);

    // Operations should still work with scalar fallback
    let data = b"hello world";
    assert_eq!(manager.find_byte(data, b'o'), Some(4));
    assert_eq!(manager.find_substring(data, b"world"), Some(6));
    assert_eq!(manager.memory_search(data, b"world"), Some(6));

    let src = vec![1u8; 64];
    let mut dst = vec![0u8; 64];
    assert!(manager.bulk_copy(&src, &mut dst).is_ok());
    assert_eq!(src, dst);

    // Test comparison
    assert!(manager.memory_compare(&src, &dst));
    assert_eq!(manager.compare_bytes(&src, &dst), std::cmp::Ordering::Equal);
}

#[test]
fn test_large_data_operations_safe() {
    let manager = SafeSimdManager::default();

    // Test with very large data
    let large_sizes = vec![1024, 4096, 16384, 65536];

    for size in large_sizes {
        let large_data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut large_dst = vec![0u8; size];

        // Should work safely
        assert!(manager.bulk_copy(&large_data, &mut large_dst).is_ok());
        assert_eq!(large_data, large_dst);

        // Test bulk_zero
        manager.bulk_zero(&mut large_dst);
        assert!(large_dst.iter().all(|&b| b == 0));

        // Test memory operations
        assert!(manager.memory_compare(&large_data, &large_data));
    }
}

#[test]
fn test_string_operations_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test find_byte with various patterns
    let test_cases: Vec<(&[u8], u8, Option<usize>)> = vec![
        (b"hello world", b'h', Some(0)),
        (b"hello world", b'o', Some(4)),
        (b"hello world", b'd', Some(10)),
        (b"hello world", b'z', None),
        (b"", b'a', None),
        (b"a", b'a', Some(0)),
        (b"a", b'b', None),
    ];

    for (haystack, needle, expected) in test_cases {
        assert_eq!(manager.find_byte(haystack, needle), expected);
    }

    // Test find_substring
    let substring_cases: Vec<(&[u8], &[u8], Option<usize>)> = vec![
        (b"hello world", b"hello", Some(0)),
        (b"hello world", b"world", Some(6)),
        (b"hello world", b"lo wo", Some(3)),
        (b"hello world", b"xyz", None),
        (b"hello world", b"", Some(0)),
    ];

    for (haystack, needle, expected) in substring_cases {
        assert_eq!(manager.find_substring(haystack, needle), expected);
    }
}

#[test]
fn test_memory_operations_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test memory_compare
    assert!(manager.memory_compare(b"hello", b"hello"));
    assert!(!manager.memory_compare(b"hello", b"world"));
    assert!(!manager.memory_compare(b"hello", b"hell"));
    assert!(manager.memory_compare(&[], &[]));

    // Test memory_search
    assert_eq!(manager.memory_search(b"hello world", b"hello"), Some(0));
    assert_eq!(manager.memory_search(b"hello world", b"world"), Some(6));
    assert_eq!(manager.memory_search(b"hello world", b"xyz"), None);
    assert_eq!(manager.memory_search(b"hello world", b""), Some(0));
}

#[test]
fn test_bulk_operations_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test bulk_copy with various sizes
    let sizes = vec![1, 16, 32, 64, 128, 256, 512, 1024];

    for size in &sizes {
        let src: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; *size];

        assert!(manager.bulk_copy(&src, &mut dst).is_ok());
        assert_eq!(src, dst);
    }

    // Test bulk_zero
    for size in &sizes {
        let mut data = vec![1u8; *size];
        manager.bulk_zero(&mut data);
        assert!(data.iter().all(|&b| b == 0));
    }
}

#[test]
fn test_compare_bytes_comprehensive() {
    let manager = SafeSimdManager::default();

    // Test various comparison scenarios
    let test_cases: Vec<(&[u8], &[u8], std::cmp::Ordering)> = vec![
        (b"abc", b"abc", std::cmp::Ordering::Equal),
        (b"abc", b"abd", std::cmp::Ordering::Less),
        (b"abd", b"abc", std::cmp::Ordering::Greater),
        (b"a", b"b", std::cmp::Ordering::Less),
        (b"b", b"a", std::cmp::Ordering::Greater),
        (b"", b"", std::cmp::Ordering::Equal),
        (b"", b"a", std::cmp::Ordering::Less),
        (b"a", b"", std::cmp::Ordering::Greater),
    ];

    for (a, b, expected) in test_cases {
        assert_eq!(manager.compare_bytes(a, b), expected);
    }
}

#[test]
fn test_safe_simd_config_variations() {
    let configs = vec![
        SafeSimdConfig {
            enabled: true,
            preferred_width: 16,
        },
        SafeSimdConfig {
            enabled: true,
            preferred_width: 32,
        },
        SafeSimdConfig {
            enabled: true,
            preferred_width: 64,
        },
        SafeSimdConfig {
            enabled: false,
            preferred_width: 16,
        },
    ];

    for config in configs {
        let manager = SafeSimdManager::new(config);

        // Operations should work with any config
        let data = b"test data";
        assert_eq!(manager.find_byte(data, b't'), Some(0));

        let src = vec![1u8; 64];
        let mut dst = vec![0u8; 64];
        assert!(manager.bulk_copy(&src, &mut dst).is_ok());
    }
}

#[test]
fn test_edge_case_handling() {
    let manager = SafeSimdManager::default();

    // Test with empty inputs
    assert_eq!(manager.find_byte(&[], b'a'), None);
    assert_eq!(manager.find_substring(&[], b"test"), None);
    assert_eq!(manager.memory_search(&[], b"test"), None);
    assert!(manager.memory_compare(&[], &[]));
    assert_eq!(manager.compare_bytes(&[], &[]), std::cmp::Ordering::Equal);

    // Test with single byte
    assert_eq!(manager.find_byte(b"a", b'a'), Some(0));
    assert_eq!(manager.find_substring(b"a", b"a"), Some(0));

    // Test with very long strings
    let long_string = vec![b'a'; 10000];
    assert_eq!(manager.find_byte(&long_string, b'a'), Some(0));
    assert_eq!(manager.find_byte(&long_string, b'b'), None);
}
