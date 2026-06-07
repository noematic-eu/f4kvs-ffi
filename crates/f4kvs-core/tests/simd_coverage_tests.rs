//! SIMD Coverage Tests
//!
//! Comprehensive tests for SIMD operations focusing on alignment edge cases,
//! boundary conditions, fallback paths, and error handling to improve test coverage.

use f4kvs_core::simd::{SimdBulkOps, SimdConfig, SimdError, SimdHashOps, SimdStringOps, SimdUtils};

#[test]
fn test_simd_misaligned_data_operations() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config.clone());
    let bulk_ops = SimdBulkOps::new(config);

    // Test with data that's not aligned to SIMD width boundaries
    // Create data with various misalignments
    for offset in 1..=15 {
        let mut data = vec![0u8; 100 + offset];
        data[offset + 10] = 42; // Place target byte after offset

        // Test find_byte with misaligned data
        let result = ops.find_byte(&data[offset..], 42);
        assert_eq!(result, Some(10));

        // Test bulk_copy with misaligned data
        let mut dst = vec![0u8; 100];
        let src = &data[offset..offset + 100];
        assert!(bulk_ops.bulk_copy(src, &mut dst).is_ok());
        assert_eq!(src, dst.as_slice());
    }
}

#[test]
fn test_simd_non_multiple_width_sizes() {
    let config = SimdConfig::default();
    let ops = SimdBulkOps::new(config);

    // Test with sizes that are not multiples of SIMD width (16, 32, 64)
    let sizes = vec![1, 2, 3, 7, 15, 17, 31, 33, 63, 65, 100, 127, 129];

    for size in sizes {
        let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; size];

        // Test bulk_copy with non-multiple sizes
        assert!(ops.bulk_copy(&src, &mut dst).is_ok());
        assert_eq!(src, dst);

        // Test bulk_compare
        assert!(ops.bulk_compare(&src, &dst).unwrap());

        // Test bulk_zero
        ops.bulk_zero(&mut dst);
        assert!(dst.iter().all(|&b| b == 0));
    }
}

#[test]
fn test_simd_fallback_paths() {
    // Test with SIMD disabled to ensure fallback works
    let mut config = SimdConfig::default();
    config.enable_avx2 = false;
    config.enable_avx512 = false;
    config.enable_sse42 = false;
    config.enable_auto_tuning = false;

    let string_ops = SimdStringOps::new(config.clone());
    let bulk_ops = SimdBulkOps::new(config.clone());
    let hash_ops = SimdHashOps::new(config);

    // Test string operations with fallback
    let data = b"Hello, World!";
    assert_eq!(string_ops.find_byte(data, b'o'), Some(4));
    assert_eq!(string_ops.find_substring(data, b"World"), Some(7));

    // Test bulk operations with fallback
    let src = vec![1u8; 100];
    let mut dst = vec![0u8; 100];
    assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
    assert_eq!(src, dst);

    // Test hash operations with fallback
    let keys = vec![b"key1".as_slice(), b"key2".as_slice()];
    let hashes = hash_ops.bulk_hash(&keys);
    assert_eq!(hashes.len(), 2);
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn test_simd_boundary_conditions() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config.clone());
    let bulk_ops = SimdBulkOps::new(config);

    // Test with data exactly at SIMD width boundaries
    let simd_widths = vec![16, 32, 64];

    for width in simd_widths {
        // Test exactly width bytes
        let data = vec![42u8; width];
        assert_eq!(ops.find_byte(&data, 42), Some(0));

        // Test width - 1 bytes
        if width > 1 {
            let data = vec![42u8; width - 1];
            assert_eq!(ops.find_byte(&data, 42), Some(0));
        }

        // Test width + 1 bytes
        let data = vec![42u8; width + 1];
        assert_eq!(ops.find_byte(&data, 42), Some(0));

        // Test bulk operations at boundaries
        let src = vec![1u8; width];
        let mut dst = vec![0u8; width];
        assert!(bulk_ops.bulk_copy(&src, &mut dst).is_ok());
    }
}

#[test]
fn test_simd_error_handling_invalid_config() {
    let config = SimdConfig::default();
    let bulk_ops = SimdBulkOps::new(config);

    // Test length mismatch error
    let src = vec![1u8; 100];
    let mut dst = vec![0u8; 50];
    let result = bulk_ops.bulk_copy(&src, &mut dst);
    assert!(matches!(result, Err(SimdError::LengthMismatch)));

    // Test bulk_compare with mismatched lengths
    let a = vec![1u8; 100];
    let b = vec![1u8; 50];
    let result = bulk_ops.bulk_compare(&a, &b);
    assert!(matches!(result, Err(SimdError::LengthMismatch)));
}

#[test]
fn test_simd_utils_alignment_validation() {
    // Test alignment validation with various scenarios
    unsafe {
        // Test valid alignment
        let ptr = 0x1001 as *mut u8;
        let aligned = SimdUtils::align_pointer_with_bounds(ptr, 100, 16);
        assert!(aligned.is_ok());
        assert_eq!(aligned.unwrap() as usize, 0x1010);

        // Test insufficient space for alignment
        let result = SimdUtils::align_pointer_with_bounds(ptr, 10, 16);
        assert!(matches!(result, Err(SimdError::OperationFailed)));

        // Test null pointer
        let result = SimdUtils::align_pointer_with_bounds(std::ptr::null_mut(), 100, 16);
        assert!(matches!(result, Err(SimdError::OperationFailed)));

        // Test zero size
        let result = SimdUtils::align_pointer_with_bounds(ptr, 0, 16);
        assert!(matches!(result, Err(SimdError::OperationFailed)));

        // Test invalid alignment (not power of 2)
        let result = SimdUtils::align_pointer_with_bounds(ptr, 100, 15);
        assert!(matches!(result, Err(SimdError::UnsupportedFeature)));

        // Test zero alignment
        let result = SimdUtils::align_pointer_with_bounds(ptr, 100, 0);
        assert!(matches!(result, Err(SimdError::UnsupportedFeature)));
    }
}

#[test]
fn test_simd_utils_is_aligned() {
    // Test alignment detection
    let aligned_ptr = 0x1000 as *const u8;
    let unaligned_ptr = 0x1001 as *const u8;

    assert!(SimdUtils::is_aligned(aligned_ptr, 16));
    assert!(!SimdUtils::is_aligned(unaligned_ptr, 16));
    assert!(SimdUtils::is_aligned(aligned_ptr, 32));
    assert!(!SimdUtils::is_aligned(unaligned_ptr, 32));

    // Test null pointer
    assert!(!SimdUtils::is_aligned(std::ptr::null(), 16));
}

#[test]
fn test_simd_utils_validate_bounds() {
    // Test bounds validation
    assert!(SimdUtils::validate_bounds(100, 0, 32));
    assert!(SimdUtils::validate_bounds(100, 50, 32));
    assert!(!SimdUtils::validate_bounds(100, 80, 32));
    assert!(!SimdUtils::validate_bounds(100, 100, 32));
    assert!(!SimdUtils::validate_bounds(100, 101, 32));

    // Test edge cases
    assert!(SimdUtils::validate_bounds(32, 0, 32));
    assert!(!SimdUtils::validate_bounds(31, 0, 32));
}

#[test]
fn test_simd_utils_validate_simd_operation() {
    let data = b"Hello, World!";

    // Test valid operation
    assert!(SimdUtils::validate_simd_operation(data, 0, 8, 16).is_ok());

    // Test invalid bounds
    assert!(SimdUtils::validate_simd_operation(data, 10, 8, 16).is_err());

    // Test empty data
    assert!(SimdUtils::validate_simd_operation(&[], 0, 8, 16).is_err());

    // Test overflow conditions
    assert!(SimdUtils::validate_simd_operation(data, 100, 8, 16).is_err());
}

#[test]
fn test_simd_string_ops_substring_edge_cases() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test empty needle
    assert_eq!(ops.find_substring(b"Hello", b""), Some(0));

    // Test needle longer than haystack
    assert_eq!(ops.find_substring(b"Hi", b"Hello"), None);

    // Test needle at start
    assert_eq!(ops.find_substring(b"Hello, World!", b"Hello"), Some(0));

    // Test needle at end
    assert_eq!(ops.find_substring(b"Hello, World!", b"World!"), Some(7));

    // Test needle in middle
    assert_eq!(ops.find_substring(b"Hello, World!", b", "), Some(5));
}

#[test]
fn test_simd_bulk_ops_edge_cases() {
    let config = SimdConfig::default();
    let ops = SimdBulkOps::new(config);

    // Test empty slices
    let empty: &[u8] = &[];
    let mut empty_dst = vec![];
    assert!(ops.bulk_copy(empty, &mut empty_dst).is_ok());
    assert!(ops.bulk_compare(empty, empty).unwrap());

    // Test single byte
    let single = vec![42u8];
    let mut single_dst = vec![0u8];
    assert!(ops.bulk_copy(&single, &mut single_dst).is_ok());
    assert_eq!(single, single_dst);

    // Test bulk_zero with various sizes
    for size in [1, 2, 15, 16, 17, 31, 32, 33, 63, 64, 65, 100] {
        let mut data = vec![1u8; size];
        ops.bulk_zero(&mut data);
        assert!(data.iter().all(|&b| b == 0));
    }
}

#[test]
fn test_simd_hash_ops_edge_cases() {
    let config = SimdConfig::default();
    let ops = SimdHashOps::new(config);

    // Test empty keys
    let empty_keys: Vec<&[u8]> = vec![];
    let hashes = ops.bulk_hash(&empty_keys);
    assert_eq!(hashes.len(), 0);

    // Test single key
    let keys = vec![b"key1".as_slice()];
    let hashes = ops.bulk_hash(&keys);
    assert_eq!(hashes.len(), 1);

    // Test multiple keys
    let keys = vec![b"key1".as_slice(), b"key2".as_slice(), b"key3".as_slice()];
    let hashes = ops.bulk_hash(&keys);
    assert_eq!(hashes.len(), 3);
    // All hashes should be different
    assert_ne!(hashes[0], hashes[1]);
    assert_ne!(hashes[1], hashes[2]);
    assert_ne!(hashes[0], hashes[2]);

    // Test empty key
    let keys = vec![b"".as_slice(), b"key".as_slice()];
    let hashes = ops.bulk_hash(&keys);
    assert_eq!(hashes.len(), 2);
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn test_simd_config_edge_cases() {
    // Test default config
    let config = SimdConfig::default();
    assert!(config.lane_width >= 16);
    assert!(config.simd_threshold > 0);

    // Test custom config with all features disabled
    let mut config = SimdConfig::default();
    config.enable_avx2 = false;
    config.enable_avx512 = false;
    config.enable_sse42 = false;
    config.enable_auto_tuning = false;
    config.simd_threshold = 128;

    let ops = SimdStringOps::new(config);
    let data = vec![0u8; 200];
    // Should use scalar fallback
    assert_eq!(ops.find_byte(&data, 0), Some(0));
}

#[test]
fn test_simd_auto_tuning_threshold() {
    let mut config = SimdConfig::default();
    config.enable_auto_tuning = true;
    config.simd_threshold = 100;

    let ops = SimdStringOps::new(config.clone());

    // Test below threshold (should use scalar)
    let small_data = vec![0u8; 50];
    assert_eq!(ops.find_byte(&small_data, 0), Some(0));

    // Test above threshold (should use SIMD if available)
    let large_data = vec![0u8; 200];
    assert_eq!(ops.find_byte(&large_data, 0), Some(0));

    // Test at threshold
    let threshold_data = vec![0u8; 100];
    assert_eq!(ops.find_byte(&threshold_data, 0), Some(0));
}

#[test]
fn test_simd_compare_strings_edge_cases() {
    let config = SimdConfig::default();
    let ops = SimdStringOps::new(config);

    // Test equal strings
    assert_eq!(
        ops.compare_strings(b"Hello", b"Hello"),
        std::cmp::Ordering::Equal
    );

    // Test different length strings
    assert_eq!(
        ops.compare_strings(b"Hi", b"Hello"),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        ops.compare_strings(b"Hello", b"Hi"),
        std::cmp::Ordering::Greater
    );

    // Test strings with different content
    assert_eq!(
        ops.compare_strings(b"Apple", b"Banana"),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        ops.compare_strings(b"Banana", b"Apple"),
        std::cmp::Ordering::Greater
    );

    // Test empty strings
    assert_eq!(ops.compare_strings(b"", b""), std::cmp::Ordering::Equal);
}
