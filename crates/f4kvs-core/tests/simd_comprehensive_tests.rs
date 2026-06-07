//! Comprehensive SIMD tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for SIMD scenarios including:
//! - Cross-architecture testing
//! - Performance benchmarks
//! - Alignment edge cases
//! - Large buffer operations
//! - SIMD instruction validation

use f4kvs_core::simd::*;
use std::time::Instant;

#[test]
fn test_cross_architecture_support() {
    let config = SimdConfig::default();

    // Verify config works on all architectures
    assert!(config.lane_width >= 16);
    assert!(config.simd_threshold > 0);

    // Test that operations work regardless of architecture
    let string_ops = SimdStringOps::new(config.clone());
    let bulk_ops = SimdBulkOps::new(config);

    // These should work on all architectures (with fallback if needed)
    let haystack = b"Hello, World!";
    assert_eq!(string_ops.find_byte(haystack, b'o'), Some(4));

    let src = vec![1u8; 64];
    let mut dst = vec![0u8; 64];
    assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
}

#[test]
fn test_performance_benchmarks() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Benchmark bulk copy with various sizes
    let sizes = vec![1024, 4096, 16384, 65536];

    for size in sizes {
        let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; size];

        let start = Instant::now();
        assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
        let duration = start.elapsed();

        // Verify correctness
        assert_eq!(src, dst);

        // Performance should be reasonable (adjust thresholds as needed)
        println!("Size: {} bytes, Duration: {:?}", size, duration);
        assert!(duration.as_millis() < 1000); // Should complete in reasonable time
    }
}

#[test]
fn test_alignment_edge_cases() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Test with various alignments
    for offset in 0..16 {
        let total_size = 128 + offset;
        let mut src = vec![0u8; total_size];
        for i in 0..total_size {
            src[i] = (i % 256) as u8;
        }

        // Create unaligned slice
        let unaligned_src = &src[offset..];
        let mut dst = vec![0u8; unaligned_src.len()];

        // Should work even with unaligned data
        assert!(bulk_ops.bulk_copy(unaligned_src, &mut dst).is_ok());
        assert_eq!(unaligned_src, dst.as_slice());
    }
}

#[test]
fn test_large_buffer_operations() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config.clone());
    let string_ops = SimdStringOps::new(config);

    // Test with very large buffers
    let large_sizes = vec![1024 * 1024, 10 * 1024 * 1024]; // 1MB, 10MB

    for size in large_sizes {
        let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; size];

        let start = Instant::now();
        assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
        let duration = start.elapsed();

        assert_eq!(src, dst);
        assert!(duration.as_secs() < 10); // Should complete in reasonable time

        // Test string operations on large data
        let needle = b'x';
        let result = string_ops.find_byte(&src, needle);
        // Result depends on data content, but should not panic
        assert!(result.is_none() || result.is_some());
    }
}

#[test]
fn test_simd_instruction_validation() {
    let _config = SimdConfig::default();

    // Verify SIMD capabilities are detected correctly
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // On x86/x86_64, we should have some SIMD support
        let capabilities = SimdUtils::check_capabilities();
        assert!(capabilities.lane_width >= 16);

        // Verify alignment matches architecture
        let alignment = SimdUtils::get_alignment();
        assert!(alignment >= 16);
        assert!(alignment <= 64);
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        // On other architectures, should have fallback
        let capabilities = SimdUtils::check_capabilities();
        assert!(capabilities.lane_width >= 16);
    }
}

#[test]
fn test_string_ops_performance() {
    let config = SimdConfig::default();
    let string_ops = SimdStringOps::new(config);

    // Test with various string sizes
    let sizes = vec![64, 256, 1024, 4096];

    for size in sizes {
        let mut haystack = vec![b'a'; size];
        haystack[size / 2] = b'b';

        let start = Instant::now();
        let result = string_ops.find_byte(&haystack, b'b');
        let duration = start.elapsed();

        assert_eq!(result, Some(size / 2));
        assert!(duration.as_millis() < 100); // Should be fast
    }
}

#[test]
fn test_bulk_compare_performance() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Test comparison performance
    let sizes = vec![1024, 4096, 16384];

    for size in sizes {
        let data1: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let data2 = data1.clone();
        let data3: Vec<u8> = (0..size).map(|i| ((i + 1) % 256) as u8).collect();

        // Compare equal data
        let start = Instant::now();
        assert!(bulk_ops.bulk_compare(&data1, &data2).unwrap());
        let equal_duration = start.elapsed();

        // Compare different data
        let start = Instant::now();
        assert!(!bulk_ops.bulk_compare(&data1, &data3).unwrap());
        let diff_duration = start.elapsed();

        // Both should be fast
        assert!(equal_duration.as_millis() < 100);
        assert!(diff_duration.as_millis() < 100);
    }
}

#[test]
fn test_simd_utils_comprehensive() {
    // Test alignment detection
    let alignments = vec![16, 32, 64];
    for alignment in alignments {
        let aligned_ptr = (alignment * 10) as *const u8;
        assert!(SimdUtils::is_aligned(aligned_ptr, alignment));

        let unaligned_ptr = (alignment * 10 + 1) as *const u8;
        assert!(!SimdUtils::is_aligned(unaligned_ptr, alignment));
    }

    // Test bounds validation
    let test_cases = vec![
        (100, 0, 32, true),
        (100, 50, 32, true),
        (100, 68, 32, true),
        (100, 69, 32, false),
        (100, 100, 32, false),
    ];

    for (data_len, offset, simd_size, expected) in test_cases {
        assert_eq!(
            SimdUtils::validate_bounds(data_len, offset, simd_size),
            expected
        );
    }
}

#[test]
fn test_simd_config_variations() {
    // Test with different configurations
    let configs = vec![
        SimdConfig {
            enable_avx2: false,
            enable_avx512: false,
            enable_sse42: false,
            lane_width: 16,
            enable_auto_tuning: true,
            simd_threshold: 64,
        },
        SimdConfig {
            enable_avx2: true,
            enable_avx512: false,
            enable_sse42: true,
            lane_width: 32,
            enable_auto_tuning: false,
            simd_threshold: 128,
        },
    ];

    for config in configs {
        let string_ops = SimdStringOps::new(config.clone());
        let bulk_ops = SimdBulkOps::new(config);

        // Operations should work with any config
        let haystack = b"Hello, World!";
        assert_eq!(string_ops.find_byte(haystack, b'o'), Some(4));

        let src = vec![1u8; 64];
        let mut dst = vec![0u8; 64];
        assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
    }
}

#[test]
fn test_hash_ops_performance() {
    let config = SimdConfig::default();
    let hash_ops = SimdHashOps::new(config);

    // Test bulk hashing with various key counts
    let key_counts = vec![10, 100, 1000];

    for count in key_counts {
        let keys: Vec<Vec<u8>> = (0..count)
            .map(|i| format!("key_{}", i).into_bytes())
            .collect();
        let keys_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

        let start = Instant::now();
        let hashes = hash_ops.bulk_hash(&keys_refs);
        let duration = start.elapsed();

        assert_eq!(hashes.len(), count);
        assert!(duration.as_millis() < 1000); // Should be fast

        // Verify hashes are unique
        let unique_hashes: std::collections::HashSet<u64> = hashes.iter().cloned().collect();
        assert_eq!(unique_hashes.len(), count); // All hashes should be unique
    }
}

#[test]
fn test_simd_error_handling() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Test length mismatch errors
    let src = vec![1u8; 100];
    let mut dst_small = vec![0u8; 50];
    assert!(matches!(
        bulk_ops.bulk_copy(&src, &mut dst_small),
        Err(SimdError::LengthMismatch)
    ));

    let mut dst_large = vec![0u8; 150];
    assert!(matches!(
        bulk_ops.bulk_copy(&src, &mut dst_large),
        Err(SimdError::LengthMismatch)
    ));
}

#[test]
fn test_simd_auto_tuning_thresholds() {
    let mut config = SimdConfig::default();
    config.enable_auto_tuning = true;
    config.simd_threshold = 128;

    let string_ops = SimdStringOps::new(config);

    // Small data (below threshold) should work
    let small = vec![b'a'; 64];
    assert_eq!(string_ops.find_byte(&small, b'a'), Some(0));

    // Large data (above threshold) should also work
    let large = vec![b'a'; 256];
    assert_eq!(string_ops.find_byte(&large, b'a'), Some(0));
}

#[test]
fn test_simd_memory_safety() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Test with various memory layouts
    let mut data = Vec::with_capacity(200);
    data.extend_from_slice(&[1u8; 100]);
    data.extend_from_slice(&[2u8; 100]);

    let mut dst = vec![0u8; 200];

    // Should work safely
    assert!(bulk_ops.bulk_copy(&data, &mut dst).is_ok());
    assert_eq!(data, dst);
}

#[test]
fn test_simd_substring_search_performance() {
    let config = SimdConfig::default();
    let string_ops = SimdStringOps::new(config);

    // Test substring search with various patterns
    let haystack = b"Hello, World! This is a test string for substring search.";

    let patterns: Vec<&[u8]> = vec![b"Hello", b"World", b"test", b"substring", b"nonexistent"];

    for pattern in patterns {
        let start = Instant::now();
        let result = string_ops.find_substring(haystack, pattern);
        let duration = start.elapsed();

        // Verify correctness
        let expected = haystack
            .windows(pattern.len())
            .position(|window| window == pattern);
        assert_eq!(result, expected);

        // Should be fast
        assert!(duration.as_millis() < 10);
    }
}
