//! Comprehensive enhanced configuration tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for enhanced configuration scenarios including:
//! - Hot-reload workflows
//! - Configuration merging from multiple sources
//! - Environment variable overrides

use f4kvs_core::enhanced_config::{
    ConfigSource, EnhancedConfig, EnhancedConfigBuilder, EnhancedConfigManager,
};
use std::env;
use std::sync::Mutex;

// Static mutex to serialize environment variable access across parallel tests
static ENV_VAR_MUTEX: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn test_hot_reload_workflow() {
    // Create a config manager
    let manager = EnhancedConfigManager::new().await.unwrap();
    let initial_config = manager.get_config().await;

    // Update configuration
    let mut new_config = initial_config.clone();
    new_config.environment.name = "production".to_string();
    new_config.environment.hot_reload = true;
    manager.update_config(new_config.clone()).await.unwrap();

    // Verify update
    let updated_config = manager.get_config().await;
    assert_eq!(updated_config.environment.name, "production");
    assert!(updated_config.environment.hot_reload);
}

#[tokio::test]
async fn test_configuration_merging_from_multiple_sources() {
    // Test merging configurations
    let mut builder = EnhancedConfigBuilder::new();
    let mut file_config = EnhancedConfig::default();
    file_config.environment.name = "staging".to_string();
    file_config.performance.enable_simd = false;

    // Merge file config
    builder.merge_config(file_config, ConfigSource::File);
    let config = builder.build().unwrap();

    assert_eq!(config.environment.name, "staging");
    assert!(!config.performance.enable_simd);
}

#[tokio::test]
async fn test_environment_variable_overrides() {
    // Use mutex to serialize access to environment variables across parallel tests
    let _guard = ENV_VAR_MUTEX.lock().unwrap();

    // Save any existing values to restore later
    let old_env_name = env::var("F4KVS_ENVIRONMENT_NAME").ok();
    let old_perf_simd = env::var("F4KVS_PERFORMANCE_ENABLE_SIMD").ok();

    // Set environment variables immediately before use
    env::set_var("F4KVS_ENVIRONMENT_NAME", "production");
    env::set_var("F4KVS_PERFORMANCE_ENABLE_SIMD", "false");

    // Load from environment immediately after setting
    let builder = EnhancedConfigBuilder::new();
    let builder = builder.load_from_env().unwrap();
    let config = builder.build().unwrap();

    // Verify environment variable was applied
    assert_eq!(
        config.environment.name, "production",
        "Environment variable override failed. Got '{}' instead of 'production'.",
        config.environment.name
    );
    assert!(
        !config.performance.enable_simd,
        "Performance SIMD override failed. Got '{}' instead of 'false'.",
        config.performance.enable_simd
    );

    // Clean up - restore previous values or remove
    match old_env_name {
        Some(val) => env::set_var("F4KVS_ENVIRONMENT_NAME", val),
        None => env::remove_var("F4KVS_ENVIRONMENT_NAME"),
    }
    match old_perf_simd {
        Some(val) => env::set_var("F4KVS_PERFORMANCE_ENABLE_SIMD", val),
        None => env::remove_var("F4KVS_PERFORMANCE_ENABLE_SIMD"),
    }
}

#[tokio::test]
async fn test_configuration_source_priority() {
    // Use mutex to serialize access to environment variables across parallel tests
    let _guard = ENV_VAR_MUTEX.lock().unwrap();

    // Save any existing value to restore later
    let old_env_name = env::var("F4KVS_ENVIRONMENT_NAME").ok();

    // Test that environment variables override file config
    let builder = EnhancedConfigBuilder::new().with_source_priority(vec![
        ConfigSource::Default,
        ConfigSource::File,
        ConfigSource::Environment,
    ]);

    // Set environment variable
    env::set_var("F4KVS_ENVIRONMENT_NAME", "production");

    // Load from environment
    let builder = builder.load_from_env().unwrap();
    let config = builder.build().unwrap();

    // Environment should override defaults
    assert_eq!(config.environment.name, "production");

    // Clean up - restore previous value or remove
    match old_env_name {
        Some(val) => env::set_var("F4KVS_ENVIRONMENT_NAME", val),
        None => env::remove_var("F4KVS_ENVIRONMENT_NAME"),
    }
}

#[tokio::test]
async fn test_configuration_validation_workflow() {
    let manager = EnhancedConfigManager::new().await.unwrap();
    let mut config = manager.get_config().await;

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid config should fail
    config.environment.name = "invalid".to_string();
    assert!(manager.update_config(config).await.is_err());
}

#[tokio::test]
async fn test_environment_specific_configurations() {
    // Test production config
    let prod_config = EnhancedConfig::for_environment("production");
    assert_eq!(prod_config.environment.name, "production");
    assert!(!prod_config.environment.debug);
    assert!(!prod_config.environment.hot_reload);
    assert!(prod_config.security.enable_auth);
    assert!(prod_config.monitoring.enable_prometheus);

    // Test staging config
    let staging_config = EnhancedConfig::for_environment("staging");
    assert_eq!(staging_config.environment.name, "staging");
    assert!(!staging_config.environment.debug);
    assert!(staging_config.environment.hot_reload);
    assert!(staging_config.security.enable_auth);

    // Test development config
    let dev_config = EnhancedConfig::for_environment("development");
    assert_eq!(dev_config.environment.name, "development");
    assert!(dev_config.environment.debug);
    assert!(dev_config.environment.hot_reload);
    assert!(!dev_config.security.enable_auth);
    assert!(dev_config.monitoring.enable_profiling);
}

#[tokio::test]
async fn test_configuration_manager_concurrent_access() {
    use std::sync::Arc;

    // Use mutex to serialize access to environment variables across parallel tests
    let _guard = ENV_VAR_MUTEX.lock().unwrap();

    // Save any existing values to restore later
    let old_env_name = env::var("F4KVS_ENVIRONMENT_NAME").ok();
    let old_perf_simd = env::var("F4KVS_PERFORMANCE_ENABLE_SIMD").ok();
    let old_config_file = env::var("F4KVS_CONFIG_FILE").ok();

    // Explicitly set environment to "development" to ensure consistent test behavior
    // This avoids race conditions with other tests that might set environment variables
    env::set_var("F4KVS_ENVIRONMENT_NAME", "development");
    env::remove_var("F4KVS_PERFORMANCE_ENABLE_SIMD");
    env::remove_var("F4KVS_CONFIG_FILE");

    // Create manager - note: due to parallel test execution, environment variables
    // from other tests might affect this, so we verify consistency rather than specific values
    let manager = Arc::new(EnhancedConfigManager::new().await.unwrap());

    // Get the initial config to verify all concurrent readers get the same value
    let initial_config = manager.get_config().await;
    let expected_env_name = initial_config.environment.name.clone();

    let mut handles = Vec::new();

    // Spawn multiple concurrent readers - verify they all get the same config
    for _ in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let expected_name = expected_env_name.clone();
        let handle = tokio::spawn(async move {
            let config = manager_clone.get_config().await;
            assert_eq!(
                config.environment.name, expected_name,
                "All concurrent readers should get the same environment name"
            );
        });
        handles.push(handle);
    }

    // Wait for all readers
    for handle in handles {
        handle.await.unwrap();
    }

    // Restore previous values
    match old_env_name {
        Some(val) => env::set_var("F4KVS_ENVIRONMENT_NAME", val),
        None => env::remove_var("F4KVS_ENVIRONMENT_NAME"),
    }
    match old_perf_simd {
        Some(val) => env::set_var("F4KVS_PERFORMANCE_ENABLE_SIMD", val),
        None => env::remove_var("F4KVS_PERFORMANCE_ENABLE_SIMD"),
    }
    match old_config_file {
        Some(val) => env::set_var("F4KVS_CONFIG_FILE", val),
        None => env::remove_var("F4KVS_CONFIG_FILE"),
    }
}

#[tokio::test]
async fn test_configuration_update_validation() {
    let manager = EnhancedConfigManager::new().await.unwrap();

    // Test valid update
    let mut valid_config = manager.get_config().await;
    valid_config.environment.name = "production".to_string();
    assert!(manager.update_config(valid_config).await.is_ok());

    // Test invalid update
    let mut invalid_config = manager.get_config().await;
    invalid_config.environment.name = "invalid".to_string();
    assert!(manager.update_config(invalid_config).await.is_err());
}

#[tokio::test]
async fn test_core_config_extraction() {
    let manager = EnhancedConfigManager::new().await.unwrap();
    let core_config = manager.get_core_config().await;

    // Core config should be valid
    assert!(core_config.validate().is_ok());
    assert!(core_config.max_key_size > 0);
    assert!(core_config.max_value_size > 0);
}

#[tokio::test]
async fn test_configuration_builder_pattern() {
    let builder = EnhancedConfigBuilder::new()
        .with_env_prefix("TEST_")
        .with_source_priority(vec![ConfigSource::Default, ConfigSource::Environment]);

    let config = builder.build().unwrap();
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_configuration_serialization_roundtrip() {
    let original = EnhancedConfig::for_environment("production");

    // Serialize to JSON
    let json = serde_json::to_string(&original).unwrap();

    // Deserialize from JSON
    let deserialized: EnhancedConfig = serde_json::from_str(&json).unwrap();

    // Verify key fields
    assert_eq!(deserialized.environment.name, original.environment.name);
    assert_eq!(deserialized.environment.debug, original.environment.debug);
    assert_eq!(
        deserialized.security.enable_auth,
        original.security.enable_auth
    );
}

#[tokio::test]
async fn test_configuration_performance_settings() {
    let config = EnhancedConfig::default();

    // Test performance configuration
    assert!(config.performance.enable_simd);
    assert!(config.performance.memory_pool.enabled);
    assert!(config.performance.cache.enabled);
    assert!(config.performance.batch.enabled);

    // Test memory pool settings
    assert!(config.performance.memory_pool.block_size > 0);
    assert!(config.performance.memory_pool.max_pool_size > 0);

    // Test cache settings
    assert!(config.performance.cache.max_size > 0);
    assert!(config.performance.cache.ttl_seconds > 0);

    // Test batch settings
    assert!(config.performance.batch.max_batch_size > 0);
    assert!(config.performance.batch.timeout_ms > 0);
}

#[tokio::test]
async fn test_configuration_security_settings() {
    let prod_config = EnhancedConfig::for_environment("production");

    // Production should have security enabled
    assert!(prod_config.security.enable_auth);
    assert!(prod_config.security.enable_audit_logging);
    assert!(!prod_config.security.jwt_secret.is_empty());

    let dev_config = EnhancedConfig::for_environment("development");

    // Development should have security disabled
    assert!(!dev_config.security.enable_auth);
    assert!(!dev_config.security.enable_audit_logging);
}

#[tokio::test]
async fn test_configuration_logging_settings() {
    let prod_config = EnhancedConfig::for_environment("production");

    // Production should use structured logging
    assert_eq!(prod_config.logging.level, "info");
    assert!(prod_config.logging.structured);

    let dev_config = EnhancedConfig::for_environment("development");

    // Development should use console logging
    assert_eq!(dev_config.logging.level, "debug");
    assert!(dev_config.logging.console);
}

#[tokio::test]
async fn test_configuration_monitoring_settings() {
    let prod_config = EnhancedConfig::for_environment("production");

    // Production should have monitoring enabled
    assert!(prod_config.monitoring.enable_prometheus);
    assert!(prod_config.monitoring.enable_health_checks);
    assert!(prod_config.monitoring.prometheus_port > 0);

    let dev_config = EnhancedConfig::for_environment("development");

    // Development should have profiling enabled
    assert!(dev_config.monitoring.enable_profiling);
}
