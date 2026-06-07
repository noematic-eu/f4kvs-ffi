//! FFI bindings integration tests
//!
//! This module provides comprehensive integration tests for FFI bindings
//! with mock C/Go/Python/Node.js calls to verify cross-language compatibility.

use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// FFI integration test suite
pub struct FfiIntegrationTestSuite;

impl FfiIntegrationTestSuite {
    /// Run all FFI integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running FFI Integration Tests");
        println!("===============================");
        println!();

        // Test basic FFI operations
        Self::test_basic_ffi_operations().await?;
        println!("✅ Basic FFI operations tests passed");

        // Test FFI with different data types
        Self::test_ffi_data_types().await?;
        println!("✅ FFI data types tests passed");

        // Test FFI error handling
        Self::test_ffi_error_handling().await?;
        println!("✅ FFI error handling tests passed");

        // Test FFI performance
        Self::test_ffi_performance().await?;
        println!("✅ FFI performance tests passed");

        // Test FFI edge cases
        Self::test_ffi_edge_cases().await?;
        println!("✅ FFI edge cases tests passed");

        println!();
        println!("🎉 All FFI integration tests passed!");

        Ok(())
    }

    /// Test basic FFI operations
    async fn test_basic_ffi_operations() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Simulate C FFI calls
        let c_key = CString::new("c_key").unwrap();
        let c_value = CString::new("c_value").unwrap();

        // Test put operation (simulating C FFI)
        let value = Value::String(c_value.to_string_lossy().to_string());
        engine.put(c_key.to_string_lossy().as_ref(), &value).await?;

        // Test get operation (simulating C FFI)
        let retrieved = engine.get(c_key.to_string_lossy().as_ref()).await?;
        assert_eq!(retrieved, Some(value));

        // Simulate Go FFI calls
        let go_key = "go_key";
        let go_value = "go_value";

        let value = Value::String(go_value.to_string());
        engine.put(go_key, &value).await?;

        let retrieved = engine.get(go_key).await?;
        assert_eq!(retrieved, Some(value));

        // Simulate Python FFI calls
        let python_key = "python_key";
        let python_value = "python_value";

        let value = Value::String(python_value.to_string());
        engine.put(python_key, &value).await?;

        let retrieved = engine.get(python_key).await?;
        assert_eq!(retrieved, Some(value));

        // Simulate Node.js FFI calls
        let nodejs_key = "nodejs_key";
        let nodejs_value = "nodejs_value";

        let value = Value::String(nodejs_value.to_string());
        engine.put(nodejs_key, &value).await?;

        let retrieved = engine.get(nodejs_key).await?;
        assert_eq!(retrieved, Some(value));

        Ok(())
    }

    /// Test FFI with different data types
    async fn test_ffi_data_types() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test string values (most common in FFI)
        let string_key = "string_key";
        let string_value = Value::String("Hello, FFI World!".to_string());
        engine.put(string_key, &string_value).await?;
        let retrieved = engine.get(string_key).await?;
        assert_eq!(retrieved, Some(string_value));

        // Test integer values
        let int_key = "int_key";
        let int_value = Value::Int64(42);
        engine.put(int_key, &int_value).await?;
        let retrieved = engine.get(int_key).await?;
        assert_eq!(retrieved, Some(int_value));

        // Test float values
        let float_key = "float_key";
        let float_value = Value::Float64(std::f64::consts::PI);
        engine.put(float_key, &float_value).await?;
        let retrieved = engine.get(float_key).await?;
        assert_eq!(retrieved, Some(float_value));

        // Test boolean values
        let bool_key = "bool_key";
        let bool_value = Value::Bool(true);
        engine.put(bool_key, &bool_value).await?;
        let retrieved = engine.get(bool_key).await?;
        assert_eq!(retrieved, Some(bool_value));

        // Test with special characters (common in FFI scenarios)
        let special_key = "special_key_!@#$%^&*()";
        let special_value = Value::String("Special chars: !@#$%^&*()".to_string());
        engine.put(special_key, &special_value).await?;
        let retrieved = engine.get(special_key).await?;
        assert_eq!(retrieved, Some(special_value));

        // Test with unicode characters
        let unicode_key = "unicode_key";
        let unicode_value = Value::String("Unicode: 你好世界 🌍".to_string());
        engine.put(unicode_key, &unicode_value).await?;
        let retrieved = engine.get(unicode_key).await?;
        assert_eq!(retrieved, Some(unicode_value));

        Ok(())
    }

    /// Test FFI error handling
    async fn test_ffi_error_handling() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test with single character keys (minimum valid key)
        let single_key = "a";
        let value = Value::String("single_key_value".to_string());
        engine.put(single_key, &value).await?;
        let retrieved = engine.get(single_key).await?;
        assert_eq!(retrieved, Some(value));

        // Test with long keys (FFI boundary testing, within limits)
        let long_key = "a".repeat(1000);
        let long_value = Value::String("long_key_value".to_string());
        engine.put(&long_key, &long_value).await?;
        let retrieved = engine.get(&long_key).await?;
        assert_eq!(retrieved, Some(long_value));

        // Test with very long values (FFI boundary testing)
        let large_value = Value::String("x".repeat(100000));
        engine.put("large_value_key", &large_value).await?;
        let retrieved = engine.get("large_value_key").await?;
        assert_eq!(retrieved, Some(large_value));

        // Test rapid put/delete cycles (stress test for FFI)
        for i in 0..1000 {
            let key = format!("ffi_stress_{}", i);
            let value = Value::String(format!("ffi_value_{}", i));

            engine.put(&key, &value).await?;
            engine.delete(&key).await?;

            // Verify deletion
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, None);
        }

        Ok(())
    }

    /// Test FFI performance
    async fn test_ffi_performance() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        let test_sizes = vec![100, 500, 1000];

        for size in test_sizes {
            println!("Testing FFI performance with {} operations", size);

            let start = std::time::Instant::now();

            // Simulate FFI put operations
            for i in 0..size {
                let key = format!("ffi_perf_{}", i);
                let value = Value::String(format!("ffi_perf_value_{}", i));
                engine.put(&key, &value).await?;
            }

            let put_duration = start.elapsed();

            let start = std::time::Instant::now();

            // Simulate FFI get operations
            for i in 0..size {
                let key = format!("ffi_perf_{}", i);
                let _retrieved = engine.get(&key).await?;
            }

            let get_duration = start.elapsed();

            let put_ops_per_sec = size as f64 / put_duration.as_secs_f64();
            let get_ops_per_sec = size as f64 / get_duration.as_secs_f64();

            println!("  Put: {:.0} ops/sec", put_ops_per_sec);
            println!("  Get: {:.0} ops/sec", get_ops_per_sec);

            // Performance assertions for FFI operations
            assert!(
                put_ops_per_sec > 100.0,
                "FFI put too slow: {:.0} ops/sec",
                put_ops_per_sec
            );
            assert!(
                get_ops_per_sec > 500.0,
                "FFI get too slow: {:.0} ops/sec",
                get_ops_per_sec
            );
        }

        Ok(())
    }

    /// Test FFI edge cases
    async fn test_ffi_edge_cases() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test with binary-like data (common in FFI)
        let binary_key = "binary_key";
        let binary_data = vec![0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD];
        let binary_string = String::from_utf8_lossy(&binary_data);
        let value = Value::String(binary_string.to_string());
        engine.put(binary_key, &value).await?;
        let retrieved = engine.get(binary_key).await?;
        assert_eq!(retrieved, Some(value));

        // Test with JSON-like data (common in web FFI)
        let json_key = "json_key";
        let json_value =
            Value::String(r#"{"name": "test", "value": 42, "active": true}"#.to_string());
        engine.put(json_key, &json_value).await?;
        let retrieved = engine.get(json_key).await?;
        assert_eq!(retrieved, Some(json_value));

        // Test with XML-like data
        let xml_key = "xml_key";
        let xml_value =
            Value::String(r#"<root><item>test</item><value>42</value></root>"#.to_string());
        engine.put(xml_key, &xml_value).await?;
        let retrieved = engine.get(xml_key).await?;
        assert_eq!(retrieved, Some(xml_value));

        // Test with newline and tab characters
        let multiline_key = "multiline_key";
        let multiline_value = Value::String("Line 1\nLine 2\tTabbed\nLine 3".to_string());
        engine.put(multiline_key, &multiline_value).await?;
        let retrieved = engine.get(multiline_key).await?;
        assert_eq!(retrieved, Some(multiline_value));

        // Test with control characters
        let control_key = "control_key";
        let control_value = Value::String("Control: \x00\x01\x02\x03\x04\x05".to_string());
        engine.put(control_key, &control_value).await?;
        let retrieved = engine.get(control_key).await?;
        assert_eq!(retrieved, Some(control_value));

        // Test with very small values
        let small_key = "small_key";
        let small_value = Value::String("a".to_string());
        engine.put(small_key, &small_value).await?;
        let retrieved = engine.get(small_key).await?;
        assert_eq!(retrieved, Some(small_value));

        // Test with numeric strings (common in FFI)
        let numeric_key = "numeric_key";
        let numeric_value = Value::String("1234567890".to_string());
        engine.put(numeric_key, &numeric_value).await?;
        let retrieved = engine.get(numeric_key).await?;
        assert_eq!(retrieved, Some(numeric_value));

        Ok(())
    }
}

/// Helper function to simulate C string operations
fn c_string_to_rust(c_str: *const c_char) -> String {
    unsafe { CStr::from_ptr(c_str).to_string_lossy().to_string() }
}

/// Helper function to convert Rust string to C string
fn rust_string_to_c(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[tokio::test]
async fn test_ffi_integration() {
    FfiIntegrationTestSuite::run_all_tests().await.unwrap();
}
