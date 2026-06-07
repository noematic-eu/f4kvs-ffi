mod common;
// Edge case tests
//
// These tests verify handling of edge cases:
// - Empty/null inputs
// - Maximum size inputs
// - Invalid UTF-8 sequences
// - Special characters
// - Unicode characters
// - Very large data structures

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

#[test]
fn test_empty_inputs() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Empty key and value - core may reject empty keys
        let empty_key = to_c_string("");
        let empty_value = to_c_string("");

        let result = f4kvs_engine_put(engine, empty_key.as_ptr(), empty_value.as_ptr());
        // Empty keys may be rejected by the storage engine
        assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);

        // If put succeeded, test get; otherwise test that get also fails appropriately
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let get_result = f4kvs_engine_get(engine, empty_key.as_ptr(), &mut value_out);

        if result == F4KvsResult::Success {
            assert_eq!(get_result, F4KvsResult::Success);
            assert!(!value_out.is_null());
            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, "");
            f4kvs_string_free(value_out);
        } else {
            // If put failed, get should also fail
            assert!(
                get_result == F4KvsResult::ErrorNotFound || get_result == F4KvsResult::ErrorStorage
            );
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_maximum_size_inputs() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with very large strings (approaching practical limits)
        let max_key = "a".repeat(10000);
        let max_value = "b".repeat(100000);

        let key = to_c_string(&max_key);
        let value = to_c_string(&max_value);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        // Large values may be rejected by the storage engine
        assert!(result == F4KvsResult::Success || result == F4KvsResult::ErrorStorage);

        // If put succeeded, test get; otherwise test that get also fails appropriately
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let get_result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);

        if result == F4KvsResult::Success {
            assert_eq!(get_result, F4KvsResult::Success);
            assert!(!value_out.is_null());
            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved.len(), max_value.len());
            f4kvs_string_free(value_out);
        } else {
            // If put failed, get should also fail
            assert!(
                get_result == F4KvsResult::ErrorNotFound || get_result == F4KvsResult::ErrorStorage
            );
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_special_characters() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let special_chars = vec![
            "!@#$%^&*()",
            "[]{}|\\:;\"'<>?,./",
            "`~-_=+",
            "\n\t\r",
            "\x00\x01\x02",
        ];

        for (i, special) in special_chars.iter().enumerate() {
            let key = to_c_string(&format!("special_key_{}", i));
            // Note: Some special characters may not be valid in C strings
            if let Ok(value) = std::ffi::CString::new(*special) {
                let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
                assert_eq!(result, F4KvsResult::Success);
            }
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_unicode_characters() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let unicode_cases = vec![
            "你好世界",   // Chinese
            "こんにちは", // Japanese Hiragana
            "안녕하세요", // Korean
            "Здравствуй", // Russian Cyrillic
            "مرحبا",      // Arabic
            "שלום",       // Hebrew
            "🚀🎉💯",     // Emoji
            "αβγδε",      // Greek
        ];

        for (i, unicode) in unicode_cases.iter().enumerate() {
            let key = to_c_string(&format!("unicode_key_{}", i));
            let value = to_c_string(unicode);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *unicode);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_very_long_key_names() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with very long key names
        let long_key = "a".repeat(1000);
        let value = to_c_string("value");

        let key = to_c_string(&long_key);
        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        assert!(!value_out.is_null());

        let retrieved = from_c_string(value_out);
        assert_eq!(retrieved, "value");

        f4kvs_string_free(value_out);
        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_repeated_operations() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");
        let value = to_c_string("value");

        // Perform the same operation many times
        for _ in 0..1000 {
            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());
            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_rapid_create_destroy() {
    // Test rapid engine creation and destruction
    for _ in 0..100 {
        unsafe {
            let engine = f4kvs_engine_new();
            assert!(!engine.is_null());

            let key = to_c_string("key");
            let value = to_c_string("value");
            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            f4kvs_engine_free(engine);
        }
    }
}

#[test]
fn test_mixed_case_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let mixed_cases = vec![
            "HelloWorld",
            "HELLO_WORLD",
            "hello_world",
            "Hello_World",
            "hELLO_wORLD",
        ];

        for (i, mixed) in mixed_cases.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(mixed);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *mixed);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_numeric_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let numeric_strings = vec![
            "0",
            "1",
            "-1",
            "1234567890",
            "3.14159",
            "1e10",
            "0xDEADBEEF",
        ];

        for (i, num_str) in numeric_strings.iter().enumerate() {
            let key = to_c_string(&format!("num_key_{}", i));
            let value = to_c_string(num_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *num_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}
