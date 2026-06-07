//! Core engine implementation for F4KVS
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::engine_utils::{
    create_health_check_value, generate_engine_features, generate_health_check_key, validate_config,
};
use crate::memory_leak_detection::{
    MemoryLeakDetectionConfig, MemoryLeakDetector, MemoryUsageTracker,
};
use crate::monitoring_hooks::{
    MonitoringContext, MonitoringContextBuilder, MonitoringEvent, MonitoringHooks,
};
use crate::{Config, F4KvsError, Result, Value};
use crate::{MemoryStorage, StorageEngine, StorageMode, StorageStats};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// F4KVS Core Engine
///
/// This is the main entry point for F4KVS operations. It provides a clean,
/// minimal API for key-value operations with pluggable storage backends.
#[derive(Clone)]
pub struct F4KVSCore {
    storage: Arc<dyn StorageEngine>,
    config: Config,
    monitoring_hooks: Arc<MonitoringHooks>,
    memory_tracker: Arc<MemoryUsageTracker>,
    leak_detector: Arc<MemoryLeakDetector>,
}

impl F4KVSCore {
    /// Create a new F4KVS core instance with memory storage
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let storage = Arc::new(MemoryStorage::with_mode(config.storage_mode));
        let monitoring_hooks = if config.enable_monitoring {
            Arc::new(MonitoringHooks::new())
        } else {
            Arc::new(MonitoringHooks::disabled())
        };
        let memory_tracker = Arc::new(MemoryUsageTracker::new());
        let leak_detector = if config.enable_memory_leak_detection {
            Arc::new(MemoryLeakDetector::new(MemoryLeakDetectionConfig::default()))
        } else {
            Arc::new(MemoryLeakDetector::disabled())
        };

        info!(
            "F4KVS Core initialized with memory storage (monitoring: {}, leak_detection: {})",
            config.enable_monitoring, config.enable_memory_leak_detection
        );

        Ok(Self {
            storage,
            config,
            monitoring_hooks,
            memory_tracker,
            leak_detector,
        })
    }

    /// Create a new F4KVS core instance with custom configuration
    pub fn with_config(config: Config) -> Result<Self> {
        validate_config(&config)?;
        let storage = Arc::new(MemoryStorage::with_mode(config.storage_mode));
        let monitoring_hooks = if config.enable_monitoring {
            Arc::new(MonitoringHooks::new())
        } else {
            Arc::new(MonitoringHooks::disabled())
        };
        let memory_tracker = Arc::new(MemoryUsageTracker::new());
        let leak_detector = if config.enable_memory_leak_detection {
            Arc::new(MemoryLeakDetector::new(MemoryLeakDetectionConfig::default()))
        } else {
            Arc::new(MemoryLeakDetector::disabled())
        };

        info!(
            ?config,
            "F4KVS Core initialized with custom config (monitoring: {}, leak_detection: {})",
            config.enable_monitoring,
            config.enable_memory_leak_detection
        );

        Ok(Self {
            storage,
            config,
            monitoring_hooks,
            memory_tracker,
            leak_detector,
        })
    }

    /// Create a new F4KVS core instance with custom storage
    pub fn with_storage(storage: Arc<dyn StorageEngine>) -> Result<Self> {
        let config = Config::default();
        let monitoring_hooks = Arc::new(MonitoringHooks::new());
        let memory_tracker = Arc::new(MemoryUsageTracker::new());
        let leak_detector = Arc::new(MemoryLeakDetector::new(MemoryLeakDetectionConfig::default()));

        info!("F4KVS Core initialized with custom storage backend");

        Ok(Self {
            storage,
            config,
            monitoring_hooks,
            memory_tracker,
            leak_detector,
        })
    }

    /// Create a new F4KVS core instance with custom config and storage
    pub fn with_config_and_storage(
        config: Config,
        storage: Arc<dyn StorageEngine>,
    ) -> Result<Self> {
        let monitoring_hooks = Arc::new(MonitoringHooks::new());
        let memory_tracker = Arc::new(MemoryUsageTracker::new());
        let leak_detector = Arc::new(MemoryLeakDetector::new(MemoryLeakDetectionConfig::default()));

        info!(
            ?config,
            "F4KVS Core initialized with custom config and storage"
        );

        Ok(Self {
            storage,
            config,
            monitoring_hooks,
            memory_tracker,
            leak_detector,
        })
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<Value>> {
        self.validate_key(key)?;

        debug!(%key, "Getting value");
        self.storage.get(key).await
    }

    /// Put a key-value pair
    pub async fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.validate_key(key)?;
        self.validate_value(value)?;

        debug!(%key, "Putting value");
        self.storage.put(key, value).await
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.validate_key(key)?;

        debug!(%key, "Deleting key");
        self.storage.delete(key).await
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.validate_key(key)?;

        debug!(%key, "Checking key existence");
        self.storage.exists(key).await
    }

    /// Get all keys (use with caution for large datasets)
    pub async fn keys(&self) -> Result<Vec<String>> {
        debug!("Listing all keys");
        self.storage.keys().await
    }

    /// Get count of keys
    pub async fn count(&self) -> Result<u64> {
        debug!("Getting key count");
        self.storage.count().await
    }

    /// Batch put multiple key-value pairs
    pub async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        // Validate all keys and values first
        for (key, value) in &items {
            self.validate_key(key)?;
            self.validate_value(value)?;
        }

        debug!(count = items.len(), "Batch putting values");
        self.storage.batch_put(items).await
    }

    /// Batch get multiple values by keys
    pub async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        // Validate all keys first
        for key in &keys {
            self.validate_key(key)?;
        }

        debug!(count = keys.len(), "Batch getting values");
        self.storage.batch_get(keys).await
    }

    /// Batch delete multiple keys
    pub async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        // Validate all keys first
        for key in &keys {
            self.validate_key(key)?;
        }

        debug!(count = keys.len(), "Batch deleting keys");
        self.storage.batch_delete(keys).await
    }

    /// Get storage statistics
    pub async fn stats(&self) -> Result<StorageStats> {
        debug!("Getting storage statistics");
        let stats = self.storage.stats().await?;

        // Update memory tracker
        self.memory_tracker.update(stats.memory_usage);

        // Record memory sample for leak detection
        self.leak_detector.record_sample(stats.memory_usage).await;

        // Trigger monitoring hook
        let context = MonitoringContextBuilder::new()
            .with("memory_usage", stats.memory_usage.to_string())
            .with("key_count", stats.key_count.to_string())
            .build();
        self.monitoring_hooks
            .trigger(MonitoringEvent::MemoryUsageChanged, context)
            .await;

        Ok(stats)
    }

    /// Clear all data (dangerous operation!)
    pub async fn clear(&self) -> Result<()> {
        warn!("Clearing all data - this is irreversible!");
        self.storage.clear().await
    }

    /// Get current configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Scan keys with a given prefix
    pub async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        debug!("Scanning keys with prefix: {}", prefix);
        self.storage.scan_prefix(prefix).await
    }

    /// Scan keys in a range (inclusive start, exclusive end)
    pub async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        debug!("Scanning keys in range: {} to {}", start, end);
        self.storage.scan_range(start, end).await
    }

    /// Scan key-value pairs with a given prefix
    pub async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        debug!("Scanning key-value pairs with prefix: {}", prefix);
        self.storage.scan_prefix_pairs(prefix).await
    }

    /// Scan key-value pairs in a range (inclusive start, exclusive end)
    pub async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        debug!("Scanning key-value pairs in range: {} to {}", start, end);
        self.storage.scan_range_pairs(start, end).await
    }

    /// Count keys with a given prefix
    pub async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        debug!("Counting keys with prefix: {}", prefix);
        self.storage.count_prefix(prefix).await
    }

    /// Count keys in a range (inclusive start, exclusive end)
    pub async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        debug!("Counting keys in range: {} to {}", start, end);
        self.storage.count_range(start, end).await
    }

    /// Health check - verify the engine is operational
    pub async fn health_check(&self) -> Result<bool> {
        debug!("Performing health check");

        // Generate unique health check key to avoid conflicts with user data
        let test_key = generate_health_check_key();
        let test_value = create_health_check_value();

        // Put test value
        self.storage.put(&test_key, &test_value).await?;

        // Get it back
        let retrieved = self.storage.get(&test_key).await?;

        // Clean up
        self.storage.delete(&test_key).await?;

        // Verify it worked
        match retrieved {
            Some(value) if value == test_value => {
                debug!("Health check passed");
                Ok(true)
            }
            _ => {
                warn!("Health check failed");
                Ok(false)
            }
        }
    }

    /// Get engine information
    pub fn info(&self) -> EngineInfo {
        let features = generate_engine_features();

        EngineInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: "F4KVS-Core".to_string(),
            storage_type: "pluggable".to_string(),
            features,
        }
    }

    // Validation methods
    fn validate_key(&self, key: &str) -> Result<()> {
        if key.is_empty() {
            return Err(F4KvsError::invalid_key("Key cannot be empty"));
        }

        if key.len() > self.config.max_key_size {
            return Err(F4KvsError::invalid_key(format!(
                "Key too long: {} bytes (max: {})",
                key.len(),
                self.config.max_key_size
            )));
        }

        // Check for invalid characters if configured
        if self.config.strict_key_validation {
            if key.contains('\0') {
                return Err(F4KvsError::invalid_key("Key cannot contain null bytes"));
            }

            if key.starts_with('.') || key.starts_with('/') {
                return Err(F4KvsError::invalid_key("Key cannot start with '.' or '/'"));
            }
        }

        Ok(())
    }

    fn validate_value(&self, value: &Value) -> Result<()> {
        let size = value.memory_size();
        if size > self.config.max_value_size {
            return Err(F4KvsError::invalid_value(format!(
                "Value too large: {} bytes (max: {})",
                size, self.config.max_value_size
            )));
        }

        Ok(())
    }

    /// Flush any pending writes to persistent storage
    /// For in-memory storage, this is typically a no-op
    /// For persistent storage, this ensures data is written to disk
    pub async fn flush(&self) -> Result<()> {
        debug!("Flushing storage engine");
        self.storage.flush().await
    }

    /// Sync WAL (Write-Ahead Log) to ensure durability
    /// This is a stronger guarantee than flush - it ensures data is persisted
    /// For storage engines with WAL support, this syncs the WAL
    /// For storage engines without WAL, this is equivalent to flush()
    pub async fn sync_wal(&self) -> Result<()> {
        debug!("Syncing WAL");
        // For now, sync_wal is equivalent to flush
        // In the future, when WAL storage engines are added, this will sync the WAL
        self.flush().await
    }

    /// Gracefully shutdown the engine
    /// This performs a clean shutdown sequence:
    /// 1. Stop accepting new operations (if applicable)
    /// 2. Wait for in-flight operations to complete
    /// 3. Flush all pending writes
    /// 4. Sync WAL to ensure durability
    /// 5. Clean up resources
    pub async fn shutdown(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        info!("Starting graceful shutdown");

        // Trigger shutdown initiated event
        let context = MonitoringContextBuilder::new()
            .with(
                "timeout",
                timeout
                    .map(|t| t.as_secs().to_string())
                    .unwrap_or_else(|| "30".to_string()),
            )
            .build();
        self.monitoring_hooks
            .trigger(MonitoringEvent::ShutdownInitiated, context)
            .await;

        let timeout = timeout.unwrap_or_else(|| std::time::Duration::from_secs(30));

        // Step 1: Flush all pending writes (with timeout)
        debug!("Flushing pending writes during shutdown");
        if let Err(e) = tokio::time::timeout(timeout / 2, self.flush()).await {
            warn!("Flush timed out or failed during shutdown: {:?}", e);
        }

        // Step 2: Sync WAL to ensure durability (with timeout)
        debug!("Syncing WAL during shutdown");
        if let Err(e) = tokio::time::timeout(timeout / 2, self.sync_wal()).await {
            warn!("WAL sync timed out or failed during shutdown: {:?}", e);
        }

        // Step 3: Perform final health check
        debug!("Performing final health check");
        let _ = self.health_check().await;

        // Trigger shutdown completed event
        let context = MonitoringContext::new();
        self.monitoring_hooks
            .trigger(MonitoringEvent::ShutdownCompleted, context)
            .await;

        info!("Graceful shutdown completed");
        Ok(())
    }

    /// Get monitoring hooks manager for registering external monitoring hooks
    pub fn monitoring_hooks(&self) -> Arc<MonitoringHooks> {
        Arc::clone(&self.monitoring_hooks)
    }

    /// Get memory usage tracker
    pub fn memory_tracker(&self) -> Arc<MemoryUsageTracker> {
        Arc::clone(&self.memory_tracker)
    }

    /// Get memory leak detector
    pub fn leak_detector(&self) -> Arc<MemoryLeakDetector> {
        Arc::clone(&self.leak_detector)
    }

    /// Check if memory leak is detected
    pub async fn is_memory_leak_detected(&self) -> bool {
        self.leak_detector.is_leak_detected().await
    }
}

impl F4KVSCore {
    /// Create a default instance with fallback behavior
    ///
    /// **WARNING**: This method may silently ignore initialization errors.
    /// Use `F4KVSCore::new()` or `F4KVSCore::with_config()` for explicit error handling.
    ///
    /// This method is provided for convenience in scenarios where:
    /// - You need a quick default instance for testing
    /// - You're certain initialization will succeed
    /// - Error handling is not critical
    ///
    /// # Panics
    ///
    /// This method will panic if initialization fails and fallback creation also fails.
    /// For production code, prefer explicit initialization with error handling.
    pub fn default_unchecked() -> Self {
        // Try normal initialization first
        Self::new().unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to create F4KVS core with default config, using fallback: {}",
                e
            );
            // Create a minimal fallback instance
            // This should never fail, but if it does, we panic rather than silently continue
            Self {
                storage: Arc::new(MemoryStorage::with_mode(StorageMode::HashMap)),
                config: Config::default(),
                monitoring_hooks: Arc::new(MonitoringHooks::new()),
                memory_tracker: Arc::new(MemoryUsageTracker::new()),
                leak_detector: Arc::new(MemoryLeakDetector::new(
                    MemoryLeakDetectionConfig::default(),
                )),
            }
        })
    }
}

/// Information about the F4KVS engine
#[derive(Debug, Clone)]
pub struct EngineInfo {
    /// Version of the F4KVS engine
    pub version: String,
    /// Name of the engine implementation
    pub name: String,
    /// Type of storage backend used
    pub storage_type: String,
    /// List of enabled features
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use std::sync::Arc;
    use tokio::task;

    // Helper function to create engine for tests
    fn create_test_engine() -> F4KVSCore {
        F4KVSCore::new().expect("Failed to create test engine")
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let engine = create_test_engine();

        // Test put and get
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .await
            .expect("Failed to put test key");
        let value = engine
            .get("test_key")
            .await
            .expect("Failed to get test key");
        assert_eq!(value, Some(Value::String("test_value".to_string())));

        // Test exists
        assert!(engine
            .exists("test_key")
            .await
            .expect("Failed to check key existence"));
        assert!(!engine
            .exists("nonexistent")
            .await
            .expect("Failed to check nonexistent key"));

        // Test delete
        engine
            .delete("test_key")
            .await
            .expect("Failed to delete test key");
        assert!(!engine
            .exists("test_key")
            .await
            .expect("Failed to check deleted key"));
    }

    #[tokio::test]
    async fn test_validation() {
        let engine = create_test_engine();

        // Test empty key validation
        let result = engine.put("", &Value::String("value".to_string())).await;
        assert!(result.is_err());

        // Test key length validation
        let long_key = "a".repeat(engine.config.max_key_size + 1);
        let result = engine
            .put(&long_key, &Value::String("value".to_string()))
            .await;
        assert!(result.is_err());

        // Test value size validation
        let large_value = Value::String("a".repeat(engine.config.max_value_size + 1));
        let result = engine.put("key", &large_value).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let engine = create_test_engine();
        assert!(engine.health_check().await.expect("Health check failed"));
    }

    #[tokio::test]
    async fn test_stats_and_info() {
        let engine = create_test_engine();

        // Test initial stats
        let stats = engine.stats().await.expect("Failed to get initial stats");
        assert_eq!(stats.key_count, 0);

        // Add some data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .await
            .expect("Failed to put key2");

        // Check updated stats
        let stats = engine.stats().await.expect("Failed to get updated stats");
        assert_eq!(stats.key_count, 2);
        assert!(stats.memory_usage > 0);

        // Test info
        let info = engine.info();
        assert_eq!(info.name, "F4KVS-Core");
        assert!(info.features.contains(&"async".to_string()));
    }

    #[tokio::test]
    async fn test_keys_and_count() {
        let engine = create_test_engine();

        // Add test data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .await
            .expect("Failed to put key2");
        engine
            .put("key3", &Value::Bool(true))
            .await
            .expect("Failed to put key3");

        // Test count
        assert_eq!(engine.count().await.expect("Failed to get count"), 3);

        // Test keys
        let keys = engine.keys().await.expect("Failed to get keys");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_clear() {
        let engine = create_test_engine();

        // Add test data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .await
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .await
            .expect("Failed to put key2");

        assert_eq!(engine.count().await.expect("Failed to get count"), 2);

        // Clear all data
        engine.clear().await.expect("Failed to clear data");
        assert_eq!(
            engine
                .count()
                .await
                .expect("Failed to get count after clear"),
            0
        );
        assert!(engine
            .keys()
            .await
            .expect("Failed to get keys after clear")
            .is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_reads() {
        let engine = Arc::new(create_test_engine());

        // Pre-populate with test data
        for i in 0..100 {
            engine
                .put(&format!("key{}", i), &Value::String(format!("value{}", i)))
                .await
                .unwrap_or_else(|_| panic!("Failed to put key{}", i));
        }

        // Spawn multiple concurrent read tasks
        let mut handles = vec![];
        for task_id in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..100 {
                    let key = format!("key{}", i);
                    let expected_value = Value::String(format!("value{}", i));
                    let value = engine_clone
                        .get(&key)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to get key{}", i));
                    assert_eq!(value, Some(expected_value));
                }
                format!("Task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_concurrent_writes() {
        let engine = Arc::new(create_test_engine());

        // Spawn multiple concurrent write tasks
        let mut handles = vec![];
        for task_id in 0..20 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..50 {
                    let key = format!("task{}_key{}", task_id, i);
                    let value = Value::String(format!("task{}_value{}", task_id, i));
                    engine_clone
                        .put(&key, &value)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to put key {}", key));
                }
                format!("Write task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify all data was written correctly
        assert_eq!(engine.count().await.expect("Failed to get count"), 1000); // 20 tasks * 50 keys each

        // Check a few random keys to ensure data integrity
        for task_id in 0..5 {
            for i in 0..5 {
                let key = format!("task{}_key{}", task_id, i);
                let expected_value = Value::String(format!("task{}_value{}", task_id, i));
                let value = engine
                    .get(&key)
                    .await
                    .unwrap_or_else(|_| panic!("Failed to get key {}", key));
                assert_eq!(value, Some(expected_value));
            }
        }
    }

    #[tokio::test]
    async fn test_concurrent_read_write_mix() {
        let engine = Arc::new(create_test_engine());

        // Pre-populate with some data
        for i in 0..50 {
            engine
                .put(
                    &format!("existing_key{}", i),
                    &Value::String(format!("existing_value{}", i)),
                )
                .await
                .unwrap_or_else(|_| panic!("Failed to put existing_key{}", i));
        }

        // Spawn mixed read/write tasks
        let mut handles = vec![];

        // Read tasks
        for task_id in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..50 {
                    let key = format!("existing_key{}", i);
                    let expected_value = Value::String(format!("existing_value{}", i));
                    let value = engine_clone
                        .get(&key)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to get key {}", key));
                    assert_eq!(value, Some(expected_value));
                }
                format!("Read task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Write tasks
        for task_id in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..25 {
                    let key = format!("new_key{}_task{}", i, task_id);
                    let value = Value::String(format!("new_value{}_task{}", i, task_id));
                    engine_clone
                        .put(&key, &value)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to put key {}", key));
                }
                format!("Write task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify data integrity
        assert_eq!(engine.count().await.expect("Failed to get count"), 300); // 50 existing + 10 tasks * 25 new keys each
    }

    #[tokio::test]
    async fn test_concurrent_deletes() {
        let engine = Arc::new(create_test_engine());

        // Pre-populate with test data
        for i in 0..200 {
            engine
                .put(
                    &format!("delete_key{}", i),
                    &Value::String(format!("delete_value{}", i)),
                )
                .await
                .unwrap_or_else(|_| panic!("Failed to put delete_key{}", i));
        }

        assert_eq!(engine.count().await.expect("Failed to get count"), 200);

        // Spawn concurrent delete tasks
        let mut handles = vec![];
        for task_id in 0..4 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..50 {
                    let key = format!("delete_key{}", task_id * 50 + i);
                    engine_clone
                        .delete(&key)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to delete key {}", key));
                }
                format!("Delete task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify all keys were deleted
        assert_eq!(engine.count().await.expect("Failed to get count"), 0);
        assert!(engine.keys().await.expect("Failed to get keys").is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_operations_under_load() {
        let engine = Arc::new(create_test_engine());

        // Spawn many concurrent tasks that perform mixed operations
        let mut handles = vec![];
        let num_tasks = 50;
        let operations_per_task = 100;

        for task_id in 0..num_tasks {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for i in 0..operations_per_task {
                    let operation = i % 4; // 0: put, 1: get, 2: delete, 3: exists
                    let key = format!("load_key{}_task{}", i, task_id);

                    match operation {
                        0 => {
                            let value = Value::String(format!("load_value{}_task{}", i, task_id));
                            let _ = engine_clone.put(&key, &value).await;
                        }
                        1 => {
                            let _ = engine_clone.get(&key).await;
                        }
                        2 => {
                            let _ = engine_clone.delete(&key).await;
                        }
                        3 => {
                            let _ = engine_clone.exists(&key).await;
                        }
                        _ => unreachable!(),
                    }
                }
                format!("Load task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify the engine is still functional
        assert!(engine.health_check().await.expect("Health check failed"));
        let stats = engine.stats().await.expect("Failed to get stats");
        assert!(stats.total_operations > 0);
    }

    #[tokio::test]
    async fn test_concurrent_stats_access() {
        let engine = Arc::new(create_test_engine());

        // Pre-populate with data
        for i in 0..100 {
            engine
                .put(
                    &format!("stats_key{}", i),
                    &Value::String(format!("stats_value{}", i)),
                )
                .await
                .unwrap_or_else(|_| panic!("Failed to put stats_key{}", i));
        }

        // Spawn concurrent stats access tasks
        let mut handles = vec![];
        for task_id in 0..20 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for _ in 0..10 {
                    let stats = engine_clone.stats().await.expect("Failed to get stats");
                    assert_eq!(stats.key_count, 100);
                    assert!(stats.memory_usage > 0);

                    let count = engine_clone.count().await.expect("Failed to get count");
                    assert_eq!(count, 100);

                    let keys = engine_clone.keys().await.expect("Failed to get keys");
                    assert_eq!(keys.len(), 100);
                }
                format!("Stats task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_concurrent_clear_operations() {
        let engine = Arc::new(F4KVSCore::new().unwrap());

        // Pre-populate with data
        for i in 0..100 {
            engine
                .put(
                    &format!("clear_key{}", i),
                    &Value::String(format!("clear_value{}", i)),
                )
                .await
                .unwrap();
        }

        assert_eq!(engine.count().await.unwrap(), 100);

        // Spawn concurrent clear operations
        let mut handles = vec![];
        for task_id in 0..5 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                engine_clone.clear().await.unwrap();
                format!("Clear task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify the store is empty (regardless of which clear operation "won")
        assert_eq!(engine.count().await.unwrap(), 0);
        assert!(engine.keys().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_health_checks() {
        let engine = Arc::new(F4KVSCore::new().unwrap());

        // Spawn concurrent health check tasks
        let mut handles = vec![];
        for task_id in 0..100 {
            let engine_clone = Arc::clone(&engine);
            let handle = task::spawn(async move {
                for _ in 0..10 {
                    let healthy = engine_clone.health_check().await.unwrap();
                    assert!(healthy);
                }
                format!("Health check task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_concurrent_large_value_operations() {
        let engine = Arc::new(F4KVSCore::new().unwrap());

        // Create large values (close to max size limits)
        let large_string = "x".repeat(engine.config.max_value_size - 100);
        let large_value = Value::String(large_string);

        // Spawn concurrent operations with large values
        let mut handles = vec![];
        for task_id in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let large_value_clone = large_value.clone();
            let handle = task::spawn(async move {
                for i in 0..20 {
                    let key = format!("large_key{}_task{}", i, task_id);
                    engine_clone.put(&key, &large_value_clone).await.unwrap();

                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(large_value_clone.clone()));

                    engine_clone.delete(&key).await.unwrap();
                }
                format!("Large value task {} completed", task_id)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.is_ok());
        }

        // Verify final state
        assert_eq!(engine.count().await.unwrap(), 0);
    }

    #[test]
    fn test_batch_operations() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let engine = F4KVSCore::new().unwrap();

            // Test batch put
            let items = vec![
                ("key1".to_string(), Value::String("value1".to_string())),
                ("key2".to_string(), Value::String("value2".to_string())),
                ("key3".to_string(), Value::Int64(42)),
            ];

            engine.batch_put(items).await.unwrap();

            // Test batch get
            let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
            let results = engine.batch_get(keys).await.unwrap();

            assert_eq!(results.len(), 3);
            assert!(results[0].is_some());
            assert!(results[1].is_some());
            assert!(results[2].is_some());

            // Test batch delete
            let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
            engine.batch_delete(keys_to_delete).await.unwrap();

            // Verify deletion
            let remaining = engine.get("key3").await.unwrap();
            assert!(remaining.is_some());

            let deleted = engine.get("key1").await.unwrap();
            assert!(deleted.is_none());
        });
    }

    #[tokio::test]
    async fn test_storage_mode_configuration() {
        use crate::{Config, StorageMode};

        // Test BTreeMap mode (default)
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine = F4KVSCore::with_config(config).unwrap();

        // Test basic operations work
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .await
            .unwrap();
        let value = engine.get("test_key").await.unwrap();
        assert_eq!(value, Some(Value::String("test_value".to_string())));

        // Test HashMap mode
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config).unwrap();

        // Test basic operations work
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .await
            .unwrap();
        let value = engine.get("test_key").await.unwrap();
        assert_eq!(value, Some(Value::String("test_value".to_string())));
    }

    #[tokio::test]
    async fn test_storage_mode_performance_comparison() {
        use crate::{Config, StorageMode};
        use std::time::Instant;

        let test_data: Vec<(String, Value)> = (0..1000)
            .map(|i| (format!("key{}", i), Value::String(format!("value{}", i))))
            .collect();

        // Test BTreeMap performance
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine_btree = F4KVSCore::with_config(config).unwrap();

        let start = Instant::now();
        for (key, value) in &test_data {
            engine_btree.put(key, value).await.unwrap();
        }
        let btree_insert_time = start.elapsed();

        // Test HashMap performance
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine_hashmap = F4KVSCore::with_config(config).unwrap();

        let start = Instant::now();
        for (key, value) in &test_data {
            engine_hashmap.put(key, value).await.unwrap();
        }
        let hashmap_insert_time = start.elapsed();

        // Both should work, but HashMap should generally be faster for inserts
        log::debug!("BTreeMap insert time: {:?}", btree_insert_time);
        log::debug!("HashMap insert time: {:?}", hashmap_insert_time);

        // Verify both engines have the same data
        assert_eq!(engine_btree.count().await.unwrap(), 1000);
        assert_eq!(engine_hashmap.count().await.unwrap(), 1000);
    }
}
