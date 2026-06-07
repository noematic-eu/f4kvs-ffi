//! Safe SIMD Operations
//!
//! This module provides safe alternatives to unsafe SIMD operations
//! using safe abstractions and optimized scalar implementations.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

/// Safe SIMD configuration
#[derive(Debug, Clone)]
pub struct SafeSimdConfig {
    /// Enable SIMD operations
    pub enabled: bool,
    /// Preferred SIMD width
    pub preferred_width: usize,
}

impl Default for SafeSimdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preferred_width: 16, // Default to 128-bit SIMD
        }
    }
}

/// Safe SIMD operations manager
pub struct SafeSimdManager {
    config: SafeSimdConfig,
}

impl Default for SafeSimdManager {
    fn default() -> Self {
        Self::new(SafeSimdConfig::default())
    }
}

impl SafeSimdManager {
    /// Create a new safe SIMD manager
    pub fn new(config: SafeSimdConfig) -> Self {
        Self { config }
    }

    /// Find the first occurrence of a byte using safe operations
    pub fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        if !self.config.enabled {
            return self.find_byte_scalar(haystack, needle);
        }

        // Use optimized scalar implementation for safe vectorized search
        self.find_byte_optimized(haystack, needle)
    }

    /// Optimized scalar implementation of byte search
    fn find_byte_optimized(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        // Use memchr for optimized byte search when available
        #[cfg(feature = "memchr")]
        {
            use memchr::memchr;
            memchr(needle, haystack)
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.find_byte_scalar(haystack, needle)
        }
    }

    /// Scalar fallback implementation
    fn find_byte_scalar(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        haystack.iter().position(|&b| b == needle)
    }

    /// Find the first occurrence of a substring using safe operations
    pub fn find_substring(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if !self.config.enabled || needle.is_empty() {
            return self.find_substring_scalar(haystack, needle);
        }

        if needle.len() == 1 {
            return self.find_byte(haystack, needle[0]);
        }

        // Use optimized substring search
        self.find_substring_optimized(haystack, needle)
    }

    /// Optimized substring search implementation
    fn find_substring_optimized(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        // Use memchr for optimized substring search when available
        #[cfg(feature = "memchr")]
        {
            use memchr::memmem;
            memmem::find(haystack, needle)
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.find_substring_scalar(haystack, needle)
        }
    }

    /// Scalar fallback implementation for substring search
    fn find_substring_scalar(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }

        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// Compare two byte slices using safe operations
    pub fn compare_bytes(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        if !self.config.enabled {
            return self.compare_bytes_scalar(a, b);
        }

        // Compare lengths first
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }

        // Use optimized byte comparison
        self.compare_bytes_optimized(a, b)
    }

    /// Optimized byte comparison implementation
    fn compare_bytes_optimized(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        // Use memcmp for optimized comparison when available
        #[cfg(feature = "memchr")]
        {
            use std::cmp::Ordering;
            // SAFETY: a and b are valid slices with length a.len(). libc::memcmp is safe to call
            // with valid pointers and length. The function compares up to a.len() bytes, which
            // is safe since both slices have at least that length (we checked a.len() == b.len()
            // earlier). The pointers are obtained from valid Rust slices, so they're non-null
            // and properly aligned.
            match unsafe { libc::memcmp(a.as_ptr() as *const _, b.as_ptr() as *const _, a.len()) } {
                x if x < 0 => Ordering::Less,
                x if x > 0 => Ordering::Greater,
                _ => Ordering::Equal,
            }
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.compare_bytes_scalar(a, b)
        }
    }

    /// Scalar fallback implementation for byte comparison
    fn compare_bytes_scalar(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        a.cmp(b)
    }

    /// Bulk copy using safe operations
    pub fn bulk_copy(&self, src: &[u8], dst: &mut [u8]) -> Result<(), &'static str> {
        if !self.config.enabled {
            return self.bulk_copy_scalar(src, dst);
        }

        if src.len() != dst.len() {
            return Err("Source and destination lengths must match");
        }

        self.bulk_copy_optimized(src, dst)
    }

    /// Optimized bulk copy implementation
    fn bulk_copy_optimized(&self, src: &[u8], dst: &mut [u8]) -> Result<(), &'static str> {
        // Use memcpy for optimized copy when available
        #[cfg(feature = "memchr")]
        {
            // SAFETY: src and dst are valid slices with matching lengths (validated earlier).
            // libc::memcpy is safe to call with valid pointers and length. The function copies
            // src.len() bytes from src to dst, which is safe since dst.len() >= src.len().
            // The pointers are obtained from valid Rust slices, so they're non-null and properly
            // aligned. The memory regions don't overlap (Rust's borrow checker ensures this).
            unsafe {
                libc::memcpy(
                    dst.as_mut_ptr() as *mut _,
                    src.as_ptr() as *const _,
                    src.len(),
                );
            }
            Ok(())
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.bulk_copy_scalar(src, dst)
        }
    }

    /// Scalar fallback implementation for bulk copy
    fn bulk_copy_scalar(&self, src: &[u8], dst: &mut [u8]) -> Result<(), &'static str> {
        if src.len() != dst.len() {
            return Err("Source and destination lengths must match");
        }

        dst.copy_from_slice(src);
        Ok(())
    }

    /// Bulk zero using safe operations
    pub fn bulk_zero(&self, dst: &mut [u8]) {
        if !self.config.enabled {
            self.bulk_zero_scalar(dst);
            return;
        }

        self.bulk_zero_optimized(dst);
    }

    /// Optimized bulk zero implementation
    fn bulk_zero_optimized(&self, dst: &mut [u8]) {
        // Use memset for optimized zero when available
        #[cfg(feature = "memchr")]
        {
            // SAFETY: dst is a valid mutable slice. libc::memset is safe to call with a valid
            // pointer and length. The function sets dst.len() bytes to 0, which is safe since
            // the pointer is obtained from a valid Rust slice, so it's non-null and properly
            // aligned. The memory region is valid for writing.
            unsafe {
                libc::memset(dst.as_mut_ptr() as *mut _, 0, dst.len());
            }
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.bulk_zero_scalar(dst);
        }
    }

    /// Scalar fallback implementation for bulk zero
    fn bulk_zero_scalar(&self, dst: &mut [u8]) {
        dst.fill(0);
    }

    /// Safe memory comparison
    pub fn memory_compare(&self, a: &[u8], b: &[u8]) -> bool {
        if !self.config.enabled {
            return self.memory_compare_scalar(a, b);
        }

        self.memory_compare_optimized(a, b)
    }

    /// Optimized memory comparison implementation
    fn memory_compare_optimized(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        // Use memcmp for optimized comparison when available
        #[cfg(feature = "memchr")]
        {
            // SAFETY: a and b are valid slices with matching lengths (checked earlier).
            // libc::memcmp is safe to call with valid pointers and length. The function compares
            // up to a.len() bytes, which is safe since both slices have at least that length.
            // The pointers are obtained from valid Rust slices, so they're non-null and properly
            // aligned.
            unsafe { libc::memcmp(a.as_ptr() as *const _, b.as_ptr() as *const _, a.len()) == 0 }
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.memory_compare_scalar(a, b)
        }
    }

    /// Scalar fallback implementation for memory comparison
    fn memory_compare_scalar(&self, a: &[u8], b: &[u8]) -> bool {
        a == b
    }

    /// Safe memory search
    pub fn memory_search(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if !self.config.enabled {
            return self.memory_search_scalar(haystack, needle);
        }

        self.memory_search_optimized(haystack, needle)
    }

    /// Optimized memory search implementation
    fn memory_search_optimized(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        // Use memmem for optimized search when available
        #[cfg(feature = "memchr")]
        {
            use memchr::memmem;
            memmem::find(haystack, needle)
        }

        #[cfg(not(feature = "memchr"))]
        {
            self.memory_search_scalar(haystack, needle)
        }
    }

    /// Scalar fallback implementation for memory search
    fn memory_search_scalar(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }

        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_byte() {
        let manager = SafeSimdManager::default();
        let data = b"hello world";

        assert_eq!(manager.find_byte(data, b'h'), Some(0));
        assert_eq!(manager.find_byte(data, b'o'), Some(4));
        assert_eq!(manager.find_byte(data, b'z'), None);
    }

    #[test]
    fn test_find_substring() {
        let manager = SafeSimdManager::default();
        let data = b"hello world";

        assert_eq!(manager.find_substring(data, b"hello"), Some(0));
        assert_eq!(manager.find_substring(data, b"world"), Some(6));
        assert_eq!(manager.find_substring(data, b"xyz"), None);
    }

    #[test]
    fn test_compare_bytes() {
        let manager = SafeSimdManager::default();

        assert_eq!(
            manager.compare_bytes(b"abc", b"abc"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            manager.compare_bytes(b"abc", b"abd"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            manager.compare_bytes(b"abd", b"abc"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_bulk_copy() {
        let manager = SafeSimdManager::default();
        let src = b"hello world";
        let mut dst = [0u8; 11];

        assert!(manager.bulk_copy(src, &mut dst).is_ok());
        assert_eq!(src, &dst);
    }

    #[test]
    fn test_bulk_zero() {
        let manager = SafeSimdManager::default();
        let mut data = [1u8; 16];

        manager.bulk_zero(&mut data);
        assert_eq!(data, [0u8; 16]);
    }

    #[test]
    fn test_memory_compare() {
        let manager = SafeSimdManager::default();

        assert!(manager.memory_compare(b"hello", b"hello"));
        assert!(!manager.memory_compare(b"hello", b"world"));
        assert!(!manager.memory_compare(b"hello", b"hell"));
    }

    #[test]
    fn test_memory_search() {
        let manager = SafeSimdManager::default();
        let data = b"hello world";

        assert_eq!(manager.memory_search(data, b"hello"), Some(0));
        assert_eq!(manager.memory_search(data, b"world"), Some(6));
        assert_eq!(manager.memory_search(data, b"xyz"), None);
    }

    #[test]
    fn test_safety_wrapper_validation() {
        let manager = SafeSimdManager::default();

        // Test with empty inputs
        assert_eq!(manager.find_byte(&[], b'a'), None);
        assert_eq!(manager.find_substring(&[], b"test"), None);
        assert_eq!(manager.memory_search(&[], b"test"), None);

        // Test with single byte
        assert_eq!(manager.find_byte(b"a", b'a'), Some(0));
        assert_eq!(manager.find_byte(b"a", b'b'), None);
    }

    #[test]
    fn test_error_handling_in_unsafe_contexts() {
        let manager = SafeSimdManager::default();

        // Test bulk_copy with length mismatch
        let src = b"hello";
        let mut dst_small = [0u8; 3];
        assert!(manager.bulk_copy(src, &mut dst_small).is_err());

        let mut dst_large = [0u8; 10];
        assert!(manager.bulk_copy(src, &mut dst_large).is_err());

        // Test with matching lengths
        let mut dst_correct = [0u8; 5];
        assert!(manager.bulk_copy(src, &mut dst_correct).is_ok());
    }

    #[test]
    fn test_alignment_guarantees() {
        let manager = SafeSimdManager::default();

        // Test with various alignments
        for offset in 0..16 {
            let total = 128 + offset;
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

    #[test]
    fn test_bounds_checking_enforcement() {
        let manager = SafeSimdManager::default();

        // Test with various buffer sizes
        let sizes = vec![1, 16, 32, 64, 128, 256, 1024];

        for size in sizes {
            let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let mut dst = vec![0u8; size];

            // Should work safely
            assert!(manager.bulk_copy(&src, &mut dst).is_ok());
            assert_eq!(src, dst);

            // Test bulk_zero
            manager.bulk_zero(&mut dst);
            assert!(dst.iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn test_safe_simd_disabled() {
        let config = SafeSimdConfig {
            enabled: false,
            preferred_width: 16,
        };
        let manager = SafeSimdManager::new(config);

        // Operations should still work with scalar fallback
        let data = b"hello world";
        assert_eq!(manager.find_byte(data, b'o'), Some(4));
        assert_eq!(manager.find_substring(data, b"world"), Some(6));

        let src = vec![1u8; 64];
        let mut dst = vec![0u8; 64];
        assert!(manager.bulk_copy(&src, &mut dst).is_ok());
        assert_eq!(src, dst);
    }

    #[test]
    fn test_concurrent_safe_simd_operations() {
        use std::sync::Arc;

        let manager = Arc::new(SafeSimdManager::default());

        // Spawn multiple concurrent operations
        let mut handles = Vec::new();
        for i in 0..20 {
            let manager_clone = Arc::clone(&manager);
            let handle = std::thread::spawn(move || {
                let data = format!("test_data_{}", i).into_bytes();
                let needle = b'_';
                manager_clone.find_byte(&data, needle)
            });
            handles.push(handle);
        }

        // Wait for all operations
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_error_propagation_testing() {
        let manager = SafeSimdManager::default();

        // Test error propagation for bulk_copy
        let src = b"test";
        let mut dst_wrong = [0u8; 2];
        let result = manager.bulk_copy(src, &mut dst_wrong);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Source and destination lengths must match"
        );
    }

    #[test]
    fn test_large_data_operations() {
        let manager = SafeSimdManager::default();

        // Test with large data
        let large_size = 10000;
        let large_data: Vec<u8> = (0..large_size).map(|i| (i % 256) as u8).collect();
        let mut large_dst = vec![0u8; large_size];

        // Should work safely
        assert!(manager.bulk_copy(&large_data, &mut large_dst).is_ok());
        assert_eq!(large_data, large_dst);

        // Test bulk_zero
        manager.bulk_zero(&mut large_dst);
        assert!(large_dst.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_compare_bytes_edge_cases() {
        let manager = SafeSimdManager::default();

        // Test with different lengths
        assert_eq!(
            manager.compare_bytes(b"short", b"longer"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            manager.compare_bytes(b"longer", b"short"),
            std::cmp::Ordering::Greater
        );

        // Test with empty slices
        assert_eq!(manager.compare_bytes(&[], &[]), std::cmp::Ordering::Equal);

        // Test with single bytes
        assert_eq!(manager.compare_bytes(b"a", b"a"), std::cmp::Ordering::Equal);
        assert_eq!(manager.compare_bytes(b"a", b"b"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_memory_compare_edge_cases() {
        let manager = SafeSimdManager::default();

        // Test with empty slices
        assert!(manager.memory_compare(&[], &[]));
        assert!(!manager.memory_compare(&[], &[1]));

        // Test with different lengths
        assert!(!manager.memory_compare(b"hello", b"hell"));
        assert!(!manager.memory_compare(b"hell", b"hello"));

        // Test with identical data
        let data = vec![1u8; 1000];
        assert!(manager.memory_compare(&data, &data));
    }

    #[test]
    fn test_memory_search_edge_cases() {
        let manager = SafeSimdManager::default();

        // Test with empty needle
        assert_eq!(manager.memory_search(b"hello", &[]), Some(0));

        // Test with needle longer than haystack
        assert_eq!(manager.memory_search(b"hello", b"hello world"), None);

        // Test with needle at start
        assert_eq!(manager.memory_search(b"hello world", b"hello"), Some(0));

        // Test with needle at end
        assert_eq!(manager.memory_search(b"hello world", b"world"), Some(6));

        // Test with needle in middle
        assert_eq!(manager.memory_search(b"hello world", b"lo wo"), Some(3));
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
                enabled: false,
                preferred_width: 16,
            },
        ];

        for config in configs {
            let manager = SafeSimdManager::new(config);

            // Operations should work with any config
            let data = b"test data";
            assert_eq!(manager.find_byte(data, b't'), Some(0));
            assert_eq!(manager.find_substring(data, b"test"), Some(0));

            let src = vec![1u8; 64];
            let mut dst = vec![0u8; 64];
            assert!(manager.bulk_copy(&src, &mut dst).is_ok());
        }
    }

    #[test]
    fn test_safe_simd_default_config() {
        let config = SafeSimdConfig::default();
        assert!(config.enabled);
        assert!(config.preferred_width >= 16);

        let manager = SafeSimdManager::default();
        let data = b"test";
        assert_eq!(manager.find_byte(data, b't'), Some(0));
    }
}
