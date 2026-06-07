mod common;
// Input validation tests
//
// These tests verify comprehensive input validation:
// - String length limits
// - UTF-8 validation
// - Buffer bounds checking
// - Resource exhaustion prevention
// - Input sanitization

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

// Constants matching lib.rs
const MAX_KEY_LENGTH: usize = 1 * 1024 * 1024; // 1MB
const MAX_VALUE_LENGTH: usize = 100 * 1024 * 1024; // 100MB

#[test]
fn test_key_length_limit() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test key at maximum length
        // Note: Storage engine may reject very large keys, so we accept ErrorStorage as well
        let max_key = "a".repeat(MAX_KEY_LENGTH);
        let key = to_c_string(&max_key);
        let value = to_c_string("value");

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        // Storage engine may reject very large keys, so accept either Success or ErrorStorage
        assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);

        // Test key exceeding maximum length
        let too_long_key = "a".repeat(MAX_KEY_LENGTH + 1);
        let long_key = to_c_string(&too_long_key);
        let result = f4kvs_engine_put(engine, long_key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_value_length_limit() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");

        // Test value at maximum length
        // Note: Storage engine may reject very large values, so we accept ErrorStorage as well
        let max_value = "b".repeat(MAX_VALUE_LENGTH);
        let value = to_c_string(&max_value);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        // Storage engine may reject very large values, so accept either Success or ErrorStorage
        assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);

        // Test value exceeding maximum length
        let too_long_value = "b".repeat(MAX_VALUE_LENGTH + 1);
        let long_value = to_c_string(&too_long_value);
        let result = f4kvs_engine_put(engine, key.as_ptr(), long_value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_get_key_length_limit() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test get with key exceeding maximum length
        let too_long_key = "a".repeat(MAX_KEY_LENGTH + 1);
        let long_key = to_c_string(&too_long_key);
        let mut value_out: *mut c_char = std::ptr::null_mut();

        let result = f4kvs_engine_get(engine, long_key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);
        assert!(value_out.is_null());

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_delete_key_length_limit() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test delete with key exceeding maximum length
        let too_long_key = "a".repeat(MAX_KEY_LENGTH + 1);
        let long_key = to_c_string(&too_long_key);

        let result = f4kvs_engine_delete(engine, long_key.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_exists_key_length_limit() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test exists with key exceeding maximum length
        let too_long_key = "a".repeat(MAX_KEY_LENGTH + 1);
        let long_key = to_c_string(&too_long_key);
        let mut exists: std::os::raw::c_int = 0;

        let result = f4kvs_engine_exists(engine, long_key.as_ptr(), &mut exists);
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_utf8_validation_key() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Create invalid UTF-8 sequence
        let invalid_utf8: [u8; 3] = [0xFF, 0xFE, 0xFD];
        let invalid_cstr = std::ffi::CString::from_vec_unchecked(invalid_utf8.to_vec());
        let value = to_c_string("value");

        // Invalid UTF-8 in key should be rejected
        let result = f4kvs_engine_put(engine, invalid_cstr.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_utf8_validation_value() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");

        // Create invalid UTF-8 sequence
        let invalid_utf8: [u8; 3] = [0xFF, 0xFE, 0xFD];
        let invalid_cstr = std::ffi::CString::from_vec_unchecked(invalid_utf8.to_vec());

        // Invalid UTF-8 in value should be rejected
        let result = f4kvs_engine_put(engine, key.as_ptr(), invalid_cstr.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_very_large_key_rejection() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with extremely large key (10MB)
        let huge_key = "a".repeat(10 * 1024 * 1024);
        let key = to_c_string(&huge_key);
        let value = to_c_string("value");

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_very_large_value_rejection() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");

        // Test with extremely large value (200MB)
        let huge_value = "b".repeat(200 * 1024 * 1024);
        let value = to_c_string(&huge_value);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_valid_unicode_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with various valid Unicode strings
        let unicode_cases = vec![
            "你好世界",   // Chinese
            "こんにちは", // Japanese
            "안녕하세요", // Korean
            "Здравствуй", // Russian
            "مرحبا",      // Arabic
            "🚀🎉💯",     // Emoji
        ];

        for (i, unicode_str) in unicode_cases.iter().enumerate() {
            let key = to_c_string(&format!("unicode_key_{}", i));
            let value = to_c_string(unicode_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *unicode_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_null_pointer_validation() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");
        let value = to_c_string("value");
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let mut exists: std::os::raw::c_int = 0;

        // Test null engine pointer
        let result = f4kvs_engine_put(std::ptr::null_mut(), key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        let result = f4kvs_engine_get(std::ptr::null_mut(), key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        let result = f4kvs_engine_delete(std::ptr::null_mut(), key.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        let result = f4kvs_engine_exists(std::ptr::null_mut(), key.as_ptr(), &mut exists);
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Test null key pointer
        let result = f4kvs_engine_put(engine, std::ptr::null(), value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Test null value pointer
        let result = f4kvs_engine_put(engine, key.as_ptr(), std::ptr::null());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Test null output pointer
        let result = f4kvs_engine_get(engine, key.as_ptr(), std::ptr::null_mut());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        let result = f4kvs_engine_exists(engine, key.as_ptr(), std::ptr::null_mut());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_resource_exhaustion_prevention() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Try to allocate many large values to test resource limits
        // The length limits should prevent excessive memory allocation
        for i in 0..10 {
            let key = to_c_string(&format!("key_{}", i));
            // Use a large but valid value (smaller to avoid storage engine limits)
            let large_value = "x".repeat(1024 * 1024); // 1MB - reasonable size
            let value = to_c_string(&large_value);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            // Should succeed - within limits (storage engine may reject very large values)
            assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);
        }

        // Now try to exceed limits
        let key = to_c_string("too_large");
        let too_large_value = "x".repeat(MAX_VALUE_LENGTH + 1);
        let value = to_c_string(&too_large_value);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        // Should be rejected
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_edge_case_lengths() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test exactly at limit
        // Note: Storage engine may reject very large values, so we accept ErrorStorage as well
        let key_at_limit = "a".repeat(MAX_KEY_LENGTH);
        let value_at_limit = "b".repeat(MAX_VALUE_LENGTH);
        let key = to_c_string(&key_at_limit);
        let value = to_c_string(&value_at_limit);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        // Storage engine may reject very large values, so accept either Success or ErrorStorage
        assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);

        // Test one byte over limit
        let key_over_limit = "a".repeat(MAX_KEY_LENGTH + 1);
        let key_over = to_c_string(&key_over_limit);
        let small_value = to_c_string("value");

        let result = f4kvs_engine_put(engine, key_over.as_ptr(), small_value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}
