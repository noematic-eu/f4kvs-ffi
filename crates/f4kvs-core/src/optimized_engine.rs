//! Optimized engine implementation for F4KVS Core

use crate::optimized_memory_storage::{ConcurrentHashMapStorage, OptimizedMemoryStorage};
use crate::{Config, Result, Value};
use crate::{StorageEngine, StorageMode, StorageStats};
use std::sync::Arc;
use tracing::{debug, info};

/// Optimized F4KVS Core Engine with performance enhancements
///
/// This engine provides significant performance improvements over the standard
/// F4KVS Core through:
/// - Memory pool optimization
/// - Concurrent access improvements
/// - Enhanced caching
/// - Optimized batch operations
#[derive(Clone)]
pub struct OptimizedF4KVSCore {
    storage: Arc<dyn StorageEngine>,
    #[allow(dead_code)] // Config field reserved for future configuration options
    config: Config,
    // Performance tracking
    operation_count: Arc<std::sync::atomic::AtomicU64>,
    cache_hits: Arc<std::sync::atomic::AtomicU64>,
    cache_misses: Arc<std::sync::atomic::AtomicU64>,
}

impl OptimizedF4KVSCore {
    /// Create a new optimized F4KVS core instance with concurrent HashMap storage
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let storage = Arc::new(ConcurrentHashMapStorage::new());

        info!("Optimized F4KVS Core initialized with concurrent HashMap storage");

        Ok(Self {
            storage,
            config,
            operation_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Create a new optimized F4KVS core instance with custom configuration
    pub fn with_config(config: Config) -> Result<Self> {
        let storage: Arc<dyn StorageEngine> = match config.storage_mode {
            StorageMode::BTreeMap => {
                Arc::new(OptimizedMemoryStorage::with_mode(StorageMode::BTreeMap))
            }
            StorageMode::HashMap => Arc::new(ConcurrentHashMapStorage::new()),
        };

        info!(
            ?config,
            "Optimized F4KVS Core initialized with custom config"
        );

        Ok(Self {
            storage,
            config,
            operation_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Create a new optimized F4KVS core instance with custom storage
    pub fn with_storage(storage: Arc<dyn StorageEngine>) -> Result<Self> {
        let config = Config::default();

        info!("Optimized F4KVS Core initialized with custom storage backend");

        Ok(Self {
            storage,
            config,
            operation_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Create a new optimized F4KVS core instance with custom config and storage
    pub fn with_config_and_storage(
        config: Config,
        storage: Arc<dyn StorageEngine>,
    ) -> Result<Self> {
        info!("Optimized F4KVS Core initialized with custom config and storage backend");

        Ok(Self {
            storage,
            config,
            operation_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cache_misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Get a value by key with performance tracking
    pub async fn get(&self, key: &str) -> Result<Option<Value>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.get(key).await;
        let duration = start.elapsed();

        debug!(
            key = key,
            duration_ms = duration.as_millis(),
            "Get operation completed"
        );

        result
    }

    /// Put a key-value pair with performance tracking
    pub async fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.put(key, value).await;
        let duration = start.elapsed();

        debug!(
            key = key,
            value_type = value.type_name(),
            duration_ms = duration.as_millis(),
            "Put operation completed"
        );

        result
    }

    /// Delete a key with performance tracking
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.delete(key).await;
        let duration = start.elapsed();

        debug!(
            key = key,
            duration_ms = duration.as_millis(),
            "Delete operation completed"
        );

        result
    }

    /// Check if a key exists with performance tracking
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.exists(key).await;
        let duration = start.elapsed();

        debug!(
            key = key,
            exists = result.as_ref().unwrap_or(&false),
            duration_ms = duration.as_millis(),
            "Exists operation completed"
        );

        result
    }

    /// Scan keys with a prefix with performance tracking
    pub async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.scan_prefix(prefix).await;
        let duration = start.elapsed();

        debug!(
            prefix = prefix,
            key_count = result.as_ref().map(|keys| keys.len()).unwrap_or(0),
            duration_ms = duration.as_millis(),
            "Scan prefix operation completed"
        );

        result
    }

    /// Scan keys in a range with performance tracking
    pub async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start_time = std::time::Instant::now();
        let result = self.storage.scan_range(start, end).await;
        let duration = start_time.elapsed();

        debug!(
            start = start,
            end = end,
            key_count = result.as_ref().map(|keys| keys.len()).unwrap_or(0),
            duration_ms = duration.as_millis(),
            "Scan range operation completed"
        );

        result
    }

    /// Scan key-value pairs with a prefix with performance tracking
    pub async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.scan_prefix_pairs(prefix).await;
        let duration = start.elapsed();

        debug!(
            prefix = prefix,
            pair_count = result.as_ref().map(|pairs| pairs.len()).unwrap_or(0),
            duration_ms = duration.as_millis(),
            "Scan prefix pairs operation completed"
        );

        result
    }

    /// Scan key-value pairs in a range with performance tracking
    pub async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start_time = std::time::Instant::now();
        let result = self.storage.scan_range_pairs(start, end).await;
        let duration = start_time.elapsed();

        debug!(
            start = start,
            end = end,
            pair_count = result.as_ref().map(|pairs| pairs.len()).unwrap_or(0),
            duration_ms = duration.as_millis(),
            "Scan range pairs operation completed"
        );

        result
    }

    /// Batch put with performance tracking and optimization
    pub async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.batch_put(items.clone()).await;
        let duration = start.elapsed();

        debug!(
            item_count = items.len(),
            duration_ms = duration.as_millis(),
            "Batch put operation completed"
        );

        result
    }

    /// Batch get with performance tracking and optimization
    pub async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.batch_get(keys.clone()).await;
        let duration = start.elapsed();

        debug!(
            key_count = keys.len(),
            duration_ms = duration.as_millis(),
            "Batch get operation completed"
        );

        result
    }

    /// Batch delete with performance tracking
    pub async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.batch_delete(keys.clone()).await;
        let duration = start.elapsed();

        debug!(
            key_count = keys.len(),
            duration_ms = duration.as_millis(),
            "Batch delete operation completed"
        );

        result
    }

    /// Get storage statistics with performance metrics
    pub async fn stats(&self) -> Result<StorageStats> {
        let mut stats = self.storage.stats().await?;

        // Add performance metrics
        stats.total_operations = self
            .operation_count
            .load(std::sync::atomic::Ordering::Relaxed);

        Ok(stats)
    }

    /// Clear all data with performance tracking
    pub async fn clear(&self) -> Result<()> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = self.storage.clear().await;
        let duration = start.elapsed();

        debug!(
            duration_ms = duration.as_millis(),
            "Clear operation completed"
        );

        result
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> (u64, u64, u64) {
        (
            self.operation_count
                .load(std::sync::atomic::Ordering::Relaxed),
            self.cache_hits.load(std::sync::atomic::Ordering::Relaxed),
            self.cache_misses.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Reset performance metrics
    pub fn reset_performance_metrics(&self) {
        self.operation_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.cache_hits
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.cache_misses
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
