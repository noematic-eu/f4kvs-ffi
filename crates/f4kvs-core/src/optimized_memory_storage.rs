//! Optimized memory storage implementation with performance enhancements

use super::{BTreeMapStorage, HashMapStorage};
use crate::config::StorageMode;
use crate::{Result, StorageEngine, StorageStats, Value};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Optimized memory storage with performance enhancements
pub struct OptimizedMemoryStorage {
    inner: Arc<dyn StorageEngine>,
    #[allow(dead_code)] // Mode field reserved for future storage mode optimizations
    mode: StorageMode,
    // Performance optimization: pre-allocated memory pools
    key_pool: Arc<DashMap<String, ()>>,
    value_pool: Arc<DashMap<String, Value>>,
}

impl Default for OptimizedMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl OptimizedMemoryStorage {
    /// Create a new optimized memory storage instance
    pub fn new() -> Self {
        Self::with_mode(StorageMode::HashMap) // Use HashMap for better performance
    }

    /// Create a new optimized memory storage instance with specified mode
    pub fn with_mode(mode: StorageMode) -> Self {
        let inner: Arc<dyn StorageEngine> = match mode {
            StorageMode::BTreeMap => Arc::new(BTreeMapStorage::new()),
            StorageMode::HashMap => Arc::new(HashMapStorage::new()),
        };

        Self {
            inner,
            mode,
            // Pre-allocate pools for better performance
            key_pool: Arc::new(DashMap::with_capacity(1000)),
            value_pool: Arc::new(DashMap::with_capacity(1000)),
        }
    }

    /// Get current memory usage for validation/testing
    pub async fn get_current_memory_usage(&self) -> u64 {
        match self.stats().await {
            Ok(stats) => stats.memory_usage,
            Err(_) => 0,
        }
    }

    /// Optimized batch put with memory pool utilization
    pub async fn optimized_batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        // Pre-warm the pools for better performance
        for (key, value) in &items {
            let key_clone = key.clone();
            self.key_pool.insert(key_clone.clone(), ());
            self.value_pool.insert(key_clone, value.clone());
        }

        // Use the underlying storage for actual persistence
        self.inner.batch_put(items).await
    }

    /// Optimized batch get with cache utilization
    pub async fn optimized_batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            // Check cache first
            if let Some(cached_value) = self.value_pool.get(&key) {
                results.push(Some(cached_value.clone()));
            } else {
                // Fall back to underlying storage
                match self.inner.get(&key).await? {
                    Some(value) => {
                        // Cache the result
                        self.value_pool.insert(key.clone(), value.clone());
                        results.push(Some(value));
                    }
                    None => results.push(None),
                }
            }
        }

        Ok(results)
    }

    /// Clear performance pools
    pub fn clear_pools(&self) {
        self.key_pool.clear();
        self.value_pool.clear();
    }

    /// Get pool statistics
    pub fn get_pool_stats(&self) -> (usize, usize) {
        (self.key_pool.len(), self.value_pool.len())
    }
}

#[async_trait]
impl StorageEngine for OptimizedMemoryStorage {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        // Check cache first for better performance
        if let Some(cached_value) = self.value_pool.get(key) {
            return Ok(Some(cached_value.clone()));
        }

        // Fall back to underlying storage
        match self.inner.get(key).await? {
            Some(value) => {
                // Cache the result for future access
                self.value_pool.insert(key.to_string(), value.clone());
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        // Update cache
        self.key_pool.insert(key.to_string(), ());
        self.value_pool.insert(key.to_string(), value.clone());

        // Persist to underlying storage
        self.inner.put(key, value).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Remove from cache
        self.key_pool.remove(key);
        self.value_pool.remove(key);

        // Remove from underlying storage
        self.inner.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        // Check cache first
        if self.key_pool.contains_key(key) {
            return Ok(true);
        }

        // Fall back to underlying storage
        self.inner.exists(key).await
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        self.inner.scan_prefix(prefix).await
    }

    async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        self.inner.scan_range(start, end).await
    }

    async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        self.inner.scan_prefix_pairs(prefix).await
    }

    async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        self.inner.scan_range_pairs(start, end).await
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        self.optimized_batch_put(items).await
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        self.optimized_batch_get(keys).await
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        // Remove from cache
        for key in &keys {
            self.key_pool.remove(key);
            self.value_pool.remove(key);
        }

        // Remove from underlying storage
        self.inner.batch_delete(keys).await
    }

    async fn stats(&self) -> Result<StorageStats> {
        self.inner.stats().await
    }

    async fn clear(&self) -> Result<()> {
        // Clear caches
        self.clear_pools();

        // Clear underlying storage
        self.inner.clear().await
    }

    async fn keys(&self) -> Result<Vec<String>> {
        self.inner.keys().await
    }

    async fn count(&self) -> Result<u64> {
        self.inner.count().await
    }

    async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        self.inner.count_prefix(prefix).await
    }

    async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        self.inner.count_range(start, end).await
    }

    async fn flush(&self) -> Result<()> {
        // Delegate to the inner storage engine
        self.inner.flush().await
    }
}

/// High-performance concurrent storage using DashMap
pub struct ConcurrentHashMapStorage {
    data: Arc<DashMap<String, Value>>,
    stats: Arc<RwLock<StorageStats>>,
}

impl Default for ConcurrentHashMapStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl ConcurrentHashMapStorage {
    /// Create a new concurrent HashMap storage instance
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(StorageStats {
                key_count: 0,
                memory_usage: 0,
                total_operations: 0,
                get_operations: 0,
                put_operations: 0,
                delete_operations: 0,
                scan_operations: 0,
                average_key_size: 0.0,
                average_value_size: 0.0,
                peak_memory_usage: 0,
                cache_hits: 0,
                cache_misses: 0,
            })),
        }
    }

    async fn update_stats<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut StorageStats),
    {
        let mut stats = self.stats.write().await;
        f(&mut stats);
        Ok(())
    }
}

#[async_trait]
impl StorageEngine for ConcurrentHashMapStorage {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        let result = self.data.get(key).map(|entry| entry.clone());
        self.update_stats(|stats| stats.get_operations += 1).await?;
        Ok(result)
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.data.insert(key.to_string(), value.clone());
        self.update_stats(|stats| {
            stats.put_operations += 1;
            stats.total_operations += 1;
        })
        .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let removed = self.data.remove(key).is_some();
        if removed {
            self.update_stats(|stats| {
                stats.delete_operations += 1;
                stats.total_operations += 1;
            })
            .await?;
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.data.contains_key(key))
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        for entry in self.data.iter() {
            if entry.key().starts_with(prefix) {
                keys.push(entry.key().clone());
            }
        }
        keys.sort();
        Ok(keys)
    }

    async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        for entry in self.data.iter() {
            let key_str = entry.key().as_str();
            if key_str >= start && key_str < end {
                keys.push(entry.key().clone());
            }
        }
        keys.sort();
        Ok(keys)
    }

    async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        // Optimize: collect into pre-allocated vector and use more efficient iteration
        let mut pairs: Vec<(String, Value)> = Vec::new();
        for entry in self.data.iter() {
            if entry.key().starts_with(prefix) {
                pairs.push((entry.key().clone(), entry.value().clone()));
            }
        }
        // Only sort if needed (can be optimized further with index-based scanning)
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(pairs)
    }

    async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        // Optimize: collect into pre-allocated vector
        let mut pairs: Vec<(String, Value)> = Vec::new();
        for entry in self.data.iter() {
            let key_str = entry.key().as_str();
            if key_str >= start && key_str < end {
                pairs.push((entry.key().clone(), entry.value().clone()));
            }
        }
        // Only sort if needed (can be optimized further with index-based scanning)
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(pairs)
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        // Optimize: reduce allocations by moving values directly
        for (key, value) in items {
            self.data.insert(key, value);
        }
        self.update_stats(|stats| {
            stats.put_operations += 1;
            stats.total_operations += 1;
        })
        .await?;
        Ok(())
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let results: Vec<Option<Value>> = keys
            .into_iter()
            .map(|key| self.data.get(&key).map(|entry| entry.clone()))
            .collect();
        self.update_stats(|stats| stats.get_operations += 1).await?;
        Ok(results)
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        let deleted = keys
            .into_iter()
            .filter(|key| self.data.remove(key).is_some())
            .count();
        if deleted > 0 {
            self.update_stats(|stats| {
                stats.delete_operations += 1;
                stats.total_operations += 1;
            })
            .await?;
        }
        Ok(())
    }

    async fn stats(&self) -> Result<StorageStats> {
        let mut stats = self.stats.read().await.clone();
        stats.key_count = self.data.len() as u64;
        stats.memory_usage = (self.data.len() * 64) as u64; // Rough estimate
        Ok(stats)
    }

    async fn clear(&self) -> Result<()> {
        self.data.clear();
        self.update_stats(|stats| {
            *stats = StorageStats {
                key_count: 0,
                memory_usage: 0,
                total_operations: 0,
                get_operations: 0,
                put_operations: 0,
                delete_operations: 0,
                scan_operations: 0,
                average_key_size: 0.0,
                average_value_size: 0.0,
                peak_memory_usage: 0,
                cache_hits: 0,
                cache_misses: 0,
            };
        })
        .await?;
        Ok(())
    }

    async fn keys(&self) -> Result<Vec<String>> {
        let mut keys: Vec<String> = self.data.iter().map(|entry| entry.key().clone()).collect();
        keys.sort();
        Ok(keys)
    }

    async fn count(&self) -> Result<u64> {
        Ok(self.data.len() as u64)
    }

    async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        let count = self
            .data
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .count();
        Ok(count as u64)
    }

    async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        let count = self
            .data
            .iter()
            .filter(|entry| {
                let key_str = entry.key().as_str();
                key_str >= start && key_str < end
            })
            .count();
        Ok(count as u64)
    }

    async fn flush(&self) -> Result<()> {
        // For in-memory ConcurrentHashMap storage, flush is a no-op
        // All data is already in memory and will be lost on process exit
        // This is expected behavior for in-memory storage
        Ok(())
    }
}
