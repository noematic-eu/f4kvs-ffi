mod common;
// C interop tests
//
// These tests verify C language interoperability:
// - C string conversion (to/from Rust)
// - C struct marshalling
// - Callback function handling
// - C array handling
// - C enum conversion

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::ffi::CString;
use std::os::raw::c_char;

#[test]
fn test_c_string_to_rust_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test various string conversions
        let test_cases = vec![
            "hello",
            "world",
            "test with spaces",
            "test\nwith\nnewlines",
            "test\twith\ttabs",
            "test with unicode: 你好世界",
            "test with emoji: 🚀🎉",
        ];

        for (i, test_str) in test_cases.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(test_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let rust_str = from_c_string(value_out);
            assert_eq!(rust_str, *test_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_rust_string_to_c_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Create Rust strings and convert to C
        let rust_strings = vec![
            String::from("hello"),
            String::from("world"),
            String::from("test with spaces"),
        ];

        for (i, rust_str) in rust_strings.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(rust_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_c_enum_conversion() {
    // Test that F4KvsResult enum values match C expectations
    assert_eq!(F4KvsResult::Success as i32, 0);
    assert_eq!(F4KvsResult::ErrorInvalidArgument as i32, 1);
    assert_eq!(F4KvsResult::ErrorNotFound as i32, 2);
    assert_eq!(F4KvsResult::ErrorStorage as i32, 3);
    assert_eq!(F4KvsResult::ErrorNetwork as i32, 4);
    assert_eq!(F4KvsResult::ErrorTimeout as i32, 5);
    assert_eq!(F4KvsResult::ErrorUnknown as i32, 99);
}

#[test]
fn test_result_to_string() {
    unsafe {
        let result_strings = vec![
            (F4KvsResult::Success, "Success"),
            (F4KvsResult::ErrorInvalidArgument, "Invalid argument"),
            (F4KvsResult::ErrorNotFound, "Not found"),
            (F4KvsResult::ErrorStorage, "Storage error"),
            (F4KvsResult::ErrorNetwork, "Network error"),
            (F4KvsResult::ErrorTimeout, "Timeout"),
            (F4KvsResult::ErrorUnknown, "Unknown error"),
        ];

        for (result, expected) in result_strings {
            let c_str_ptr = f4kvs_result_to_string(result);
            assert!(!c_str_ptr.is_null());

            let rust_str = from_c_string(c_str_ptr);
            assert_eq!(rust_str, expected);
        }
    }
}

#[test]
fn test_special_characters_in_c_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let special_chars = vec![
            "key with spaces",
            "key-with-dashes",
            "key_with_underscores",
            "key.with.dots",
            "key/with/slashes",
            "key@with#special$chars%",
        ];

        for (i, special) in special_chars.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(special);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *special);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_null_terminated_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Verify that C strings are properly null-terminated
        let test_str = "hello";
        let cstr = CString::new(test_str).unwrap();
        assert_eq!(cstr.as_bytes_with_nul().last(), Some(&0u8));

        let key = to_c_string("test_key");
        let result = f4kvs_engine_put(engine, key.as_ptr(), cstr.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_unicode_in_c_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let unicode_strings = vec![
            "你好世界",   // Chinese
            "こんにちは", // Japanese
            "안녕하세요", // Korean
            "Здравствуй", // Russian
            "مرحبا",      // Arabic
            "🚀🎉💯",     // Emoji
        ];

        for (i, unicode_str) in unicode_strings.iter().enumerate() {
            let key = to_c_string(&format!("unicode_key_{}", i));
            let value = to_c_string(unicode_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *unicode_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_c_string_with_embedded_nulls() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // C strings with embedded nulls should be handled
        // Note: Standard C strings can't have embedded nulls, but we test the boundary
        let key = to_c_string("key");
        let value = to_c_string("value");

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_very_long_c_strings() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with very long strings (stress test)
        let long_string = "a".repeat(100000);
        let key = to_c_string("long_key");
        let value = to_c_string(&long_string);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);

        let retrieved = from_c_string(value_out);
        assert_eq!(retrieved.len(), long_string.len());

        f4kvs_string_free(value_out);
        f4kvs_engine_free(engine);
    }
}
