mod common;
// Integration tests
//
// These tests verify integration with f4kvs-core and end-to-end workflows

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

#[test]
fn test_end_to_end_workflow() {
    unsafe {
        // Create engine
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Put multiple values
        let test_data = vec![("key1", "value1"), ("key2", "value2"), ("key3", "value3")];

        for (key, value) in &test_data {
            let key_cstr = to_c_string(key);
            let value_cstr = to_c_string(value);

            let result = f4kvs_engine_put(engine, key_cstr.as_ptr(), value_cstr.as_ptr());
            assert_eq!(result, F4KvsResult::Success);
        }

        // Get all values
        for (key, expected_value) in &test_data {
            let key_cstr = to_c_string(key);
            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key_cstr.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *expected_value);

            f4kvs_string_free(value_out);
        }

        // Check existence
        for (key, _) in &test_data {
            let key_cstr = to_c_string(key);
            let mut exists: std::os::raw::c_int = 0;
            let result = f4kvs_engine_exists(engine, key_cstr.as_ptr(), &mut exists);
            assert_eq!(result, F4KvsResult::Success);
            assert_eq!(exists, 1);
        }

        // Delete values
        for (key, _) in &test_data {
            let key_cstr = to_c_string(key);
            let result = f4kvs_engine_delete(engine, key_cstr.as_ptr());
            assert_eq!(result, F4KvsResult::Success);
        }

        // Verify deletion
        for (key, _) in &test_data {
            let key_cstr = to_c_string(key);
            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key_cstr.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::ErrorNotFound);
            assert!(value_out.is_null());
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_large_dataset_workflow() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Store large dataset
        let num_items = 1000;
        for i in 0..num_items {
            let key = to_c_string(&format!("key_{}", i));
            let value = to_c_string(&format!("value_{}", i));

            let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
            assert_eq!(result, F4KvsResult::Success);
        }

        // Retrieve all items
        for i in 0..num_items {
            let key = to_c_string(&format!("key_{}", i));
            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, format!("value_{}", i));

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_overwrite_workflow() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");
        let value1 = to_c_string("value1");

        // Put initial value
        let result = f4kvs_engine_put(engine, key.as_ptr(), value1.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        // Verify initial value
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        let retrieved1 = from_c_string(value_out);
        assert_eq!(retrieved1, "value1");
        f4kvs_string_free(value_out);

        // Overwrite with new value
        let value2 = to_c_string("value2");
        let result = f4kvs_engine_put(engine, key.as_ptr(), value2.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        // Verify new value
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        let retrieved2 = from_c_string(value_out);
        assert_eq!(retrieved2, "value2");
        f4kvs_string_free(value_out);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_error_recovery_workflow() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Trigger an error
        let null_key = std::ptr::null();
        let value = to_c_string("value");
        let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Recover and perform valid operations
        let key = to_c_string("recovery_key");
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
fn test_multiple_engines() {
    unsafe {
        // Create multiple engines
        let engine1 = f4kvs_engine_new();
        let engine2 = f4kvs_engine_new();
        assert!(!engine1.is_null());
        assert!(!engine2.is_null());

        // Store different values in each engine
        let key1 = to_c_string("key");
        let value1 = to_c_string("value1");
        let result = f4kvs_engine_put(engine1, key1.as_ptr(), value1.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let key2 = to_c_string("key");
        let value2 = to_c_string("value2");
        let result = f4kvs_engine_put(engine2, key2.as_ptr(), value2.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        // Verify engines are independent
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine1, key1.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        let retrieved1 = from_c_string(value_out);
        assert_eq!(retrieved1, "value1");
        f4kvs_string_free(value_out);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine2, key2.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        let retrieved2 = from_c_string(value_out);
        assert_eq!(retrieved2, "value2");
        f4kvs_string_free(value_out);

        f4kvs_engine_free(engine1);
        f4kvs_engine_free(engine2);
    }
}

#[test]
fn test_complex_value_types() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Test various value types (as strings)
        let test_cases = vec![
            ("string_key", "simple string"),
            ("number_key", "12345"),
            ("boolean_key", "true"),
            ("json_key", r#"{"name":"test","value":42}"#),
            ("multiline_key", "line1\nline2\nline3"),
        ];

        for (key, value) in &test_cases {
            let key_cstr = to_c_string(key);
            let value_cstr = to_c_string(value);

            let result = f4kvs_engine_put(engine, key_cstr.as_ptr(), value_cstr.as_ptr());
            assert_eq!(result, F4KvsResult::Success);

            let mut value_out: *mut c_char = std::ptr::null_mut();
            let result = f4kvs_engine_get(engine, key_cstr.as_ptr(), &mut value_out);
            assert_eq!(result, F4KvsResult::Success);
            assert!(!value_out.is_null());

            let retrieved = from_c_string(value_out);
            assert_eq!(retrieved, *value);

            f4kvs_string_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_batch_put_bytes_workflow() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let keys = [
            to_c_string("chunk:doc-0001:chunk-000001"),
            to_c_string("chunk:doc-0001:chunk-000002"),
            to_c_string("chunk:doc-0002:chunk-000001"),
        ];
        let values: [&[u8]; 3] = [b"payload-a", b"payload-b", b"payload-c"];
        let key_ptrs: Vec<*const c_char> = keys.iter().map(|k| k.as_ptr()).collect();
        let value_ptrs: Vec<*const u8> = values.iter().map(|v| v.as_ptr()).collect();
        let value_lens: Vec<usize> = values.iter().map(|v| v.len()).collect();

        let result = f4kvs_engine_batch_put_bytes(
            engine,
            key_ptrs.as_ptr(),
            value_ptrs.as_ptr(),
            value_lens.as_ptr(),
            keys.len(),
        );
        assert_eq!(result, F4KvsResult::Success);

        for (i, key) in keys.iter().enumerate() {
            let mut value_out: *mut u8 = std::ptr::null_mut();
            let mut value_len: usize = 0;
            let result = f4kvs_engine_get_bytes(engine, key.as_ptr(), &mut value_out, &mut value_len);
            assert_eq!(result, F4KvsResult::Success);
            let retrieved = std::slice::from_raw_parts(value_out, value_len);
            assert_eq!(retrieved, values[i]);
            f4kvs_bytes_free(value_out);
        }

        f4kvs_engine_free(engine);
    }
}
