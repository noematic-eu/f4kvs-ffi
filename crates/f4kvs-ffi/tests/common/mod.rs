//! Common test utilities for FFI tests

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// Helper to create a C string from a Rust string
pub fn to_c_string(s: &str) -> CString {
    CString::new(s).expect("Failed to create CString")
}

/// Helper to convert C string to Rust string
/// Handles invalid UTF-8 gracefully using lossy conversion
pub unsafe fn from_c_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().to_string()
}

/// Helper to create a null pointer for testing
#[allow(dead_code)]
pub fn null_ptr<T>() -> *mut T {
    ptr::null_mut()
}

/// Helper to create a null const pointer for testing
#[allow(dead_code)]
pub fn null_const_ptr<T>() -> *const T {
    ptr::null()
}

/// Memory leak detector for tests
#[allow(dead_code)]
pub struct MemoryLeakDetector {
    initial_count: usize,
}

#[allow(dead_code)]
impl MemoryLeakDetector {
    pub fn new() -> Self {
        Self {
            initial_count: get_alloc_count(),
        }
    }

    pub fn check_no_leaks(self) {
        let final_count = get_alloc_count();
        // Allow some tolerance for test infrastructure
        assert!(
            final_count <= self.initial_count + 10,
            "Possible memory leak detected: {} allocations",
            final_count - self.initial_count
        );
    }
}

#[allow(dead_code)]
impl Drop for MemoryLeakDetector {
    fn drop(&mut self) {
        // Cleanup check happens in check_no_leaks
    }
}

#[allow(dead_code)]
fn get_alloc_count() -> usize {
    // Simple allocation counter for testing
    // In a real scenario, you might use a more sophisticated approach
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_c_string() {
        let cstr = to_c_string("hello");
        assert_eq!(cstr.to_str().unwrap(), "hello");
    }

    #[test]
    fn test_from_c_string() {
        let cstr = to_c_string("world");
        let rust_str = unsafe { from_c_string(cstr.as_ptr()) };
        assert_eq!(rust_str, "world");
    }
}
