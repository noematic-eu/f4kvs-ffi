mod common;
// Error handling tests
//
// These tests verify error handling at FFI boundaries:
// - Error code propagation
// - Error message conversion
// - Invalid input error handling
// - Error recovery mechanisms

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

#[test]
fn test_error_code_propagation() {
    unsafe {
        // Test that error codes are properly propagated
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Invalid operation should return appropriate error code
        let null_key = std::ptr::null();
        let value = to_c_string("value");
        let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_error_message_retrieval() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Trigger an error
        let null_key = std::ptr::null();
        let value = to_c_string("value");
        let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Retrieve error message
        let error_msg_ptr = f4kvs_get_last_error();
        eprintln!("DEBUG: error_msg_ptr: {:?}", error_msg_ptr);
        if !error_msg_ptr.is_null() {
            let error_msg = from_c_string(error_msg_ptr);
            // Debug: print the actual error message
            eprintln!("DEBUG: Retrieved error message: '{}'", error_msg);
            eprintln!(
                "DEBUG: Contains 'Invalid': {}",
                error_msg.contains("Invalid")
            );
            eprintln!("DEBUG: Contains 'null': {}", error_msg.contains("null"));
            eprintln!("DEBUG: Length: {}", error_msg.len());
            eprintln!("DEBUG: Bytes: {:?}", error_msg.as_bytes());

            // Check if the error message is actually populated
            if error_msg.is_empty() {
                eprintln!("ERROR: Error message is empty but pointer is not null!");
                eprintln!(
                    "This indicates the error was not properly set in the global error store"
                );
                panic!("Error message retrieval failed - error store not working correctly");
            }

            assert!(error_msg.contains("Invalid") || error_msg.contains("null"));
        } else {
            eprintln!("DEBUG: error_msg_ptr is null!");
            panic!("Expected error message pointer to be non-null after error");
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_not_found_error() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Try to get a non-existent key
        let key = to_c_string("nonexistent_key");
        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::ErrorNotFound);
        assert!(value_out.is_null());

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_invalid_argument_errors() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        let key = to_c_string("key");
        let value = to_c_string("value");

        // Test various invalid argument scenarios
        let test_cases = vec![
            (std::ptr::null(), value.as_ptr(), "null engine"),
            (key.as_ptr(), std::ptr::null(), "null value"),
        ];

        for (test_key, test_value, description) in test_cases {
            let result = f4kvs_engine_put(engine, test_key, test_value);
            assert_eq!(
                result,
                F4KvsResult::ErrorInvalidArgument,
                "Failed for: {}",
                description
            );
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_error_recovery() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Trigger an error
        let null_key = std::ptr::null();
        let value = to_c_string("value");
        let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Verify we can recover and perform valid operations
        let valid_key = to_c_string("valid_key");
        let result = f4kvs_engine_put(engine, valid_key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, valid_key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        assert!(!value_out.is_null());

        let retrieved = from_c_string(value_out);
        assert_eq!(retrieved, "value");

        f4kvs_string_free(value_out);
        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_error_message_consistency() {
    unsafe {
        // Test that error messages are consistent
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Trigger same error multiple times
        let null_key = std::ptr::null();
        let value = to_c_string("value");

        for _ in 0..5 {
            let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
            assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

            let error_msg_ptr = f4kvs_get_last_error();
            if !error_msg_ptr.is_null() {
                let error_msg = from_c_string(error_msg_ptr);
                assert!(!error_msg.is_empty());
            }
        }

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_result_to_string_all_codes() {
    unsafe {
        // Test all result codes can be converted to strings
        let results = vec![
            F4KvsResult::Success,
            F4KvsResult::ErrorInvalidArgument,
            F4KvsResult::ErrorNotFound,
            F4KvsResult::ErrorStorage,
            F4KvsResult::ErrorNetwork,
            F4KvsResult::ErrorTimeout,
            F4KvsResult::ErrorUnknown,
        ];

        for result in results {
            let str_ptr = f4kvs_result_to_string(result);
            assert!(!str_ptr.is_null());

            let result_str = from_c_string(str_ptr);
            assert!(!result_str.is_empty());
        }
    }
}

#[test]
fn test_concurrent_error_handling() {
    use std::thread;

    unsafe {
        let mut handles = vec![];

        // Spawn multiple threads that trigger errors
        for i in 0..10 {
            let handle = thread::spawn(move || {
                let engine = f4kvs_engine_new();
                assert!(!engine.is_null());

                let null_key = std::ptr::null();
                let value = to_c_string(&format!("value_{}", i));
                let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
                assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

                f4kvs_engine_free(engine);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

#[test]
fn test_error_after_success() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        // Perform successful operation
        let key = to_c_string("key");
        let value = to_c_string("value");
        let result = f4kvs_engine_put(engine, key.as_ptr(), value.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        // Clear error state by checking last error (should be None or previous)
        let _ = f4kvs_get_last_error();

        // Perform error operation
        let null_key = std::ptr::null();
        let result = f4kvs_engine_put(engine, null_key, value.as_ptr());
        assert_eq!(result, F4KvsResult::ErrorInvalidArgument);

        // Verify error message is set
        let error_msg_ptr = f4kvs_get_last_error();
        if !error_msg_ptr.is_null() {
            let error_msg = from_c_string(error_msg_ptr);
            assert!(!error_msg.is_empty());
        }

        f4kvs_engine_free(engine);
    }
}
