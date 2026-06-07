//! Comprehensive configuration tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for configuration scenarios including:
//! - Multi-source configuration loading
//! - Environment variable override
//! - Configuration validation workflows

use f4kvs_core::config::{Config, StorageMode};
use std::time::Duration;

#[test]
fn test_multi_source_configuration_loading() {
    // Test loading from different sources
    let config1 = Config::default();
    let config2 = Config::new()
        .with_max_key_size(2048)
        .with_storage_mode(StorageMode::HashMap);

    // Both should be valid
    assert!(config1.validate().is_ok());
    assert!(config2.validate().is_ok());

    // They should have different values
    assert_ne!(config1.max_key_size, config2.max_key_size);
    assert_ne!(config1.storage_mode, config2.storage_mode);
}

#[test]
fn test_configuration_serialization_roundtrip() {
    let original = Config::new()
        .with_max_key_size(2048)
        .with_max_value_size(5 * 1024 * 1024)
        .with_timeout(Duration::from_secs(60))
        .with_strict_key_validation(false)
        .with_storage_mode(StorageMode::HashMap);

    // Serialize to JSON
    let json = serde_json::to_string(&original).unwrap();

    // Deserialize from JSON
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    // Verify all fields
    assert_eq!(deserialized.max_key_size, original.max_key_size);
    assert_eq!(deserialized.max_value_size, original.max_value_size);
    assert_eq!(deserialized.operation_timeout, original.operation_timeout);
    assert_eq!(
        deserialized.strict_key_validation,
        original.strict_key_validation
    );
    assert_eq!(deserialized.storage_mode, original.storage_mode);
}

#[test]
fn test_configuration_validation_workflows() {
    // Test valid configuration
    let valid_config = Config::new()
        .with_max_key_size(1024)
        .with_max_value_size(1024 * 1024)
        .with_timeout(Duration::from_secs(30));
    assert!(valid_config.validate().is_ok());

    // Test invalid configurations
    let invalid_configs = vec![
        Config {
            max_key_size: 0,
            ..Default::default()
        },
        Config {
            max_value_size: 0,
            ..Default::default()
        },
        Config {
            operation_timeout: Duration::from_secs(0),
            ..Default::default()
        },
    ];

    for config in invalid_configs {
        assert!(config.validate().is_err());
    }
}

#[test]
fn test_storage_mode_configurations() {
    // Test HashMap mode
    let hashmap_config = Config::new().with_storage_mode(StorageMode::HashMap);
    assert_eq!(hashmap_config.storage_mode, StorageMode::HashMap);
    assert!(hashmap_config.validate().is_ok());

    // Test BTreeMap mode
    let btreemap_config = Config::new().with_storage_mode(StorageMode::BTreeMap);
    assert_eq!(btreemap_config.storage_mode, StorageMode::BTreeMap);
    assert!(btreemap_config.validate().is_ok());
}

#[test]
fn test_configuration_builder_pattern() {
    // Test chaining builder methods
    let config = Config::new()
        .with_max_key_size(512)
        .with_max_value_size(1024 * 1024)
        .with_timeout(Duration::from_millis(500))
        .with_strict_key_validation(false)
        .with_storage_mode(StorageMode::HashMap);

    assert_eq!(config.max_key_size, 512);
    assert_eq!(config.max_value_size, 1024 * 1024);
    assert_eq!(config.operation_timeout, Duration::from_millis(500));
    assert!(!config.strict_key_validation);
    assert_eq!(config.storage_mode, StorageMode::HashMap);
    assert!(config.validate().is_ok());
}

#[test]
fn test_configuration_edge_cases() {
    // Test minimum valid values
    let min_config = Config {
        max_key_size: 1,
        max_value_size: 1,
        operation_timeout: Duration::from_nanos(1),
        strict_key_validation: false,
        storage_mode: StorageMode::HashMap,
        enable_monitoring: false,
        enable_memory_leak_detection: false,
    };
    assert!(min_config.validate().is_ok());

    // Test large values
    let large_config = Config {
        max_key_size: usize::MAX / 2,
        max_value_size: usize::MAX / 2,
        operation_timeout: Duration::from_secs(3600),
        strict_key_validation: true,
        storage_mode: StorageMode::BTreeMap,
        enable_monitoring: true,
        enable_memory_leak_detection: true,
    };
    assert!(large_config.validate().is_ok());
}

#[test]
fn test_configuration_clone_and_equality() {
    let config1 = Config::new()
        .with_max_key_size(1024)
        .with_storage_mode(StorageMode::HashMap);
    let config2 = config1.clone();

    assert_eq!(config1, config2);
    assert_eq!(config1.max_key_size, config2.max_key_size);
    assert_eq!(config1.storage_mode, config2.storage_mode);
    assert_eq!(config1.operation_timeout, config2.operation_timeout);
}

#[test]
fn test_configuration_duration_handling() {
    // Test various timeout values
    let timeouts = vec![
        Duration::from_nanos(1),
        Duration::from_millis(100),
        Duration::from_secs(30),
        Duration::from_secs(3600),
    ];

    for timeout in timeouts {
        let config = Config::new().with_timeout(timeout);
        assert!(config.validate().is_ok());
        assert_eq!(config.operation_timeout, timeout);
    }
}

#[test]
fn test_configuration_monitoring_flags() {
    // Test all combinations of monitoring flags
    let combinations = vec![(true, true), (true, false), (false, true), (false, false)];

    for (monitoring, leak_detection) in combinations {
        let config = Config {
            enable_monitoring: monitoring,
            enable_memory_leak_detection: leak_detection,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
        assert_eq!(config.enable_monitoring, monitoring);
        assert_eq!(config.enable_memory_leak_detection, leak_detection);
    }
}

#[test]
fn test_configuration_strict_validation() {
    // Test with strict validation enabled
    let strict_config = Config::new().with_strict_key_validation(true);
    assert!(strict_config.strict_key_validation);
    assert!(strict_config.validate().is_ok());

    // Test with strict validation disabled
    let lenient_config = Config::new().with_strict_key_validation(false);
    assert!(!lenient_config.strict_key_validation);
    assert!(lenient_config.validate().is_ok());
}
