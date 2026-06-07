//! Configuration and feature flag tests for F4KVS Core
//!
//! These tests verify configuration validation, feature flag combinations,
//! and runtime configuration changes.

use f4kvs_core::*;

/// Test invalid configuration handling
#[tokio::test]
async fn test_invalid_configuration_handling() {
    // Test with zero max_key_size
    let invalid_config = Config {
        max_key_size: 0,
        max_value_size: 1024,
        strict_key_validation: true,
        ..Default::default()
    };

    let result = invalid_config.validate();
    assert!(result.is_err());

    // Test with zero max_value_size
    let invalid_config = Config {
        max_key_size: 1024,
        max_value_size: 0,
        strict_key_validation: true,
        ..Default::default()
    };

    let result = invalid_config.validate();
    assert!(result.is_err());
}

/// Test configuration with different storage modes
#[tokio::test]
async fn test_configuration_storage_modes() {
    // Test with HashMap storage mode
    let config_hashmap = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        strict_key_validation: true,
        storage_mode: StorageMode::HashMap,
        ..Default::default()
    };

    let engine_hashmap = F4KVSCore::with_config(config_hashmap).unwrap();
    assert_eq!(engine_hashmap.config().storage_mode, StorageMode::HashMap);

    // Test with BTreeMap storage mode
    let config_btree = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        strict_key_validation: true,
        storage_mode: StorageMode::BTreeMap,
        ..Default::default()
    };

    let engine_btree = F4KVSCore::with_config(config_btree).unwrap();
    assert_eq!(engine_btree.config().storage_mode, StorageMode::BTreeMap);
}

/// Test configuration with different key validation settings
#[tokio::test]
async fn test_configuration_key_validation() {
    // Test with strict key validation enabled
    let config_strict = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        strict_key_validation: true,
        ..Default::default()
    };

    let engine_strict = F4KVSCore::with_config(config_strict).unwrap();
    assert!(engine_strict.config().strict_key_validation);

    // Test with strict key validation disabled
    let config_loose = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        strict_key_validation: false,
        ..Default::default()
    };

    let engine_loose = F4KVSCore::with_config(config_loose).unwrap();
    assert!(!engine_loose.config().strict_key_validation);
}

/// Test configuration with different size limits
#[tokio::test]
async fn test_configuration_size_limits() {
    // Test with small size limits
    let config_small = Config {
        max_key_size: 10,
        max_value_size: 100,
        strict_key_validation: true,
        ..Default::default()
    };

    let engine_small = F4KVSCore::with_config(config_small).unwrap();
    assert_eq!(engine_small.config().max_key_size, 10);
    assert_eq!(engine_small.config().max_value_size, 100);

    // Test with large size limits
    let config_large = Config {
        max_key_size: 1024 * 1024,          // 1MB
        max_value_size: 1024 * 1024 * 1024, // 1GB
        strict_key_validation: true,
        ..Default::default()
    };

    let engine_large = F4KVSCore::with_config(config_large).unwrap();
    assert_eq!(engine_large.config().max_key_size, 1024 * 1024);
    assert_eq!(engine_large.config().max_value_size, 1024 * 1024 * 1024);
}

/// Test configuration with different timeouts
#[tokio::test]
async fn test_configuration_timeouts() {
    use std::time::Duration;

    // Test with short timeout
    let config_short = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        operation_timeout: Duration::from_millis(100),
        strict_key_validation: true,
        ..Default::default()
    };

    let engine_short = F4KVSCore::with_config(config_short).unwrap();
    assert_eq!(
        engine_short.config().operation_timeout,
        Duration::from_millis(100)
    );

    // Test with long timeout
    let config_long = Config {
        max_key_size: 1024,
        max_value_size: 1024 * 1024,
        operation_timeout: Duration::from_secs(300), // 5 minutes
        strict_key_validation: true,
        ..Default::default()
    };

    let engine_long = F4KVSCore::with_config(config_long).unwrap();
    assert_eq!(
        engine_long.config().operation_timeout,
        Duration::from_secs(300)
    );
}

/// Test configuration validation with edge values
#[tokio::test]
async fn test_configuration_edge_values() {
    // Test with maximum allowed values
    let config_max = Config {
        max_key_size: usize::MAX,
        max_value_size: usize::MAX,
        strict_key_validation: true,
        ..Default::default()
    };

    let result = F4KVSCore::with_config(config_max);
    assert!(result.is_ok());

    // Test with minimum allowed values
    let config_min = Config {
        max_key_size: 1,
        max_value_size: 1,
        strict_key_validation: false,
        ..Default::default()
    };

    let result = F4KVSCore::with_config(config_min);
    assert!(result.is_ok());
}

/// Test configuration persistence
#[tokio::test]
async fn test_configuration_persistence() {
    let engine = F4KVSCore::new().unwrap();
    let initial_config = engine.config();

    // Verify configuration is accessible
    assert_eq!(initial_config.max_key_size, 1024);
    assert_eq!(initial_config.max_value_size, 10 * 1024 * 1024);
    assert!(initial_config.strict_key_validation);
    assert_eq!(initial_config.storage_mode, StorageMode::BTreeMap);
}

/// Test configuration with different combinations
#[tokio::test]
async fn test_configuration_combinations() {
    // Test combination 1: HashMap + strict validation + small limits
    let config1 = Config {
        max_key_size: 100,
        max_value_size: 1000,
        strict_key_validation: true,
        storage_mode: StorageMode::HashMap,
        ..Default::default()
    };

    let engine1 = F4KVSCore::with_config(config1).unwrap();
    assert_eq!(engine1.config().storage_mode, StorageMode::HashMap);
    assert!(engine1.config().strict_key_validation);
    assert_eq!(engine1.config().max_key_size, 100);

    // Test combination 2: BTreeMap + loose validation + large limits
    let config2 = Config {
        max_key_size: 10000,
        max_value_size: 100000,
        strict_key_validation: false,
        storage_mode: StorageMode::BTreeMap,
        ..Default::default()
    };

    let engine2 = F4KVSCore::with_config(config2).unwrap();
    assert_eq!(engine2.config().storage_mode, StorageMode::BTreeMap);
    assert!(!engine2.config().strict_key_validation);
    assert_eq!(engine2.config().max_key_size, 10000);
}

/// Test configuration with extreme values
#[tokio::test]
async fn test_configuration_extreme_values() {
    // Test with very large values
    let config = Config {
        max_key_size: 1024 * 1024,          // 1MB key
        max_value_size: 1024 * 1024 * 1024, // 1GB value
        strict_key_validation: true,
        ..Default::default()
    };

    let result = F4KVSCore::with_config(config);
    assert!(result.is_ok());

    // Test with very small values
    let config = Config {
        max_key_size: 1,
        max_value_size: 1,
        strict_key_validation: true,
        ..Default::default()
    };

    let result = F4KVSCore::with_config(config);
    assert!(result.is_ok());
}
