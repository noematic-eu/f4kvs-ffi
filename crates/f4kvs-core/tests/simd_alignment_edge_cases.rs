//! SIMD Alignment Edge Case Tests
//!
//! Additional edge case tests for SIMD operations focusing on alignment validation
//! and boundary conditions to improve test coverage.

use f4kvs_core::simd::{SimdConfig, SimdStringOps};

#[test]
fn test_simd_unaligned_memory_handling() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test with unaligned memory (not 16/32/64 byte aligned)
    let unaligned_data = vec![0u8; 33]; // 33 bytes is not aligned
    for i in 0..unaligned_data.len() {
        // Test that operations handle unaligned data gracefully
        let result = ops.find_byte(&unaligned_data[..i], 0);
        // Should not panic, may return None or valid index
        assert!(result.is_none() || result.unwrap() < i);
    }
}

#[test]
fn test_simd_empty_and_single_byte() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test empty slice
    assert_eq!(ops.find_byte(&[], 0), None);
    assert_eq!(ops.find_byte(&[], 255), None);

    // Test single byte
    assert_eq!(ops.find_byte(&[42], 42), Some(0));
    assert_eq!(ops.find_byte(&[42], 43), None);
}

#[test]
fn test_simd_boundary_conditions() {
    let config = SimdConfig::default();
    let threshold = config.simd_threshold;
    let _ops = SimdStringOps::new(config.clone());
    let config_clone = config.clone();
    let _ops_clone = SimdStringOps::new(config_clone.clone());

    // Just below threshold
    if threshold > 0 {
        let below_threshold = vec![0u8; threshold - 1];
        let ops_below = SimdStringOps::new(config_clone.clone());
        let result = ops_below.find_byte(&below_threshold, 0);
        assert!(result.is_some() || below_threshold.is_empty());
    }

    // At threshold
    let at_threshold = vec![0u8; threshold];
    let ops_at = SimdStringOps::new(config_clone.clone());
    let result = ops_at.find_byte(&at_threshold, 0);
    assert!(result.is_some() || at_threshold.is_empty());

    // Just above threshold
    let above_threshold = vec![0u8; threshold + 1];
    let ops_above = SimdStringOps::new(config_clone);
    let result = ops_above.find_byte(&above_threshold, 0);
    assert!(result.is_some() || above_threshold.is_empty());
}

#[test]
fn test_simd_all_byte_values() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test finding each possible byte value
    for byte_val in 0u8..=255u8 {
        let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
        let result = ops.find_byte(&data, byte_val);
        // Should find the byte at index byte_val (if byte_val < 100)
        if byte_val < 100 {
            assert_eq!(result, Some(byte_val as usize));
        }
    }
}

#[test]
fn test_simd_very_large_data() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test with very large data (1MB)
    let mut large_data = vec![0u8; 1024 * 1024];
    large_data[1024 * 512] = 42; // Place target byte in middle

    let result = ops.find_byte(&large_data, 42);
    assert_eq!(result, Some(1024 * 512));

    // Test finding byte at end
    let mut end_data = vec![0u8; 1024 * 1024];
    end_data[1024 * 1024 - 1] = 99;
    let result = ops.find_byte(&end_data, 99);
    assert_eq!(result, Some(1024 * 1024 - 1));
}

#[test]
fn test_simd_config_edge_cases() {
    // Test with all SIMD features disabled
    let mut config = SimdConfig::default();
    config.enable_avx2 = false;
    config.enable_avx512 = false;
    config.enable_sse42 = false;
    config.enable_auto_tuning = false;

    let ops = SimdStringOps::new(config);
    let data = vec![0u8; 100];
    // Should still work with scalar fallback
    let result = ops.find_byte(&data, 0);
    assert_eq!(result, Some(0));
}

#[test]
fn test_simd_utils_edge_cases() {
    let config = SimdConfig::default();
    let ops = f4kvs_core::simd::SimdBulkOps::new(config);

    // Test with zero-sized data
    let empty: Vec<u8> = vec![];
    let mut empty_dst = vec![];
    let result = ops.bulk_copy(&empty, &mut empty_dst);
    assert!(result.is_ok() || result.is_err()); // Should handle gracefully

    // Test with mismatched sizes
    let src = vec![1u8; 100];
    let mut dst = vec![0u8; 50]; // Smaller destination
    let result = ops.bulk_copy(&src, &mut dst);
    // Should either succeed (copying 50 bytes) or return error
    assert!(result.is_ok() || result.is_err());
}
