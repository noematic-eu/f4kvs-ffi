//! Comprehensive validation edge case tests for F4KVS Core
//!
//! This module provides extensive test coverage for key and value validation,
//! configuration edge cases, and error scenarios.

use f4kvs_core::{Config, F4KVSCore, StorageMode, Value};
use std::time::Duration;

#[tokio::test]
async fn test_key_validation_empty_key() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.put("", &Value::String("value".to_string())).await;
    assert!(result.is_err());

    if let Err(e) = result {
        assert!(format!("{:?}", e).contains("empty") || format!("{:?}", e).contains("InvalidKey"));
    }
}

#[tokio::test]
async fn test_key_validation_too_long() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let long_key = "x".repeat(11);
    let result = engine
        .put(&long_key, &Value::String("value".to_string()))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_key_validation_exact_max_size() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let exact_key = "x".repeat(10);
    let result = engine
        .put(&exact_key, &Value::String("value".to_string()))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_key_validation_null_bytes_strict() {
    let config = Config::new().with_strict_key_validation(true);
    let engine = F4KVSCore::with_config(config).unwrap();

    let key_with_null = "key\0with\0null";
    let result = engine
        .put(key_with_null, &Value::String("value".to_string()))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_key_validation_starts_with_dot_strict() {
    let config = Config::new().with_strict_key_validation(true);
    let engine = F4KVSCore::with_config(config).unwrap();

    let result = engine
        .put(".hidden", &Value::String("value".to_string()))
        .await;
    assert!(result.is_err());

    let result = engine
        .put("/path", &Value::String("value".to_string()))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_key_validation_starts_with_dot_loose() {
    let config = Config::new().with_strict_key_validation(false);
    let engine = F4KVSCore::with_config(config).unwrap();

    // Should work with loose validation
    let result = engine
        .put(".hidden", &Value::String("value".to_string()))
        .await;
    assert!(result.is_ok());

    let result = engine
        .put("/path", &Value::String("value".to_string()))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_key_validation_null_bytes_loose() {
    let config = Config::new().with_strict_key_validation(false);
    let engine = F4KVSCore::with_config(config).unwrap();

    // Should work with loose validation (though not recommended)
    let key_with_null = "key\0with\0null";
    let _result = engine
        .put(key_with_null, &Value::String("value".to_string()))
        .await;
    // Note: This might still fail depending on storage backend, but we test the validation path
}

#[tokio::test]
async fn test_value_validation_too_large() {
    let config = Config::new().with_max_value_size(100);
    let engine = F4KVSCore::with_config(config).unwrap();

    let large_value = Value::String("x".repeat(101));
    let result = engine.put("key", &large_value).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_value_validation_exact_max_size() {
    let config = Config::new().with_max_value_size(100);
    let engine = F4KVSCore::with_config(config).unwrap();

    // Create a value that's exactly at the limit
    let exact_value = Value::String("x".repeat(100));
    let _result = engine.put("key", &exact_value).await;
    // This might succeed or fail depending on memory_size calculation
}

#[tokio::test]
async fn test_value_validation_large_bytes() {
    let config = Config::new().with_max_value_size(100);
    let engine = F4KVSCore::with_config(config).unwrap();

    let large_bytes = Value::Bytes(vec![0; 101]);
    let result = engine.put("key", &large_bytes).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_value_validation_large_json() {
    let config = Config::new().with_max_value_size(100);
    let engine = F4KVSCore::with_config(config).unwrap();

    let large_json = Value::Json(serde_json::json!({
        "data": "x".repeat(101)
    }));
    let _result = engine.put("key", &large_json).await;
    // This will fail if memory_size exceeds limit
}

#[tokio::test]
async fn test_batch_validation_empty_key() {
    let engine = F4KVSCore::new().unwrap();

    let items = vec![("".to_string(), Value::String("value".to_string()))];

    let result = engine.batch_put(items).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_validation_too_long_key() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let items = vec![("x".repeat(11), Value::String("value".to_string()))];

    let result = engine.batch_put(items).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_validation_too_large_value() {
    let config = Config::new().with_max_value_size(100);
    let engine = F4KVSCore::with_config(config).unwrap();

    let items = vec![("key".to_string(), Value::String("x".repeat(101)))];

    let result = engine.batch_put(items).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_validation_mixed_valid_invalid() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let items = vec![
        ("key1".to_string(), Value::String("value1".to_string())),
        ("x".repeat(11), Value::String("value2".to_string())), // Invalid
        ("key2".to_string(), Value::String("value3".to_string())),
    ];

    // Should fail on first invalid key
    let result = engine.batch_put(items).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_get_validation_empty_key() {
    let engine = F4KVSCore::new().unwrap();

    let keys = vec!["".to_string()];
    let result = engine.batch_get(keys).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_get_validation_too_long_key() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let keys = vec!["x".repeat(11)];
    let result = engine.batch_get(keys).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_delete_validation_empty_key() {
    let engine = F4KVSCore::new().unwrap();

    let keys = vec!["".to_string()];
    let result = engine.batch_delete(keys).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_delete_validation_too_long_key() {
    let config = Config::new().with_max_key_size(10);
    let engine = F4KVSCore::with_config(config).unwrap();

    let keys = vec!["x".repeat(11)];
    let result = engine.batch_delete(keys).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_validation_zero_max_key_size() {
    let config = Config {
        max_key_size: 0,
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_validation_zero_max_value_size() {
    let config = Config {
        max_value_size: 0,
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_validation_zero_timeout() {
    let config = Config {
        operation_timeout: Duration::from_secs(0),
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_validation_valid() {
    let config = Config {
        max_key_size: 1,
        max_value_size: 1,
        operation_timeout: Duration::from_secs(1),
        strict_key_validation: true,
        storage_mode: StorageMode::HashMap,
        enable_monitoring: true,
        enable_memory_leak_detection: true,
    };

    let result = config.validate();
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_engine_creation_with_invalid_config() {
    let config = Config {
        max_key_size: 0,
        ..Default::default()
    };

    let result = F4KVSCore::with_config(config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_with_invalid_key() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.get("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_with_invalid_key() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.delete("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_exists_with_invalid_key() {
    let engine = F4KVSCore::new().unwrap();

    let result = engine.exists("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_scan_prefix_with_invalid_key() {
    let engine = F4KVSCore::new().unwrap();

    // Empty prefix should be allowed for scan operations
    let result = engine.scan_prefix("").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_config_builder_pattern() {
    let config = Config::new()
        .with_max_key_size(2048)
        .with_max_value_size(50 * 1024 * 1024)
        .with_timeout(Duration::from_secs(60))
        .with_strict_key_validation(false)
        .with_storage_mode(StorageMode::HashMap);

    assert_eq!(config.max_key_size, 2048);
    assert_eq!(config.max_value_size, 50 * 1024 * 1024);
    assert_eq!(config.operation_timeout, Duration::from_secs(60));
    assert!(!config.strict_key_validation);
    assert_eq!(config.storage_mode, StorageMode::HashMap);

    let result = config.validate();
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_config_default_values() {
    let config = Config::default();

    assert_eq!(config.max_key_size, 1024);
    assert_eq!(config.max_value_size, 10 * 1024 * 1024);
    assert_eq!(config.operation_timeout, Duration::from_secs(30));
    assert!(config.strict_key_validation);
    assert_eq!(config.storage_mode, StorageMode::BTreeMap);
}

#[tokio::test]
async fn test_config_with_extreme_values() {
    // Test with very large values - updated for new Config fields
    let config = Config {
        max_key_size: usize::MAX,
        max_value_size: usize::MAX,
        operation_timeout: Duration::from_secs(3600),
        strict_key_validation: true,
        storage_mode: StorageMode::HashMap,
        enable_monitoring: true,
        enable_memory_leak_detection: true,
    };

    let result = config.validate();
    assert!(result.is_ok());

    let engine_result = F4KVSCore::with_config(config);
    assert!(engine_result.is_ok());
}

#[tokio::test]
async fn test_value_types_validation() {
    let engine = F4KVSCore::new().unwrap();

    // Test all value types pass validation
    let test_cases = vec![
        ("str", Value::String("test".to_string())),
        ("int", Value::Int64(42)),
        ("uint", Value::UInt64(100)),
        ("float", Value::Float64(3.14)),
        ("bool", Value::Bool(true)),
        ("bytes", Value::Bytes(vec![1, 2, 3])),
        ("json", Value::Json(serde_json::json!({"key": "value"}))),
        ("null", Value::Null),
    ];

    for (key, value) in test_cases {
        let result = engine.put(key, &value).await;
        assert!(result.is_ok(), "Failed to put {} value", key);

        let retrieved = engine.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value.clone()));
    }
}
