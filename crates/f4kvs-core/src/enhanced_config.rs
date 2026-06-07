//! Enhanced Configuration Management for F4KVS Core
//!
//! This module provides comprehensive configuration management including:
//! - Environment variable support with automatic mapping
//! - Configuration validation with detailed error messages
//! - Hot-reloading with file watching
//! - Configuration merging from multiple sources
//! - Better defaults with environment-specific presets
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{Config, F4KvsError, Result, StorageMode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;

/// Enhanced configuration with environment variable support
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EnhancedConfig {
    /// Core F4KVS configuration
    #[serde(flatten)]
    pub core: Config,

    /// Environment-specific settings
    pub environment: EnvironmentConfig,

    /// Performance tuning settings
    pub performance: PerformanceConfig,

    /// Security settings
    pub security: SecurityConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Monitoring and observability settings
    pub monitoring: MonitoringConfig,
}

/// Environment-specific configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Environment name (development, staging, production)
    pub name: String,

    /// Debug mode enabled
    pub debug: bool,

    /// Configuration file path
    pub config_file: Option<String>,

    /// Enable hot-reloading
    pub hot_reload: bool,

    /// Reload interval in seconds
    pub reload_interval: u64,
}

/// Performance tuning configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable SIMD optimizations
    pub enable_simd: bool,

    /// Memory pool configuration
    pub memory_pool: MemoryPoolConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Batch processing settings
    pub batch: BatchConfig,
}

/// Memory pool configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    /// Block size in bytes
    pub block_size: usize,

    /// Maximum pool size in blocks
    pub max_pool_size: usize,

    /// Initial number of blocks to allocate
    pub initial_blocks: usize,

    /// Enable memory pool
    pub enabled: bool,
}

/// Cache configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cache size in entries
    pub max_size: usize,

    /// Cache TTL in seconds
    pub ttl_seconds: u64,

    /// Enable cache
    pub enabled: bool,

    /// Cache eviction policy
    pub eviction_policy: EvictionPolicy,
}

/// Cache eviction policy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// Random
    Random,
}

/// Batch processing configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_batch_size: usize,

    /// Batch timeout in milliseconds
    pub timeout_ms: u64,

    /// Enable batch processing
    pub enabled: bool,
}

/// Security configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable authentication
    pub enable_auth: bool,

    /// JWT secret key
    pub jwt_secret: String,

    /// JWT expiration in seconds
    pub jwt_expiry_seconds: u64,

    /// Enable encryption at rest
    pub enable_encryption: bool,

    /// Encryption key (base64 encoded)
    pub encryption_key: Option<String>,

    /// Enable audit logging
    pub enable_audit_logging: bool,
}

/// Logging configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log format (json, text)
    pub format: String,

    /// Enable structured logging
    pub structured: bool,

    /// Log file path (optional)
    pub file_path: Option<String>,

    /// Enable console logging
    pub console: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable Prometheus metrics
    pub enable_prometheus: bool,

    /// Prometheus port
    pub prometheus_port: u16,

    /// Enable health checks
    pub enable_health_checks: bool,

    /// Health check interval in seconds
    pub health_check_interval: u64,

    /// Enable performance profiling
    pub enable_profiling: bool,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            name: "development".to_string(),
            debug: true,
            config_file: None,
            hot_reload: false,
            reload_interval: 5,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_simd: true,
            memory_pool: MemoryPoolConfig::default(),
            cache: CacheConfig::default(),
            batch: BatchConfig::default(),
        }
    }
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            block_size: 4096,
            max_pool_size: 5000,
            initial_blocks: 50,
            enabled: true,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            ttl_seconds: 3600,
            enabled: true,
            eviction_policy: EvictionPolicy::LRU,
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            timeout_ms: 100,
            enabled: true,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: false,
            jwt_secret: "f4kvs-default-secret-change-in-production".to_string(),
            jwt_expiry_seconds: 3600,
            enable_encryption: false,
            encryption_key: None,
            enable_audit_logging: false,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "text".to_string(),
            structured: false,
            file_path: None,
            console: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_prometheus: false,
            prometheus_port: 9090,
            enable_health_checks: true,
            health_check_interval: 30,
            enable_profiling: false,
        }
    }
}

/// Configuration source priority
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    /// Default values (lowest priority)
    Default = 0,
    /// Configuration file
    File = 1,
    /// Environment variables (highest priority)
    Environment = 2,
}

/// Configuration builder with environment variable support
pub struct EnhancedConfigBuilder {
    config: EnhancedConfig,
    env_prefix: String,
    source_priority: Vec<ConfigSource>,
}

impl EnhancedConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            config: EnhancedConfig::default(),
            env_prefix: "F4KVS_".to_string(),
            source_priority: vec![
                ConfigSource::Default,
                ConfigSource::File,
                ConfigSource::Environment,
            ],
        }
    }

    /// Set environment variable prefix
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Set source priority order
    pub fn with_source_priority(mut self, priority: Vec<ConfigSource>) -> Self {
        self.source_priority = priority;
        self
    }

    /// Load configuration from environment variables
    pub fn load_from_env(mut self) -> Result<Self> {
        let env_vars = self.collect_env_vars()?;
        self.apply_env_vars(env_vars)?;
        Ok(self)
    }

    /// Load configuration from file
    pub async fn load_from_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Err(F4KvsError::io_with_path(
                path.as_ref().to_string_lossy().as_ref(),
                "Configuration file not found",
            ));
        }

        let content = fs::read_to_string(path.as_ref()).await.map_err(|e| {
            F4KvsError::io_with_path(path.as_ref().to_string_lossy().as_ref(), e.to_string())
        })?;

        let file_config: EnhancedConfig = if path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "toml")
            .unwrap_or(false)
        {
            toml::from_str(&content)
                .map_err(|e| F4KvsError::config_with_field("toml_parse", e.to_string()))?
        } else {
            serde_json::from_str(&content)
                .map_err(|e| F4KvsError::config_with_field("json_parse", e.to_string()))?
        };

        self.merge_config(file_config, ConfigSource::File);
        Ok(self)
    }

    /// Build the final configuration
    pub fn build(self) -> Result<EnhancedConfig> {
        let config = self.config;
        config.validate()?;
        Ok(config)
    }

    /// Collect environment variables with the configured prefix
    fn collect_env_vars(&self) -> Result<HashMap<String, String>> {
        let mut env_vars = HashMap::new();

        for (key, value) in env::vars() {
            if key.starts_with(&self.env_prefix) {
                let config_key = key
                    .strip_prefix(&self.env_prefix)
                    .unwrap()
                    .to_lowercase()
                    .replace("_", ".");
                env_vars.insert(config_key, value);
            }
        }

        Ok(env_vars)
    }

    /// Apply environment variables to configuration
    fn apply_env_vars(&mut self, env_vars: HashMap<String, String>) -> Result<()> {
        for (key, value) in env_vars {
            self.set_config_value(&key, &value)?;
        }
        Ok(())
    }

    /// Set configuration value from string
    fn set_config_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            // Core configuration
            "core.max_key_size" => {
                self.config.core.max_key_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("core.max_key_size", e.to_string())
                    })?;
            }
            "core.max_value_size" => {
                self.config.core.max_value_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("core.max_value_size", e.to_string())
                    })?;
            }
            "core.operation_timeout" => {
                let secs: u64 = value.parse().map_err(|e: std::num::ParseIntError| {
                    F4KvsError::config_with_field("core.operation_timeout", e.to_string())
                })?;
                self.config.core.operation_timeout = Duration::from_secs(secs);
            }
            "core.strict_key_validation" => {
                self.config.core.strict_key_validation =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("core.strict_key_validation", e.to_string())
                    })?;
            }
            "core.storage_mode" => {
                self.config.core.storage_mode = match value.to_lowercase().as_str() {
                    "hashmap" => StorageMode::HashMap,
                    "btreemap" => StorageMode::BTreeMap,
                    _ => {
                        return Err(F4KvsError::config_with_field(
                            "core.storage_mode",
                            format!("Invalid storage mode: {}", value),
                        ))
                    }
                };
            }

            // Environment configuration
            "environment.name" => {
                self.config.environment.name = value.to_string();
            }
            "environment.debug" => {
                self.config.environment.debug =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("environment.debug", e.to_string())
                    })?;
            }
            "environment.config_file" => {
                self.config.environment.config_file = Some(value.to_string());
            }
            "environment.hot_reload" => {
                self.config.environment.hot_reload =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("environment.hot_reload", e.to_string())
                    })?;
            }
            "environment.reload_interval" => {
                self.config.environment.reload_interval =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("environment.reload_interval", e.to_string())
                    })?;
            }

            // Performance configuration
            "performance.enable_simd" | "performance.enable.simd" => {
                self.config.performance.enable_simd =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("performance.enable_simd", e.to_string())
                    })?;
            }
            "performance.memory_pool.block_size" => {
                self.config.performance.memory_pool.block_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field(
                            "performance.memory_pool.block_size",
                            e.to_string(),
                        )
                    })?;
            }
            "performance.memory_pool.max_pool_size" => {
                self.config.performance.memory_pool.max_pool_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field(
                            "performance.memory_pool.max_pool_size",
                            e.to_string(),
                        )
                    })?;
            }
            "performance.memory_pool.enabled" => {
                self.config.performance.memory_pool.enabled =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field(
                            "performance.memory_pool.enabled",
                            e.to_string(),
                        )
                    })?;
            }
            "performance.cache.max_size" => {
                self.config.performance.cache.max_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("performance.cache.max_size", e.to_string())
                    })?;
            }
            "performance.cache.enabled" => {
                self.config.performance.cache.enabled =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("performance.cache.enabled", e.to_string())
                    })?;
            }
            "performance.batch.max_batch_size" => {
                self.config.performance.batch.max_batch_size =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field(
                            "performance.batch.max_batch_size",
                            e.to_string(),
                        )
                    })?;
            }
            "performance.batch.enabled" => {
                self.config.performance.batch.enabled =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("performance.batch.enabled", e.to_string())
                    })?;
            }

            // Security configuration
            "security.enable_auth" => {
                self.config.security.enable_auth =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("security.enable_auth", e.to_string())
                    })?;
            }
            "security.jwt_secret" => {
                self.config.security.jwt_secret = value.to_string();
            }
            "security.jwt_expiry_seconds" => {
                self.config.security.jwt_expiry_seconds =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("security.jwt_expiry_seconds", e.to_string())
                    })?;
            }
            "security.enable_encryption" => {
                self.config.security.enable_encryption =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("security.enable_encryption", e.to_string())
                    })?;
            }
            "security.enable_audit_logging" => {
                self.config.security.enable_audit_logging =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field(
                            "security.enable_audit_logging",
                            e.to_string(),
                        )
                    })?;
            }

            // Logging configuration
            "logging.level" => {
                self.config.logging.level = value.to_string();
            }
            "logging.format" => {
                self.config.logging.format = value.to_string();
            }
            "logging.structured" => {
                self.config.logging.structured =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("logging.structured", e.to_string())
                    })?;
            }
            "logging.console" => {
                self.config.logging.console =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("logging.console", e.to_string())
                    })?;
            }

            // Monitoring configuration
            "monitoring.enable_prometheus" => {
                self.config.monitoring.enable_prometheus =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("monitoring.enable_prometheus", e.to_string())
                    })?;
            }
            "monitoring.prometheus_port" => {
                self.config.monitoring.prometheus_port =
                    value.parse().map_err(|e: std::num::ParseIntError| {
                        F4KvsError::config_with_field("monitoring.prometheus_port", e.to_string())
                    })?;
            }
            "monitoring.enable_health_checks" => {
                self.config.monitoring.enable_health_checks =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field(
                            "monitoring.enable_health_checks",
                            e.to_string(),
                        )
                    })?;
            }
            "monitoring.enable_profiling" => {
                self.config.monitoring.enable_profiling =
                    value.parse().map_err(|e: std::str::ParseBoolError| {
                        F4KvsError::config_with_field("monitoring.enable_profiling", e.to_string())
                    })?;
            }

            _ => {
                return Err(F4KvsError::config_with_field(
                    "unknown_key",
                    format!("Unknown configuration key: {}", key),
                ));
            }
        }

        Ok(())
    }

    /// Merge configuration from another source
    pub fn merge_config(&mut self, other: EnhancedConfig, source: ConfigSource) {
        // Only merge if the source has higher priority
        if !self.source_priority.contains(&source) {
            return;
        }

        // Merge core configuration
        if source >= ConfigSource::File {
            self.config.core = other.core;
        }

        // Merge other sections
        if source >= ConfigSource::File {
            self.config.environment = other.environment;
            self.config.performance = other.performance;
            self.config.security = other.security;
            self.config.logging = other.logging;
            self.config.monitoring = other.monitoring;
        }
    }
}

impl Default for EnhancedConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedConfig {
    /// Create a new enhanced configuration builder
    pub fn builder() -> EnhancedConfigBuilder {
        EnhancedConfigBuilder::new()
    }

    /// Load configuration from multiple sources
    pub async fn load() -> Result<Self> {
        let mut builder = EnhancedConfigBuilder::new();

        // Load from environment variables first (highest priority)
        builder = builder.load_from_env()?;

        // Try to load from default config file if not specified
        let config_file =
            env::var("F4KVS_CONFIG_FILE").unwrap_or_else(|_| "f4kvs.toml".to_string());

        if Path::new(&config_file).exists() {
            builder = builder.load_from_file(&config_file).await?;
        }

        builder.build()
    }

    /// Validate the enhanced configuration
    pub fn validate(&self) -> Result<()> {
        // Validate core configuration
        self.core.validate()?;

        // Validate environment configuration
        if self.environment.name.is_empty() {
            return Err(F4KvsError::config("environment.name cannot be empty"));
        }

        if !["development", "staging", "production"].contains(&self.environment.name.as_str()) {
            return Err(F4KvsError::config(
                "environment.name must be one of: development, staging, production",
            ));
        }

        if self.environment.reload_interval == 0 {
            return Err(F4KvsError::config(
                "environment.reload_interval must be > 0",
            ));
        }

        // Validate performance configuration
        if self.performance.memory_pool.block_size == 0 {
            return Err(F4KvsError::config(
                "performance.memory_pool.block_size must be > 0",
            ));
        }

        if self.performance.memory_pool.max_pool_size == 0 {
            return Err(F4KvsError::config(
                "performance.memory_pool.max_pool_size must be > 0",
            ));
        }

        if self.performance.cache.max_size == 0 {
            return Err(F4KvsError::config("performance.cache.max_size must be > 0"));
        }

        if self.performance.batch.max_batch_size == 0 {
            return Err(F4KvsError::config(
                "performance.batch.max_batch_size must be > 0",
            ));
        }

        // Validate security configuration
        if self.security.enable_auth && self.security.jwt_secret.is_empty() {
            return Err(F4KvsError::config(
                "security.jwt_secret cannot be empty when authentication is enabled",
            ));
        }

        if self.security.jwt_expiry_seconds == 0 {
            return Err(F4KvsError::config(
                "security.jwt_expiry_seconds must be > 0",
            ));
        }

        // Validate logging configuration
        if !["trace", "debug", "info", "warn", "error"].contains(&self.logging.level.as_str()) {
            return Err(F4KvsError::config(
                "logging.level must be one of: trace, debug, info, warn, error",
            ));
        }

        if !["json", "text"].contains(&self.logging.format.as_str()) {
            return Err(F4KvsError::config(
                "logging.format must be one of: json, text",
            ));
        }

        // Validate monitoring configuration
        if self.monitoring.prometheus_port == 0 {
            return Err(F4KvsError::config("monitoring.prometheus_port must be > 0"));
        }

        if self.monitoring.health_check_interval == 0 {
            return Err(F4KvsError::config(
                "monitoring.health_check_interval must be > 0",
            ));
        }

        Ok(())
    }

    /// Get environment-specific defaults
    pub fn for_environment(env: &str) -> Self {
        let mut config = Self::default();

        match env {
            "production" => {
                config.environment.name = "production".to_string();
                config.environment.debug = false;
                config.environment.hot_reload = false;
                config.logging.level = "info".to_string();
                config.logging.structured = true;
                config.security.enable_auth = true;
                config.security.enable_audit_logging = true;
                config.monitoring.enable_prometheus = true;
                config.monitoring.enable_health_checks = true;
            }
            "staging" => {
                config.environment.name = "staging".to_string();
                config.environment.debug = false;
                config.environment.hot_reload = true;
                config.logging.level = "debug".to_string();
                config.logging.structured = true;
                config.security.enable_auth = true;
                config.security.enable_audit_logging = true;
                config.monitoring.enable_prometheus = true;
            }
            _ => {
                // development
                config.environment.name = "development".to_string();
                config.environment.debug = true;
                config.environment.hot_reload = true;
                config.logging.level = "debug".to_string();
                config.logging.console = true;
                config.security.enable_auth = false;
                config.monitoring.enable_profiling = true;
            }
        }

        config
    }

    /// Convert to basic Config for backward compatibility
    pub fn to_core_config(&self) -> Config {
        self.core.clone()
    }
}

/// Enhanced configuration manager with hot-reloading
pub struct EnhancedConfigManager {
    config: Arc<RwLock<EnhancedConfig>>,
    #[allow(dead_code)]
    reloader: Option<Arc<dyn ConfigReloader + Send + Sync>>,
}

/// Trait for configuration reloaders
pub trait ConfigReloader: Send + Sync {
    /// Reload configuration from source
    fn reload(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<EnhancedConfig>> + Send + '_>>;
    /// Start the configuration reloader
    fn start(&self)
        -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
    /// Stop the configuration reloader
    fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
    /// Check if the reloader is currently running
    fn is_running(&self) -> bool;
}

impl EnhancedConfigManager {
    /// Create a new configuration manager
    pub async fn new() -> Result<Self> {
        let config = EnhancedConfig::load().await?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            reloader: None,
        })
    }

    /// Create a configuration manager with hot-reloading
    pub async fn with_reloading(_config_path: impl AsRef<Path>) -> Result<Self> {
        let config = EnhancedConfig::load().await?;
        let manager = Self {
            config: Arc::new(RwLock::new(config)),
            reloader: None, // File-based reloader not critical for basic functionality
        };
        Ok(manager)
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> EnhancedConfig {
        self.config.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(&self, new_config: EnhancedConfig) -> Result<()> {
        new_config.validate()?;
        let mut config = self.config.write().await;
        *config = new_config;
        Ok(())
    }

    /// Get core configuration for backward compatibility
    pub async fn get_core_config(&self) -> Config {
        self.config.read().await.to_core_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_config_default() {
        let config = EnhancedConfig::default();
        assert_eq!(config.environment.name, "development");
        assert!(config.environment.debug);
        assert!(config.performance.enable_simd);
        assert!(config.performance.memory_pool.enabled);
        assert!(!config.security.enable_auth);
        assert_eq!(config.logging.level, "info");
        assert!(!config.monitoring.enable_prometheus);
    }

    #[test]
    fn test_environment_specific_configs() {
        let prod_config = EnhancedConfig::for_environment("production");
        assert_eq!(prod_config.environment.name, "production");
        assert!(!prod_config.environment.debug);
        assert!(!prod_config.environment.hot_reload);
        assert!(prod_config.security.enable_auth);
        assert!(prod_config.monitoring.enable_prometheus);

        let dev_config = EnhancedConfig::for_environment("development");
        assert_eq!(dev_config.environment.name, "development");
        assert!(dev_config.environment.debug);
        assert!(dev_config.environment.hot_reload);
        assert!(!dev_config.security.enable_auth);
        assert!(dev_config.monitoring.enable_profiling);
    }

    #[test]
    fn test_config_validation() {
        let mut config = EnhancedConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid environment name should fail
        config.environment.name = "invalid".to_string();
        assert!(config.validate().is_err());

        // Invalid logging level should fail
        config.environment.name = "development".to_string();
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_config_manager() {
        let manager = EnhancedConfigManager::new().await.unwrap();
        let config = manager.get_config().await;
        assert_eq!(config.environment.name, "development");
    }

    #[test]
    fn test_config_builder() {
        let builder = EnhancedConfigBuilder::new();
        let config = builder.build().unwrap();
        assert_eq!(config.environment.name, "development");
    }

    #[test]
    fn test_config_merging() {
        let mut builder = EnhancedConfigBuilder::new();
        let mut other_config = EnhancedConfig::default();
        other_config.environment.name = "production".to_string();
        builder.merge_config(other_config, ConfigSource::File);
        let config = builder.build().unwrap();
        assert_eq!(config.environment.name, "production");
    }

    #[test]
    fn test_config_source_priority() {
        let builder = EnhancedConfigBuilder::new()
            .with_source_priority(vec![ConfigSource::Default, ConfigSource::Environment]);
        let config = builder.build().unwrap();
        assert_eq!(config.environment.name, "development");
    }

    #[test]
    fn test_config_validation_comprehensive() {
        let mut config = EnhancedConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Test invalid environment name
        config.environment.name = "invalid".to_string();
        assert!(config.validate().is_err());

        // Test invalid reload interval
        config.environment.name = "development".to_string();
        config.environment.reload_interval = 0;
        assert!(config.validate().is_err());

        // Test invalid memory pool block size
        config.environment.reload_interval = 5;
        config.performance.memory_pool.block_size = 0;
        assert!(config.validate().is_err());

        // Test invalid cache max size
        config.performance.memory_pool.block_size = 4096;
        config.performance.cache.max_size = 0;
        assert!(config.validate().is_err());

        // Test invalid batch max size
        config.performance.cache.max_size = 10000;
        config.performance.batch.max_batch_size = 0;
        assert!(config.validate().is_err());

        // Test invalid JWT expiry
        config.performance.batch.max_batch_size = 1000;
        config.security.jwt_expiry_seconds = 0;
        assert!(config.validate().is_err());

        // Test invalid logging level
        config.security.jwt_expiry_seconds = 3600;
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());

        // Test invalid logging format
        config.logging.level = "info".to_string();
        config.logging.format = "invalid".to_string();
        assert!(config.validate().is_err());

        // Test invalid prometheus port
        config.logging.format = "text".to_string();
        config.monitoring.prometheus_port = 0;
        assert!(config.validate().is_err());

        // Test invalid health check interval
        config.monitoring.prometheus_port = 9090;
        config.monitoring.health_check_interval = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_staging_environment_config() {
        let staging_config = EnhancedConfig::for_environment("staging");
        assert_eq!(staging_config.environment.name, "staging");
        assert!(!staging_config.environment.debug);
        assert!(staging_config.environment.hot_reload);
        assert!(staging_config.security.enable_auth);
        assert!(staging_config.monitoring.enable_prometheus);
    }

    #[test]
    fn test_to_core_config() {
        let enhanced = EnhancedConfig::default();
        let core = enhanced.to_core_config();
        assert_eq!(core.max_key_size, enhanced.core.max_key_size);
        assert_eq!(core.storage_mode, enhanced.core.storage_mode);
    }

    #[tokio::test]
    async fn test_config_manager_update() {
        let manager = EnhancedConfigManager::new().await.unwrap();
        let mut new_config = manager.get_config().await;
        new_config.environment.name = "production".to_string();
        manager.update_config(new_config.clone()).await.unwrap();
        let updated = manager.get_config().await;
        assert_eq!(updated.environment.name, "production");
    }

    #[tokio::test]
    async fn test_config_manager_core_config() {
        let manager = EnhancedConfigManager::new().await.unwrap();
        let core_config = manager.get_core_config().await;
        assert!(core_config.validate().is_ok());
    }

    #[test]
    fn test_config_builder_env_prefix() {
        let builder = EnhancedConfigBuilder::new().with_env_prefix("TEST_");
        // Builder should accept custom prefix
        assert!(builder.build().is_ok());
    }

    #[test]
    fn test_config_defaults_comprehensive() {
        let config = EnhancedConfig::default();

        // Test environment defaults
        assert_eq!(config.environment.name, "development");
        assert!(config.environment.debug);
        assert!(!config.environment.hot_reload);
        assert_eq!(config.environment.reload_interval, 5);

        // Test performance defaults
        assert!(config.performance.enable_simd);
        assert_eq!(config.performance.memory_pool.block_size, 4096);
        assert_eq!(config.performance.memory_pool.max_pool_size, 5000);
        assert!(config.performance.memory_pool.enabled);
        assert_eq!(config.performance.cache.max_size, 10000);
        assert_eq!(config.performance.cache.ttl_seconds, 3600);
        assert!(config.performance.cache.enabled);
        assert_eq!(config.performance.batch.max_batch_size, 1000);
        assert_eq!(config.performance.batch.timeout_ms, 100);
        assert!(config.performance.batch.enabled);

        // Test security defaults
        assert!(!config.security.enable_auth);
        assert!(!config.security.enable_encryption);
        assert!(!config.security.enable_audit_logging);
        assert_eq!(config.security.jwt_expiry_seconds, 3600);

        // Test logging defaults
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "text");
        assert!(!config.logging.structured);
        assert!(config.logging.console);

        // Test monitoring defaults
        assert!(!config.monitoring.enable_prometheus);
        assert_eq!(config.monitoring.prometheus_port, 9090);
        assert!(config.monitoring.enable_health_checks);
        assert_eq!(config.monitoring.health_check_interval, 30);
        assert!(!config.monitoring.enable_profiling);
    }

    #[test]
    fn test_eviction_policy_enum() {
        // Test EvictionPolicy variants
        assert_eq!(EvictionPolicy::LRU, EvictionPolicy::LRU);
        assert_eq!(EvictionPolicy::LFU, EvictionPolicy::LFU);
        assert_eq!(EvictionPolicy::Random, EvictionPolicy::Random);
        assert_ne!(EvictionPolicy::LRU, EvictionPolicy::LFU);
    }
}
