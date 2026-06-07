mod common;
// Data type conversion tests
//
// These tests verify data type conversions at FFI boundaries:
// - Rust String to C string conversion
// - C string to Rust String conversion
// - Numeric type conversions
// - Boolean conversions
// - Array/slice conversions
// - Optional type handling

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

#[test]
fn test_rust_string_to_c_string() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let rust_strings = vec![
            String::from("hello"),
            String::from("world"),
            String::from("test with spaces"),
            String::from("test\nwith\nnewlines"),
        ];

        for (i, rust_str) in rust_strings.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let c_value = to_c_string(rust_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), c_value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_c_string_to_rust_string() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let c_strings = vec!["hello", "world", "test"];

        for (i, c_str) in c_strings.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(c_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let rust_str = from_c_string(value_out);
            assert_eq!(rust_str, *c_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_numeric_string_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let numeric_strings = vec![
            "0",
            "123",
            "-456",
            "789.012",
            "3.14159",
            "1e10",
            "9223372036854775807", // Max i64
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

#[test]
fn test_boolean_string_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let boolean_strings = vec!["true", "false", "True", "False", "TRUE", "FALSE"];

        for (i, bool_str) in boolean_strings.iter().enumerate() {
            let key = to_c_string(&format!("bool_key_{}", i));
            let value = to_c_string(bool_str);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *bool_str);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_empty_string_handling() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("empty_key");
        let empty_value = to_c_string("");

        let result = f4kvs_engine_put(engine, key.as_ptr(), empty_value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        assert!(!value_out.is_null());

        let retrieved = from_c_string(value_out);
        assert_eq!(retrieved, "");

        f4kvs_string_free(value_out);
        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_unicode_string_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let unicode_strings = vec![
            "你好世界",
            "こんにちは",
            "안녕하세요",
            "Здравствуй",
            "مرحبا",
            "🚀🎉💯",
        ];

        for (i, unicode_str) in unicode_strings.iter().enumerate() {
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
fn test_optional_type_handling() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test that non-existent keys return ErrorNotFound (None equivalent)
        let key = to_c_string("nonexistent_key");
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::ErrorNotFound);
        assert!(value_out.is_null());

        // Test that existing keys return Success (Some equivalent)
        let value = to_c_string("value");
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
fn test_special_characters_conversion() {
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
            "key\nwith\nnewlines",
            "key\twith\ttabs",
        ];

        for (i, special) in special_chars.iter().enumerate() {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(special);

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *special);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_very_large_string_conversion() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test with very large strings
        let large_string = "a".repeat(100000);
        let key = to_c_string("large_key");
        let value = to_c_string(&large_string);

        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        assert!(!value_out.is_null());

        let retrieved = from_c_string(value_out);
        assert_eq!(retrieved.len(), large_string.len());
        assert_eq!(retrieved, large_string);

        f4kvs_string_free(value_out);
        f4kvs_engine_free(engine);
    }
}
