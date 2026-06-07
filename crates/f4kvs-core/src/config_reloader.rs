//! Configuration hot-reloading for F4KVS Core
//!
//! This module provides the ability to reload configuration at runtime without
//! restarting the application. It monitors configuration files for changes and
//! notifies subscribers when updates occur.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{Config, F4KvsError, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::{watch, RwLock};
use tokio::time::interval;

/// Type alias for configuration validator function
pub type ConfigValidator = dyn Fn(&Config) -> Result<()> + Send + Sync;

/// Configuration reloader that monitors files for changes
pub struct ConfigReloader {
    /// Current configuration
    config: Arc<RwLock<Config>>,
    /// Configuration file path
    config_path: String,
    /// Watch channel for configuration changes
    watch_sender: watch::Sender<Config>,
    /// Watch receiver for configuration changes
    watch_receiver: watch::Receiver<Config>,
    /// Reload interval
    reload_interval: Duration,
    /// Whether the reloader is running
    running: Arc<RwLock<bool>>,
    /// Configuration validation function
    validator: Option<Arc<ConfigValidator>>,
}

/// Configuration change event
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeEvent {
    /// Configuration was reloaded successfully
    Reloaded(Config),
    /// Configuration reload failed
    ReloadFailed(String),
    /// Configuration file not found
    FileNotFound,
    /// Configuration validation failed
    ValidationFailed(String),
}

/// Configuration reloader builder
pub struct ConfigReloaderBuilder {
    config_path: Option<String>,
    reload_interval: Duration,
    validator: Option<Arc<ConfigValidator>>,
}

impl ConfigReloaderBuilder {
    /// Create a new configuration reloader builder
    pub fn new() -> Self {
        Self {
            config_path: None,
            reload_interval: Duration::from_secs(5),
            validator: None,
        }
    }
}

impl Default for ConfigReloaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigReloaderBuilder {
    /// Set the configuration file path
    pub fn with_config_path(mut self, path: impl Into<String>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set the reload interval
    pub fn with_reload_interval(mut self, interval: Duration) -> Self {
        self.reload_interval = interval;
        self
    }

    /// Set a custom validator function
    pub fn with_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&Config) -> Result<()> + Send + Sync + 'static,
    {
        self.validator = Some(Arc::new(validator));
        self
    }

    /// Build the configuration reloader
    pub async fn build(self) -> Result<ConfigReloader> {
        let config_path = self
            .config_path
            .ok_or_else(|| F4KvsError::config("Configuration file path is required"))?;

        // Load initial configuration
        let initial_config = Self::load_config_from_file(&config_path).await?;

        // Validate initial configuration
        if let Some(ref validator) = self.validator {
            validator(&initial_config)?;
        } else {
            initial_config.validate()?;
        }

        // Create watch channel
        let (watch_sender, watch_receiver) = watch::channel(initial_config.clone());

        Ok(ConfigReloader {
            config: Arc::new(RwLock::new(initial_config)),
            config_path,
            watch_sender,
            watch_receiver,
            reload_interval: self.reload_interval,
            running: Arc::new(RwLock::new(false)),
            validator: self.validator,
        })
    }

    /// Load configuration from file
    async fn load_config_from_file(path: &str) -> Result<Config> {
        if !Path::new(path).exists() {
            return Err(F4KvsError::io_with_path(
                path,
                "Configuration file not found",
            ));
        }

        let content = fs::read_to_string(path)
            .await
            .map_err(|e| F4KvsError::io_with_path(path, e.to_string()))?;

        // Try to parse as TOML first, then JSON
        let config: Config = if path.ends_with(".toml") {
            toml::from_str(&content)
                .map_err(|e| F4KvsError::config_with_field("toml_parse", e.to_string()))?
        } else if path.ends_with(".json") {
            serde_json::from_str(&content)
                .map_err(|e| F4KvsError::config_with_field("json_parse", e.to_string()))?
        } else {
            // Default to TOML
            toml::from_str(&content)
                .map_err(|e| F4KvsError::config_with_field("toml_parse", e.to_string()))?
        };

        Ok(config)
    }
}

impl ConfigReloader {
    /// Create a new configuration reloader builder
    pub fn builder() -> ConfigReloaderBuilder {
        ConfigReloaderBuilder::new()
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Get a receiver for configuration changes
    pub fn subscribe(&self) -> watch::Receiver<Config> {
        self.watch_receiver.clone()
    }

    /// Start the configuration reloader
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(F4KvsError::config(
                "Configuration reloader is already running",
            ));
        }
        *running = true;
        drop(running);

        let config = Arc::clone(&self.config);
        let config_path = self.config_path.clone();
        let watch_sender = self.watch_sender.clone();
        let reload_interval = self.reload_interval;
        let running = Arc::clone(&self.running);
        let validator = self.validator.as_ref().map(|v| Arc::clone(v));

        tokio::spawn(async move {
            let mut interval = interval(reload_interval);

            while *running.read().await {
                interval.tick().await;

                // Check if we should still be running
                if !*running.read().await {
                    break;
                }

                // Try to reload configuration
                match ConfigReloaderBuilder::load_config_from_file(&config_path).await {
                    Ok(new_config) => {
                        // Validate new configuration
                        let validation_result = if let Some(ref validator) = validator {
                            validator(&new_config)
                        } else {
                            new_config.validate()
                        };

                        match validation_result {
                            Ok(_) => {
                                // Update configuration
                                {
                                    let mut current_config = config.write().await;
                                    *current_config = new_config.clone();
                                }

                                // Notify subscribers
                                if watch_sender.send(new_config).is_err() {
                                    // No more subscribers, stop reloading
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Configuration validation failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to reload configuration: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the configuration reloader
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Check if the reloader is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Manually trigger a configuration reload
    pub async fn reload(&self) -> Result<ConfigChangeEvent> {
        match ConfigReloaderBuilder::load_config_from_file(&self.config_path).await {
            Ok(new_config) => {
                // Validate new configuration
                let validation_result = if let Some(ref validator) = self.validator {
                    validator(&new_config)
                } else {
                    new_config.validate()
                };

                match validation_result {
                    Ok(_) => {
                        // Update configuration
                        {
                            let mut current_config = self.config.write().await;
                            *current_config = new_config.clone();
                        }

                        // Notify subscribers
                        if self.watch_sender.send(new_config.clone()).is_err() {
                            return Err(F4KvsError::internal(
                                "Failed to notify configuration subscribers",
                            ));
                        }

                        Ok(ConfigChangeEvent::Reloaded(new_config))
                    }
                    Err(e) => Ok(ConfigChangeEvent::ValidationFailed(e.to_string())),
                }
            }
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(ConfigChangeEvent::FileNotFound)
                } else {
                    Ok(ConfigChangeEvent::ReloadFailed(e.to_string()))
                }
            }
        }
    }

    /// Get configuration file path
    pub fn config_path(&self) -> &str {
        &self.config_path
    }

    /// Get reload interval
    pub fn reload_interval(&self) -> Duration {
        self.reload_interval
    }
}

/// Configuration manager that provides a unified interface for configuration
pub struct ConfigManager {
    reloader: Option<ConfigReloader>,
    config: Arc<RwLock<Config>>,
}

impl ConfigManager {
    /// Create a new configuration manager with static configuration
    pub fn new(config: Config) -> Self {
        Self {
            reloader: None,
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Create a new configuration manager with hot-reloading
    pub async fn with_reloading(config_path: impl Into<String>) -> Result<Self> {
        let reloader = ConfigReloader::builder()
            .with_config_path(config_path)
            .build()
            .await?;

        let config = reloader.get_config().await;
        let manager = Self {
            reloader: Some(reloader),
            config: Arc::new(RwLock::new(config)),
        };

        Ok(manager)
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> Config {
        if let Some(ref reloader) = self.reloader {
            reloader.get_config().await
        } else {
            self.config.read().await.clone()
        }
    }

    /// Update configuration (only works for static configuration)
    pub async fn update_config(&self, new_config: Config) -> Result<()> {
        if self.reloader.is_some() {
            return Err(F4KvsError::config(
                "Cannot update configuration when hot-reloading is enabled",
            ));
        }

        new_config.validate()?;
        let mut config = self.config.write().await;
        *config = new_config;
        Ok(())
    }

    /// Subscribe to configuration changes (only works with hot-reloading)
    pub fn subscribe(&self) -> Option<watch::Receiver<Config>> {
        self.reloader.as_ref().map(|r| r.subscribe())
    }

    /// Start hot-reloading (only works with hot-reloading enabled)
    pub async fn start_reloading(&self) -> Result<()> {
        if let Some(ref reloader) = self.reloader {
            reloader.start().await
        } else {
            Err(F4KvsError::config("Hot-reloading is not enabled"))
        }
    }

    /// Stop hot-reloading
    pub async fn stop_reloading(&self) -> Result<()> {
        if let Some(ref reloader) = self.reloader {
            reloader.stop().await
        } else {
            Err(F4KvsError::config("Hot-reloading is not enabled"))
        }
    }

    /// Check if hot-reloading is enabled
    pub fn is_reloading_enabled(&self) -> bool {
        self.reloader.is_some()
    }

    /// Check if hot-reloading is running
    pub async fn is_reloading(&self) -> bool {
        if let Some(ref reloader) = self.reloader {
            reloader.is_running().await
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_config_reloader_builder() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_string_lossy().to_string();

        // Write test configuration
        let config_content = r#"
max_key_size = 2048
max_value_size = 1048576
operation_timeout = 60
strict_key_validation = false
storage_mode = "HashMap"
enable_monitoring = true
enable_memory_leak_detection = true
"#;
        std::fs::write(&config_path, config_content).unwrap();

        let reloader = ConfigReloader::builder()
            .with_config_path(&config_path)
            .with_reload_interval(Duration::from_millis(100))
            .build()
            .await
            .unwrap();

        assert_eq!(reloader.config_path(), &config_path);
        assert_eq!(reloader.reload_interval(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_config_manager_static() {
        let config = Config::new().with_max_key_size(1024);
        let manager = ConfigManager::new(config.clone());

        let retrieved_config = manager.get_config().await;
        assert_eq!(retrieved_config.max_key_size, 1024);
        assert!(!manager.is_reloading_enabled());
    }

    #[tokio::test]
    async fn test_config_manager_with_reloading() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_string_lossy().to_string();

        // Write test configuration
        let config_content = r#"
max_key_size = 2048
max_value_size = 1048576
operation_timeout = 60
strict_key_validation = false
storage_mode = "HashMap"
enable_monitoring = true
enable_memory_leak_detection = true
"#;
        std::fs::write(&config_path, config_content).unwrap();

        let manager = ConfigManager::with_reloading(&config_path).await.unwrap();
        assert!(manager.is_reloading_enabled());
        assert!(!manager.is_reloading().await);

        let config = manager.get_config().await;
        assert_eq!(config.max_key_size, 2048);
    }

    #[tokio::test]
    async fn test_config_reload() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path().to_string_lossy().to_string();

        // Write initial configuration
        let initial_config = r#"
max_key_size = 1024
max_value_size = 1048576
operation_timeout = 30
strict_key_validation = true
storage_mode = "BTreeMap"
enable_monitoring = true
enable_memory_leak_detection = true
"#;
        std::fs::write(&config_path, initial_config).unwrap();

        let reloader = ConfigReloader::builder()
            .with_config_path(&config_path)
            .build()
            .await
            .unwrap();

        let initial_config = reloader.get_config().await;
        assert_eq!(initial_config.max_key_size, 1024);

        // Update configuration file
        let updated_config = r#"
max_key_size = 2048
max_value_size = 2097152
operation_timeout = 60
strict_key_validation = false
storage_mode = "HashMap"
enable_monitoring = true
enable_memory_leak_detection = true
"#;
        std::fs::write(&config_path, updated_config).unwrap();

        // Manually trigger reload
        let event = reloader.reload().await.unwrap();
        match event {
            ConfigChangeEvent::Reloaded(config) => {
                assert_eq!(config.max_key_size, 2048);
                assert_eq!(config.max_value_size, 2097152);
                assert!(!config.strict_key_validation);
            }
            _ => panic!("Expected Reloaded event"),
        }
    }
}
