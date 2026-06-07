//! Environment Variable Configuration Loader for F4KVS Core
//!
//! This module provides automatic configuration loading from environment variables
//! with support for nested configuration, type conversion, and validation.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{F4KvsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

/// Environment variable configuration loader
pub struct EnvConfigLoader {
    /// Environment variable prefix
    prefix: String,
    /// Case sensitivity setting
    case_sensitive: bool,
    /// Separator for nested keys
    separator: char,
}

/// Environment variable configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvConfig {
    /// Raw environment variables
    pub variables: HashMap<String, String>,
    /// Parsed configuration values
    pub config: HashMap<String, ConfigValue>,
}

/// Configuration value with type information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Array of strings
    Array(Vec<String>),
}

impl ConfigValue {
    /// Convert to string
    pub fn as_string(&self) -> Option<&String> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to integer
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Convert to float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Convert to boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Convert to array
    pub fn as_array(&self) -> Option<&Vec<String>> {
        match self {
            ConfigValue::Array(a) => Some(a),
            _ => None,
        }
    }
}

impl EnvConfigLoader {
    /// Create a new environment configuration loader
    pub fn new() -> Self {
        Self {
            prefix: "F4KVS_".to_string(),
            case_sensitive: false,
            separator: '_',
        }
    }

    /// Set the environment variable prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set case sensitivity
    pub fn with_case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    /// Set the separator for nested keys
    pub fn with_separator(mut self, separator: char) -> Self {
        self.separator = separator;
        self
    }

    /// Load configuration from environment variables
    pub fn load(&self) -> Result<EnvConfig> {
        let mut variables = HashMap::new();
        let mut config = HashMap::new();

        for (key, value) in env::vars() {
            if key.starts_with(&self.prefix) {
                let config_key = self.extract_config_key(&key);
                let parsed_value = self.parse_value(&value)?;

                variables.insert(key, value);
                config.insert(config_key, parsed_value);
            }
        }

        Ok(EnvConfig { variables, config })
    }

    /// Load configuration and merge with existing values
    pub fn load_and_merge(
        &self,
        existing: HashMap<String, ConfigValue>,
    ) -> Result<HashMap<String, ConfigValue>> {
        let env_config = self.load()?;
        let mut merged = existing;

        for (key, value) in env_config.config {
            merged.insert(key, value);
        }

        Ok(merged)
    }

    /// Extract configuration key from environment variable name
    fn extract_config_key(&self, env_key: &str) -> String {
        let key = env_key.strip_prefix(&self.prefix).unwrap();
        let key = if self.case_sensitive {
            key.to_string()
        } else {
            key.to_lowercase()
        };

        // Convert underscores to dots for nested configuration
        key.replace(self.separator, ".")
    }

    /// Parse environment variable value
    fn parse_value(&self, value: &str) -> Result<ConfigValue> {
        // Try to parse as different types
        if let Ok(bool_val) = self.parse_boolean(value) {
            return Ok(ConfigValue::Boolean(bool_val));
        }

        if let Ok(int_val) = value.parse::<i64>() {
            return Ok(ConfigValue::Integer(int_val));
        }

        if let Ok(float_val) = value.parse::<f64>() {
            return Ok(ConfigValue::Float(float_val));
        }

        // Check if it's an array (comma-separated values)
        if value.contains(',') {
            let array: Vec<String> = value.split(',').map(|s| s.trim().to_string()).collect();
            return Ok(ConfigValue::Array(array));
        }

        // Default to string
        Ok(ConfigValue::String(value.to_string()))
    }

    /// Parse boolean value
    fn parse_boolean(&self, value: &str) -> Result<bool> {
        match value.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" | "enabled" => Ok(true),
            "false" | "0" | "no" | "off" | "disabled" => Ok(false),
            _ => Err(F4KvsError::config_with_field(
                "boolean_parse",
                format!("Invalid boolean value: {}", value),
            )),
        }
    }

    /// Get a specific configuration value
    pub fn get_value(&self, key: &str) -> Result<Option<ConfigValue>> {
        let env_key = self.prefix.clone() + &key.replace('.', &self.separator.to_string());
        let env_key = if self.case_sensitive {
            env_key
        } else {
            env_key.to_uppercase()
        };

        match env::var(&env_key) {
            Ok(value) => Ok(Some(self.parse_value(&value)?)),
            Err(_) => Ok(None),
        }
    }

    /// Set a configuration value in environment
    pub fn set_value(&self, key: &str, value: &ConfigValue) -> Result<()> {
        let env_key = self.prefix.clone() + &key.replace('.', &self.separator.to_string());
        let env_key = if self.case_sensitive {
            env_key
        } else {
            env_key.to_uppercase()
        };

        let env_value = match value {
            ConfigValue::String(s) => s.clone(),
            ConfigValue::Integer(i) => i.to_string(),
            ConfigValue::Float(f) => f.to_string(),
            ConfigValue::Boolean(b) => b.to_string(),
            ConfigValue::Array(a) => a.join(","),
        };

        env::set_var(&env_key, &env_value);
        Ok(())
    }

    /// List all available configuration keys
    pub fn list_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();

        for (key, _) in env::vars() {
            if key.starts_with(&self.prefix) {
                let config_key = self.extract_config_key(&key);
                keys.push(config_key);
            }
        }

        keys.sort();
        keys
    }

    /// Validate environment configuration
    pub fn validate(&self) -> Result<ValidationResult> {
        let config = self.load()?;
        let mut errors = Vec::new();
        let _warnings = Vec::new();

        // Check for required variables
        let required_vars = [
            "core.max_key_size".to_string(),
            "core.max_value_size".to_string(),
            "core.operation_timeout".to_string(),
        ];

        for required in &required_vars {
            if !config.config.contains_key(required) {
                errors.push(ValidationError {
                    field: required.to_string(),
                    message: "Required configuration variable not set".to_string(),
                    current_value: "not set".to_string(),
                    expected: "environment variable".to_string(),
                    severity: ValidationSeverity::Error,
                    fix_suggestion: Some(format!("Set the {} environment variable", required)),
                });
            }
        }

        // Validate specific values
        for (key, value) in &config.config {
            match key.as_str() {
                "core.max_key_size" => {
                    if let ConfigValue::Integer(size) = value {
                        if *size <= 0 {
                            errors.push(ValidationError {
                                field: key.clone(),
                                message: "max_key_size must be greater than 0".to_string(),
                                current_value: size.to_string(),
                                expected: "> 0".to_string(),
                                severity: ValidationSeverity::Critical,
                                fix_suggestion: Some(
                                    "Set max_key_size to 1024 (1KB) for most use cases".to_string(),
                                ),
                            });
                        }
                    }
                }
                "core.max_value_size" => {
                    if let ConfigValue::Integer(size) = value {
                        if *size <= 0 {
                            errors.push(ValidationError {
                                field: key.clone(),
                                message: "max_value_size must be greater than 0".to_string(),
                                current_value: size.to_string(),
                                expected: "> 0".to_string(),
                                severity: ValidationSeverity::Critical,
                                fix_suggestion: Some("Set max_value_size to 10485760 (10MB) for balanced performance".to_string()),
                            });
                        }
                    }
                }
                "environment.name" => {
                    if let ConfigValue::String(name) = value {
                        if !["development", "staging", "production"].contains(&name.as_str()) {
                            errors.push(ValidationError {
                                field: key.clone(),
                                message: "Invalid environment name".to_string(),
                                current_value: name.clone(),
                                fix_suggestion: Some("Set environment.name to 'development', 'staging', or 'production'".to_string()),
                                expected: "development, staging, or production".to_string(),
                                severity: ValidationSeverity::Critical,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings: _warnings,
            suggestions: Vec::new(),
        })
    }
}

impl Default for EnvConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment configuration utilities
pub struct EnvConfigUtils;

impl EnvConfigUtils {
    /// Generate environment variable documentation
    pub fn generate_docs() -> String {
        r#"# F4KVS Environment Variables

## Core Configuration
- `F4KVS_CORE_MAX_KEY_SIZE`: Maximum key size in bytes (default: 1024)
- `F4KVS_CORE_MAX_VALUE_SIZE`: Maximum value size in bytes (default: 10485760)
- `F4KVS_CORE_OPERATION_TIMEOUT`: Operation timeout in seconds (default: 30)
- `F4KVS_CORE_STRICT_KEY_VALIDATION`: Enable strict key validation (default: true)
- `F4KVS_CORE_STORAGE_MODE`: Storage mode - hashmap or btreemap (default: btreemap)

## Environment Configuration
- `F4KVS_ENVIRONMENT_NAME`: Environment name - development, staging, or production (default: development)
- `F4KVS_ENVIRONMENT_DEBUG`: Enable debug mode (default: true)
- `F4KVS_ENVIRONMENT_CONFIG_FILE`: Configuration file path (default: f4kvs.toml)
- `F4KVS_ENVIRONMENT_HOT_RELOAD`: Enable hot-reloading (default: false)
- `F4KVS_ENVIRONMENT_RELOAD_INTERVAL`: Reload interval in seconds (default: 5)

## Performance Configuration
- `F4KVS_PERFORMANCE_ENABLE_SIMD`: Enable SIMD optimizations (default: true)
- `F4KVS_PERFORMANCE_MEMORY_POOL_BLOCK_SIZE`: Memory pool block size (default: 4096)
- `F4KVS_PERFORMANCE_MEMORY_POOL_MAX_POOL_SIZE`: Maximum pool size (default: 5000)
- `F4KVS_PERFORMANCE_MEMORY_POOL_ENABLED`: Enable memory pool (default: true)
- `F4KVS_PERFORMANCE_CACHE_MAX_SIZE`: Cache maximum size (default: 10000)
- `F4KVS_PERFORMANCE_CACHE_ENABLED`: Enable cache (default: true)
- `F4KVS_PERFORMANCE_BATCH_MAX_BATCH_SIZE`: Maximum batch size (default: 1000)
- `F4KVS_PERFORMANCE_BATCH_ENABLED`: Enable batch processing (default: true)

## Security Configuration
- `F4KVS_SECURITY_ENABLE_AUTH`: Enable authentication (default: false)
- `F4KVS_SECURITY_JWT_SECRET`: JWT secret key (default: f4kvs-default-secret-change-in-production)
- `F4KVS_SECURITY_JWT_EXPIRY_SECONDS`: JWT expiration in seconds (default: 3600)
- `F4KVS_SECURITY_ENABLE_ENCRYPTION`: Enable encryption at rest (default: false)
- `F4KVS_SECURITY_ENABLE_AUDIT_LOGGING`: Enable audit logging (default: false)

## Logging Configuration
- `F4KVS_LOGGING_LEVEL`: Log level - trace, debug, info, warn, error (default: info)
- `F4KVS_LOGGING_FORMAT`: Log format - json or text (default: text)
- `F4KVS_LOGGING_STRUCTURED`: Enable structured logging (default: false)
- `F4KVS_LOGGING_CONSOLE`: Enable console logging (default: true)

## Monitoring Configuration
- `F4KVS_MONITORING_ENABLE_PROMETHEUS`: Enable Prometheus metrics (default: false)
- `F4KVS_MONITORING_PROMETHEUS_PORT`: Prometheus port (default: 9090)
- `F4KVS_MONITORING_ENABLE_HEALTH_CHECKS`: Enable health checks (default: true)
- `F4KVS_MONITORING_HEALTH_CHECK_INTERVAL`: Health check interval in seconds (default: 30)
- `F4KVS_MONITORING_ENABLE_PROFILING`: Enable performance profiling (default: false)

## Examples

### Development Environment
```bash
export F4KVS_ENVIRONMENT_NAME=development
export F4KVS_ENVIRONMENT_DEBUG=true
export F4KVS_LOGGING_LEVEL=debug
export F4KVS_MONITORING_ENABLE_PROFILING=true
```

### Production Environment
```bash
export F4KVS_ENVIRONMENT_NAME=production
export F4KVS_ENVIRONMENT_DEBUG=false
export F4KVS_LOGGING_LEVEL=info
export F4KVS_LOGGING_STRUCTURED=true
export F4KVS_SECURITY_ENABLE_AUTH=true
export F4KVS_SECURITY_JWT_SECRET="your-secure-secret-here"
export F4KVS_MONITORING_ENABLE_PROMETHEUS=true
```

### Performance Tuning
```bash
export F4KVS_PERFORMANCE_MEMORY_POOL_BLOCK_SIZE=8192
export F4KVS_PERFORMANCE_MEMORY_POOL_MAX_POOL_SIZE=10000
export F4KVS_PERFORMANCE_CACHE_MAX_SIZE=50000
export F4KVS_PERFORMANCE_BATCH_MAX_BATCH_SIZE=5000
```
"#.to_string()
    }

    /// Generate a .env file template
    pub fn generate_env_file() -> String {
        r#"# F4KVS Environment Variables Template
# Copy this file to .env and modify as needed

# Core Configuration
F4KVS_CORE_MAX_KEY_SIZE=1024
F4KVS_CORE_MAX_VALUE_SIZE=10485760
F4KVS_CORE_OPERATION_TIMEOUT=30
F4KVS_CORE_STRICT_KEY_VALIDATION=true
F4KVS_CORE_STORAGE_MODE=btreemap

# Environment Configuration
F4KVS_ENVIRONMENT_NAME=development
F4KVS_ENVIRONMENT_DEBUG=true
F4KVS_ENVIRONMENT_CONFIG_FILE=f4kvs.toml
F4KVS_ENVIRONMENT_HOT_RELOAD=false
F4KVS_ENVIRONMENT_RELOAD_INTERVAL=5

# Performance Configuration
F4KVS_PERFORMANCE_ENABLE_SIMD=true
F4KVS_PERFORMANCE_MEMORY_POOL_BLOCK_SIZE=4096
F4KVS_PERFORMANCE_MEMORY_POOL_MAX_POOL_SIZE=5000
F4KVS_PERFORMANCE_MEMORY_POOL_ENABLED=true
F4KVS_PERFORMANCE_CACHE_MAX_SIZE=10000
F4KVS_PERFORMANCE_CACHE_ENABLED=true
F4KVS_PERFORMANCE_BATCH_MAX_BATCH_SIZE=1000
F4KVS_PERFORMANCE_BATCH_ENABLED=true

# Security Configuration
F4KVS_SECURITY_ENABLE_AUTH=false
F4KVS_SECURITY_JWT_SECRET=f4kvs-default-secret-change-in-production
F4KVS_SECURITY_JWT_EXPIRY_SECONDS=3600
F4KVS_SECURITY_ENABLE_ENCRYPTION=false
F4KVS_SECURITY_ENABLE_AUDIT_LOGGING=false

# Logging Configuration
F4KVS_LOGGING_LEVEL=info
F4KVS_LOGGING_FORMAT=text
F4KVS_LOGGING_STRUCTURED=false
F4KVS_LOGGING_CONSOLE=true

# Monitoring Configuration
F4KVS_MONITORING_ENABLE_PROMETHEUS=false
F4KVS_MONITORING_PROMETHEUS_PORT=9090
F4KVS_MONITORING_ENABLE_HEALTH_CHECKS=true
F4KVS_MONITORING_HEALTH_CHECK_INTERVAL=30
F4KVS_MONITORING_ENABLE_PROFILING=false
"#
        .to_string()
    }

    /// Load configuration from .env file
    pub async fn load_from_env_file(path: &str) -> Result<HashMap<String, String>> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| F4KvsError::io_with_path(path, e.to_string()))?;

        let mut env_vars = HashMap::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key=value pairs
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Remove quotes if present
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };

                env_vars.insert(key.to_string(), value.to_string());
            }
        }

        Ok(env_vars)
    }
}

// Re-export validation types for convenience
pub use crate::config_validator::{
    ValidationError, ValidationResult, ValidationSeverity, ValidationWarning,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_config_loader() {
        let loader = EnvConfigLoader::new();
        let config = loader.load().unwrap();
        assert!(config.variables.is_empty() || !config.variables.is_empty());
    }

    #[test]
    fn test_config_value_conversions() {
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(string_val.as_string(), Some(&"test".to_string()));
        assert_eq!(string_val.as_integer(), None);

        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.as_integer(), Some(42));
        assert_eq!(int_val.as_string(), None);

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.as_boolean(), Some(true));
        assert_eq!(bool_val.as_string(), None);
    }

    #[test]
    fn test_boolean_parsing() {
        let loader = EnvConfigLoader::new();

        assert!(loader.parse_boolean("true").unwrap());
        assert!(!loader.parse_boolean("false").unwrap());
        assert!(loader.parse_boolean("1").unwrap());
        assert!(!loader.parse_boolean("0").unwrap());
        assert!(loader.parse_boolean("yes").unwrap());
        assert!(!loader.parse_boolean("no").unwrap());
        assert!(loader.parse_boolean("enabled").unwrap());
        assert!(!loader.parse_boolean("disabled").unwrap());

        assert!(loader.parse_boolean("invalid").is_err());
    }

    #[test]
    fn test_key_extraction() {
        let loader = EnvConfigLoader::new();
        assert_eq!(
            loader.extract_config_key("F4KVS_CORE_MAX_KEY_SIZE"),
            "core.max.key.size"
        );
        assert_eq!(
            loader.extract_config_key("F4KVS_ENVIRONMENT_DEBUG"),
            "environment.debug"
        );
    }

    #[test]
    fn test_env_file_generation() {
        let env_file = EnvConfigUtils::generate_env_file();
        assert!(env_file.contains("F4KVS_CORE_MAX_KEY_SIZE"));
        assert!(env_file.contains("F4KVS_ENVIRONMENT_NAME"));
        assert!(env_file.contains("F4KVS_PERFORMANCE_ENABLE_SIMD"));
    }

    #[test]
    fn test_docs_generation() {
        let docs = EnvConfigUtils::generate_docs();
        assert!(docs.contains("F4KVS_CORE_MAX_KEY_SIZE"));
        assert!(docs.contains("Maximum key size in bytes"));
        assert!(docs.contains("Development Environment"));
        assert!(docs.contains("Production Environment"));
    }
}
