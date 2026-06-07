//! Configuration for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::Duration;

/// Storage backend mode for memory storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageMode {
    /// Use HashMap for O(1) average-case lookups (faster for pure KV operations)
    HashMap,
    /// Use BTreeMap for O(log n) lookups but ordered keys (better for range queries)
    BTreeMap,
}

/// Core configuration for F4KVS
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Maximum key size in bytes (default: 1KB)
    #[serde(default = "default_max_key_size")]
    pub max_key_size: usize,

    /// Maximum value size in bytes (default: 10MB)
    #[serde(default = "default_max_value_size")]
    pub max_value_size: usize,

    /// Operation timeout in seconds (default: 30s)
    #[serde(
        deserialize_with = "deserialize_duration_from_secs",
        serialize_with = "serialize_duration_as_secs",
        default = "default_operation_timeout"
    )]
    pub operation_timeout: Duration,

    /// Enable strict key validation (default: true)
    #[serde(default = "default_strict_key_validation")]
    pub strict_key_validation: bool,

    /// Storage mode for memory storage (default: BTreeMap for compatibility)
    #[serde(default = "default_storage_mode")]
    pub storage_mode: StorageMode,

    /// Enable monitoring hooks (default: true)
    /// WARNING: Disabling monitoring may cause deadlocks in stress tests
    #[serde(default = "default_enable_monitoring")]
    pub enable_monitoring: bool,

    /// Enable memory leak detection (default: true)
    /// WARNING: Memory leak detector may cause async deadlocks
    #[serde(default = "default_enable_memory_leak_detection")]
    pub enable_memory_leak_detection: bool,
}

/// Deserialize Duration from seconds (for TOML compatibility)
fn deserialize_duration_from_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}

/// Serialize Duration as seconds (for TOML compatibility)
fn serialize_duration_as_secs<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(duration.as_secs())
}

/// Default values for TOML deserialization
fn default_max_key_size() -> usize {
    1024
}
fn default_max_value_size() -> usize {
    10 * 1024 * 1024
}
fn default_operation_timeout() -> Duration {
    Duration::from_secs(30)
}
fn default_strict_key_validation() -> bool {
    true
}
fn default_storage_mode() -> StorageMode {
    StorageMode::BTreeMap
}
fn default_enable_monitoring() -> bool {
    true
}
fn default_enable_memory_leak_detection() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_key_size: 1024,               // 1KB
            max_value_size: 10 * 1024 * 1024, // 10MB
            operation_timeout: Duration::from_secs(30),
            strict_key_validation: true,
            storage_mode: StorageMode::BTreeMap, // Default to BTreeMap for compatibility
            enable_monitoring: true,
            enable_memory_leak_detection: true,
        }
    }
}

impl Config {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum key size
    pub fn with_max_key_size(mut self, size: usize) -> Self {
        self.max_key_size = size;
        self
    }

    /// Set maximum value size
    pub fn with_max_value_size(mut self, size: usize) -> Self {
        self.max_value_size = size;
        self
    }

    /// Set operation timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    /// Enable/disable strict key validation
    pub fn with_strict_key_validation(mut self, strict: bool) -> Self {
        self.strict_key_validation = strict;
        self
    }

    /// Set storage mode for memory storage
    pub fn with_storage_mode(mut self, mode: StorageMode) -> Self {
        self.storage_mode = mode;
        self
    }

    /// Validate configuration values
    pub fn validate(&self) -> crate::Result<()> {
        use crate::F4KvsError;

        if self.max_key_size == 0 {
            return Err(F4KvsError::config("max_key_size must be > 0"));
        }

        if self.max_value_size == 0 {
            return Err(F4KvsError::config("max_value_size must be > 0"));
        }

        if self.operation_timeout.is_zero() {
            return Err(F4KvsError::config("operation_timeout must be > 0"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.max_key_size, 1024);
        assert_eq!(config.max_value_size, 10 * 1024 * 1024);
        assert!(config.strict_key_validation);
        config.validate().unwrap();
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .with_max_key_size(2048)
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.max_key_size, 2048);
        assert_eq!(config.operation_timeout, Duration::from_secs(60));
        config.validate().unwrap();
    }

    #[test]
    fn test_config_validation() {
        let config = Config {
            max_key_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_storage_mode_config() {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);

        assert_eq!(config.storage_mode, StorageMode::HashMap);
        config.validate().unwrap();
    }

    #[test]
    fn test_storage_mode_enum() {
        // Test StorageMode enum variants
        assert_eq!(StorageMode::HashMap, StorageMode::HashMap);
        assert_eq!(StorageMode::BTreeMap, StorageMode::BTreeMap);
        assert_ne!(StorageMode::HashMap, StorageMode::BTreeMap);

        // Test Debug, Clone, Copy, PartialEq, Eq
        let mode = StorageMode::HashMap;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("HashMap"));

        let cloned = mode;
        assert_eq!(mode, cloned);

        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::new()
            .with_max_key_size(2048)
            .with_max_value_size(5 * 1024 * 1024)
            .with_timeout(Duration::from_secs(60))
            .with_strict_key_validation(false)
            .with_storage_mode(StorageMode::HashMap);

        // Test serialization
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("2048"));
        assert!(serialized.contains("HashMap"));

        // Test deserialization
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.max_key_size, 2048);
        assert_eq!(deserialized.storage_mode, StorageMode::HashMap);
        assert!(!deserialized.strict_key_validation);
    }

    #[test]
    fn test_config_validation_comprehensive() {
        // Test valid config
        let valid_config = Config::new();
        assert!(valid_config.validate().is_ok());

        // Test invalid max_key_size
        let invalid_key_size = Config {
            max_key_size: 0,
            ..Default::default()
        };
        assert!(invalid_key_size.validate().is_err());
        let error = invalid_key_size.validate().unwrap_err();
        assert!(
            matches!(error, crate::F4KvsError::Config { message } if message.contains("max_key_size"))
        );

        // Test invalid max_value_size
        let invalid_value_size = Config {
            max_value_size: 0,
            ..Default::default()
        };
        assert!(invalid_value_size.validate().is_err());
        let error = invalid_value_size.validate().unwrap_err();
        assert!(
            matches!(error, crate::F4KvsError::Config { message } if message.contains("max_value_size"))
        );

        // Test invalid operation_timeout
        let invalid_timeout = Config {
            operation_timeout: Duration::from_secs(0),
            ..Default::default()
        };
        assert!(invalid_timeout.validate().is_err());
        let error = invalid_timeout.validate().unwrap_err();
        assert!(
            matches!(error, crate::F4KvsError::Config { message } if message.contains("operation_timeout"))
        );
    }

    #[test]
    fn test_config_builder_chain() {
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

        // Should still be valid
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_edge_cases() {
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
            max_key_size: usize::MAX,
            max_value_size: usize::MAX,
            operation_timeout: Duration::from_secs(3600),
            strict_key_validation: true,
            storage_mode: StorageMode::BTreeMap,
            enable_monitoring: true,
            enable_memory_leak_detection: true,
        };
        assert!(large_config.validate().is_ok());
    }

    #[test]
    fn test_config_clone_and_equality() {
        let config1 = Config::new().with_max_key_size(1024);
        let config2 = config1.clone();

        assert_eq!(config1, config2);
        assert_eq!(config1.max_key_size, config2.max_key_size);
        assert_eq!(config1.storage_mode, config2.storage_mode);
    }

    #[test]
    fn test_default_value_validation() {
        let config = Config::default();

        // Verify all default values
        assert_eq!(config.max_key_size, 1024);
        assert_eq!(config.max_value_size, 10 * 1024 * 1024);
        assert_eq!(config.operation_timeout, Duration::from_secs(30));
        assert!(config.strict_key_validation);
        assert_eq!(config.storage_mode, StorageMode::BTreeMap);
        assert!(config.enable_monitoring);
        assert!(config.enable_memory_leak_detection);

        // Default config should be valid
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
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

        // Verify all fields match
        assert_eq!(deserialized.max_key_size, original.max_key_size);
        assert_eq!(deserialized.max_value_size, original.max_value_size);
        assert_eq!(deserialized.operation_timeout, original.operation_timeout);
        assert_eq!(
            deserialized.strict_key_validation,
            original.strict_key_validation
        );
        assert_eq!(deserialized.storage_mode, original.storage_mode);
        assert_eq!(deserialized.enable_monitoring, original.enable_monitoring);
        assert_eq!(
            deserialized.enable_memory_leak_detection,
            original.enable_memory_leak_detection
        );
    }

    #[test]
    fn test_invalid_configuration_detection() {
        // Test zero max_key_size
        let config1 = Config {
            max_key_size: 0,
            ..Default::default()
        };
        assert!(config1.validate().is_err());

        // Test zero max_value_size
        let config2 = Config {
            max_value_size: 0,
            ..Default::default()
        };
        assert!(config2.validate().is_err());

        // Test zero timeout
        let config3 = Config {
            operation_timeout: Duration::from_secs(0),
            ..Default::default()
        };
        assert!(config3.validate().is_err());
    }

    #[test]
    fn test_storage_mode_selection() {
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
    fn test_config_with_all_builder_methods() {
        let config = Config::new()
            .with_max_key_size(512)
            .with_max_value_size(2 * 1024 * 1024)
            .with_timeout(Duration::from_millis(1000))
            .with_strict_key_validation(true)
            .with_storage_mode(StorageMode::HashMap);

        assert_eq!(config.max_key_size, 512);
        assert_eq!(config.max_value_size, 2 * 1024 * 1024);
        assert_eq!(config.operation_timeout, Duration::from_millis(1000));
        assert!(config.strict_key_validation);
        assert_eq!(config.storage_mode, StorageMode::HashMap);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization_formats() {
        let config = Config::new()
            .with_max_key_size(1024)
            .with_storage_mode(StorageMode::HashMap);

        // Test JSON serialization
        let json = serde_json::to_string(&config).unwrap();
        let from_json: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.max_key_size, config.max_key_size);
        assert_eq!(from_json.storage_mode, config.storage_mode);

        // Test that Duration serializes correctly
        assert!(json.contains("operation_timeout"));
    }

    #[test]
    fn test_config_duration_serialization() {
        let config = Config::new().with_timeout(Duration::from_secs(120));

        // Serialize
        let json = serde_json::to_string(&config).unwrap();

        // Deserialize
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.operation_timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_config_with_extreme_values() {
        // Test with very large values
        let large_config = Config {
            max_key_size: usize::MAX / 2,
            max_value_size: usize::MAX / 2,
            operation_timeout: Duration::from_secs(u64::MAX / 1000),
            ..Default::default()
        };
        assert!(large_config.validate().is_ok());

        // Test with very small but valid values
        let small_config = Config {
            max_key_size: 1,
            max_value_size: 1,
            operation_timeout: Duration::from_nanos(1),
            ..Default::default()
        };
        assert!(small_config.validate().is_ok());
    }

    #[test]
    fn test_config_monitoring_flags() {
        // Test with monitoring enabled
        let config_with_monitoring = Config {
            enable_monitoring: true,
            enable_memory_leak_detection: true,
            ..Default::default()
        };
        assert!(config_with_monitoring.validate().is_ok());

        // Test with monitoring disabled
        let config_without_monitoring = Config {
            enable_monitoring: false,
            enable_memory_leak_detection: false,
            ..Default::default()
        };
        assert!(config_without_monitoring.validate().is_ok());
    }
}
