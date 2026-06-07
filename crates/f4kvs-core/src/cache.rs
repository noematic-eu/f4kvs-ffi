//! Caching layer for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{Result, StorageEngine, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    value: Value,
    created_at: Instant,
    access_count: u64,
    last_accessed: Instant,
}

impl CacheEntry {
    fn new(value: Value) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            access_count: 1,
            last_accessed: now,
        }
    }

    fn access(&mut self) -> &Value {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.value
    }

    fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    fn time_since_access(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

/// LRU Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in the cache
    pub max_entries: usize,
    /// Maximum age of entries before they're considered stale
    pub max_age: Duration,
    /// Maximum time since last access before eviction
    pub max_idle_time: Duration,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_age: Duration::from_secs(300),      // 5 minutes
            max_idle_time: Duration::from_secs(60), // 1 minute
            auto_cleanup: true,
            cleanup_interval: Duration::from_secs(30), // 30 seconds
        }
    }
}

/// LRU Cache implementation
pub struct LruCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
}

/// Statistics about cache performance and usage
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of entries evicted from cache
    pub evictions: u64,
    /// Current number of entries in cache
    pub entries: usize,
    /// Memory usage in bytes
    pub memory_usage: u64,
}

impl CacheStats {
    /// Calculate the cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl Default for LruCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LruCache {
    /// Create a new LRU cache with default configuration
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new LRU cache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get a value from the cache
    pub async fn get(&self, key: &str) -> Option<Value> {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(key) {
            // Check if entry is still valid
            if entry.age() < self.config.max_age
                && entry.time_since_access() < self.config.max_idle_time
            {
                let value = entry.access().clone();
                drop(entries);

                // Update stats
                self.update_stats(|stats| stats.hits += 1).await;
                return Some(value);
            } else {
                // Entry is stale, remove it
                entries.remove(key);
                self.update_stats(|stats| stats.evictions += 1).await;
            }
        }

        drop(entries);
        self.update_stats(|stats| stats.misses += 1).await;
        None
    }

    /// Put a value into the cache
    pub async fn put(&self, key: String, value: Value) {
        let mut entries = self.entries.write().await;

        // Check if we need to evict entries
        if entries.len() >= self.config.max_entries {
            self.evict_lru(&mut entries).await;
        }

        let entry = CacheEntry::new(value);
        entries.insert(key, entry);

        // Update stats
        self.update_stats(|stats| {
            stats.entries = entries.len();
            stats.memory_usage = self.calculate_memory_usage(&entries);
        })
        .await;
    }

    /// Remove a value from the cache
    pub async fn remove(&self, key: &str) -> Option<Value> {
        let mut entries = self.entries.write().await;
        let result = entries.remove(key).map(|entry| entry.value);

        self.update_stats(|stats| {
            stats.entries = entries.len();
            stats.memory_usage = self.calculate_memory_usage(&entries);
        })
        .await;

        result
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();

        self.update_stats(|stats| {
            stats.entries = 0;
            stats.memory_usage = 0;
        })
        .await;
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let stats = self.stats.read().await;

        CacheStats {
            entries: entries.len(),
            memory_usage: self.calculate_memory_usage(&entries),
            ..*stats
        }
    }

    /// Clean up stale entries
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        let mut to_remove = Vec::new();

        for (key, entry) in entries.iter() {
            if entry.age() >= self.config.max_age
                || entry.time_since_access() >= self.config.max_idle_time
            {
                to_remove.push(key.clone());
            }
        }

        let eviction_count = to_remove.len();
        for key in to_remove {
            entries.remove(&key);
        }

        self.update_stats(|stats| {
            stats.entries = entries.len();
            stats.memory_usage = self.calculate_memory_usage(&entries);
            stats.evictions += eviction_count as u64;
        })
        .await;
    }

    /// Evict the least recently used entry
    async fn evict_lru(&self, entries: &mut HashMap<String, CacheEntry>) {
        if let Some((key_to_remove, _)) =
            entries.iter().min_by_key(|(_, entry)| entry.last_accessed)
        {
            let key = key_to_remove.clone();
            entries.remove(&key);
            self.update_stats(|stats| stats.evictions += 1).await;
        }
    }

    /// Calculate approximate memory usage
    fn calculate_memory_usage(&self, entries: &HashMap<String, CacheEntry>) -> u64 {
        entries
            .iter()
            .map(|(key, entry)| {
                key.len() + entry.value.memory_size() + std::mem::size_of::<CacheEntry>()
            })
            .sum::<usize>() as u64
    }

    /// Update cache statistics
    async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut CacheStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut stats);
    }
}

/// Cached storage engine wrapper
pub struct CachedStorageEngine {
    inner: Arc<dyn StorageEngine>,
    cache: LruCache,
}

impl CachedStorageEngine {
    /// Create a new cached storage engine
    pub fn new(inner: Arc<dyn StorageEngine>) -> Self {
        Self {
            inner,
            cache: LruCache::new(),
        }
    }

    /// Create a new cached storage engine with custom cache configuration
    pub fn with_cache_config(inner: Arc<dyn StorageEngine>, config: CacheConfig) -> Self {
        Self {
            inner,
            cache: LruCache::with_config(config),
        }
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        self.cache.stats().await
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    /// Clean up stale cache entries
    pub async fn cleanup_cache(&self) {
        self.cache.cleanup().await;
    }
}

#[async_trait::async_trait]
impl StorageEngine for CachedStorageEngine {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        // Try cache first
        if let Some(value) = self.cache.get(key).await {
            return Ok(Some(value));
        }

        // Fall back to storage
        let result = self.inner.get(key).await?;

        // Cache the result if found
        if let Some(ref value) = result {
            self.cache.put(key.to_string(), value.clone()).await;
        }

        Ok(result)
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        // Update storage
        self.inner.put(key, value).await?;

        // Update cache
        self.cache.put(key.to_string(), value.clone()).await;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Update storage
        self.inner.delete(key).await?;

        // Remove from cache
        self.cache.remove(key).await;

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        // Check cache first
        if self.cache.get(key).await.is_some() {
            return Ok(true);
        }

        // Fall back to storage
        self.inner.exists(key).await
    }

    async fn keys(&self) -> Result<Vec<String>> {
        self.inner.keys().await
    }

    async fn count(&self) -> Result<u64> {
        self.inner.count().await
    }

    async fn stats(&self) -> Result<crate::StorageStats> {
        self.inner.stats().await
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear().await?;
        self.cache.clear().await;
        Ok(())
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        // Update cache first to avoid cloning
        for (key, value) in &items {
            self.cache.put(key.clone(), value.clone()).await;
        }

        // Then update storage
        self.inner.batch_put(items).await?;

        Ok(())
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let mut results = Vec::new();
        let mut uncached_keys = Vec::new();

        // Check cache first
        for key in &keys {
            if let Some(value) = self.cache.get(key).await {
                results.push(Some(value));
            } else {
                results.push(None);
                uncached_keys.push(key.clone());
            }
        }

        // Get uncached keys from storage
        if !uncached_keys.is_empty() {
            let storage_results = self.inner.batch_get(uncached_keys.clone()).await?;

            // Update results and cache
            let mut uncached_index = 0;
            for (i, key) in keys.iter().enumerate() {
                if results[i].is_none() {
                    if let Some(value) = &storage_results[uncached_index] {
                        results[i] = Some(value.clone());
                        self.cache.put(key.clone(), value.clone()).await;
                    }
                    uncached_index += 1;
                }
            }
        }

        Ok(results)
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        self.inner.batch_delete(keys.clone()).await?;

        // Remove from cache
        for key in keys {
            self.cache.remove(&key).await;
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStorage, StorageMode};

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = LruCache::new();

        // Test put and get
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        assert_eq!(
            cache.get("key1").await,
            Some(Value::String("value1".to_string()))
        );

        // Test miss
        assert_eq!(cache.get("key2").await, None);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let config = CacheConfig {
            max_entries: 2,
            max_age: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(30),
            auto_cleanup: false,
            cleanup_interval: Duration::from_secs(30),
        };
        let cache = LruCache::with_config(config);

        // Fill cache to capacity
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache
            .put("key2".to_string(), Value::String("value2".to_string()))
            .await;

        // Add one more to trigger eviction
        cache
            .put("key3".to_string(), Value::String("value3".to_string()))
            .await;

        // key1 should be evicted (LRU)
        assert_eq!(cache.get("key1").await, None);
        assert_eq!(
            cache.get("key2").await,
            Some(Value::String("value2".to_string()))
        );
        assert_eq!(
            cache.get("key3").await,
            Some(Value::String("value3".to_string()))
        );
    }

    #[tokio::test]
    async fn test_cached_storage_engine() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let cached = CachedStorageEngine::new(storage);

        // Test put and get
        cached
            .put("key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        assert_eq!(
            cached.get("key1").await.unwrap(),
            Some(Value::String("value1".to_string()))
        );

        // Test cache hit
        let stats = cached.cache_stats().await;
        assert!(stats.hits > 0);
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let config = CacheConfig {
            max_entries: 100,
            max_age: Duration::from_millis(100),
            max_idle_time: Duration::from_millis(50),
            auto_cleanup: false,
            cleanup_interval: Duration::from_millis(30),
        };
        let cache = LruCache::with_config(config);

        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;

        // Wait for entry to become stale
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Cleanup should remove stale entry
        cache.cleanup().await;
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_cache_hit_miss_tracking() {
        let cache = LruCache::new();

        // Miss
        assert_eq!(cache.get("key1").await, None);
        let stats = cache.stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Put and hit
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        assert_eq!(
            cache.get("key1").await,
            Some(Value::String("value1".to_string()))
        );

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_eviction_policy_enforcement() {
        let config = CacheConfig {
            max_entries: 3,
            max_age: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(30),
            auto_cleanup: false,
            cleanup_interval: Duration::from_secs(30),
        };
        let cache = LruCache::with_config(config);

        // Fill cache
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache
            .put("key2".to_string(), Value::String("value2".to_string()))
            .await;
        cache
            .put("key3".to_string(), Value::String("value3".to_string()))
            .await;

        // Access key1 to make it more recently used
        let _ = cache.get("key1").await;

        // Add key4, should evict key2 (least recently used)
        cache
            .put("key4".to_string(), Value::String("value4".to_string()))
            .await;

        // key2 should be evicted
        assert_eq!(cache.get("key2").await, None);
        assert_eq!(
            cache.get("key1").await,
            Some(Value::String("value1".to_string()))
        );
        assert_eq!(
            cache.get("key3").await,
            Some(Value::String("value3".to_string()))
        );
        assert_eq!(
            cache.get("key4").await,
            Some(Value::String("value4".to_string()))
        );
    }

    #[tokio::test]
    async fn test_ttl_expiration_handling() {
        let config = CacheConfig {
            max_entries: 100,
            max_age: Duration::from_millis(100),
            max_idle_time: Duration::from_secs(60),
            auto_cleanup: false,
            cleanup_interval: Duration::from_millis(30),
        };
        let cache = LruCache::with_config(config);

        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;

        // Entry should be valid immediately
        assert_eq!(
            cache.get("key1").await,
            Some(Value::String("value1".to_string()))
        );

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Entry should be expired
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_cache_size_limits() {
        let config = CacheConfig {
            max_entries: 5,
            max_age: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(30),
            auto_cleanup: false,
            cleanup_interval: Duration::from_secs(30),
        };
        let cache = LruCache::with_config(config);

        // Add more entries than max
        for i in 0..10 {
            cache
                .put(format!("key_{}", i), Value::String(format!("value_{}", i)))
                .await;
        }

        // Cache should not exceed max_entries
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 5);
    }

    #[tokio::test]
    async fn test_concurrent_cache_access() {
        let cache = Arc::new(LruCache::new());

        // Spawn multiple concurrent operations
        let mut handles = Vec::new();
        for i in 0..20 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                let key = format!("key_{}", i);
                let value = Value::String(format!("value_{}", i));
                cache_clone.put(key.clone(), value.clone()).await;
                cache_clone.get(&key).await
            });
            handles.push(handle);
        }

        // Wait for all operations
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }

        // Verify cache state
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 20);
    }

    #[tokio::test]
    async fn test_cache_remove_operation() {
        let cache = LruCache::new();

        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache
            .put("key2".to_string(), Value::String("value2".to_string()))
            .await;

        // Remove key1
        let removed = cache.remove("key1").await;
        assert_eq!(removed, Some(Value::String("value1".to_string())));

        // key1 should be gone
        assert_eq!(cache.get("key1").await, None);
        assert_eq!(
            cache.get("key2").await,
            Some(Value::String("value2".to_string()))
        );

        // Remove non-existent key
        let removed = cache.remove("nonexistent").await;
        assert_eq!(removed, None);
    }

    #[tokio::test]
    async fn test_cache_clear_operation() {
        let cache = LruCache::new();

        // Add entries
        for i in 0..10 {
            cache
                .put(format!("key_{}", i), Value::String(format!("value_{}", i)))
                .await;
        }

        let stats_before = cache.stats().await;
        assert_eq!(stats_before.entries, 10);

        // Clear cache
        cache.clear().await;

        let stats_after = cache.stats().await;
        assert_eq!(stats_after.entries, 0);
        assert_eq!(stats_after.memory_usage, 0);

        // All entries should be gone
        for i in 0..10 {
            assert_eq!(cache.get(&format!("key_{}", i)).await, None);
        }
    }

    #[tokio::test]
    async fn test_cache_stats_accuracy() {
        let cache = LruCache::new();

        // Initial stats
        let initial_stats = cache.stats().await;
        assert_eq!(initial_stats.entries, 0);
        assert_eq!(initial_stats.hits, 0);
        assert_eq!(initial_stats.misses, 0);
        assert_eq!(initial_stats.evictions, 0);

        // Perform operations
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache.get("key1").await; // Hit
        cache.get("key1").await; // Hit
        cache.get("key2").await; // Miss
        cache.remove("key1").await;

        let stats = cache.stats().await;
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_hit_rate_calculation() {
        let cache = LruCache::new();

        // No operations yet
        let stats = cache.stats().await;
        assert_eq!(stats.hit_rate(), 0.0);

        // Add entry and get it
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache.get("key1").await; // Hit
        cache.get("key1").await; // Hit
        cache.get("key2").await; // Miss

        let stats = cache.stats().await;
        let hit_rate = stats.hit_rate();
        assert_eq!(hit_rate, 2.0 / 3.0); // 2 hits out of 3 total operations
    }

    #[tokio::test]
    async fn test_idle_time_eviction() {
        let config = CacheConfig {
            max_entries: 100,
            max_age: Duration::from_secs(60),
            max_idle_time: Duration::from_millis(100),
            auto_cleanup: false,
            cleanup_interval: Duration::from_millis(30),
        };
        let cache = LruCache::with_config(config);

        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;

        // Access immediately (should be valid)
        assert_eq!(
            cache.get("key1").await,
            Some(Value::String("value1".to_string()))
        );

        // Wait for idle time to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Entry should be expired due to idle time
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_memory_usage_tracking() {
        let cache = LruCache::new();

        let initial_stats = cache.stats().await;
        assert_eq!(initial_stats.memory_usage, 0);

        // Add entries
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache
            .put("key2".to_string(), Value::String("value2".to_string()))
            .await;

        let stats_after = cache.stats().await;
        assert!(stats_after.memory_usage > initial_stats.memory_usage);

        // Remove entry
        cache.remove("key1").await;
        let stats_after_remove = cache.stats().await;
        assert!(stats_after_remove.memory_usage < stats_after.memory_usage);
    }

    #[tokio::test]
    async fn test_eviction_count_tracking() {
        let config = CacheConfig {
            max_entries: 2,
            max_age: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(30),
            auto_cleanup: false,
            cleanup_interval: Duration::from_secs(30),
        };
        let cache = LruCache::with_config(config);

        let initial_stats = cache.stats().await;
        assert_eq!(initial_stats.evictions, 0);

        // Fill cache and trigger evictions
        cache
            .put("key1".to_string(), Value::String("value1".to_string()))
            .await;
        cache
            .put("key2".to_string(), Value::String("value2".to_string()))
            .await;
        cache
            .put("key3".to_string(), Value::String("value3".to_string()))
            .await; // Should evict key1

        let stats = cache.stats().await;
        assert!(stats.evictions > 0);
    }
}

#[cfg(test)]
#[cfg(feature = "proptest")]
mod proptest_tests {
    use super::*;
    #[cfg(feature = "proptest")]
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn test_lru_cache_operations_property(
            operations in prop::collection::vec(
                prop_oneof![
                    (any::<String>(), any::<Vec<u8>>()).prop_map(|(k, v)| ("put", k, v)),
                    any::<String>().prop_map(|k| ("get", k, vec![])),
                    any::<String>().prop_map(|k| ("remove", k, vec![])),
                ],
                0..100
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = CacheConfig {
                max_entries: 1000,
                max_age: Duration::from_secs(3600),
                max_idle_time: Duration::from_secs(1800),
                auto_cleanup: true,
                cleanup_interval: Duration::from_secs(300),
            };

            let cache = LruCache::with_config(config);
            let mut expected_keys = std::collections::HashSet::new();

            for (op, key, value) in operations {
                match op {
                    "put" => {
                        let value = Value::Bytes(value);
                        rt.block_on(cache.put(key.clone(), value));
                        expected_keys.insert(key.clone());
                    }
                    "get" => {
                        let result = rt.block_on(cache.get(&key));
                        if expected_keys.contains(&key) {
                            prop_assert!(result.is_some());
                        } else {
                            prop_assert!(result.is_none());
                        }
                    }
                    "remove" => {
                        let result = rt.block_on(cache.remove(&key));
                        let was_present = expected_keys.contains(&key);
                        expected_keys.remove(&key);
                        if was_present {
                            prop_assert!(result.is_some());
                        } else {
                            prop_assert!(result.is_none());
                        }
                    }
                    _ => {}
                }
            }
        }

        #[test]
        fn test_lru_cache_eviction_property(
            items in prop::collection::vec(
                (any::<String>(), prop::collection::vec(any::<u8>(), 0..100)),
                0..200
            ),
            max_size in 10..100usize
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = CacheConfig {
                max_entries: max_size,
                max_age: Duration::from_secs(3600),
                max_idle_time: Duration::from_secs(1800),
                auto_cleanup: true,
                cleanup_interval: Duration::from_secs(300),
            };

            let cache = LruCache::with_config(config);

            // Insert items
            for (key, value) in &items {
                let value = Value::Bytes(value.clone());
                rt.block_on(cache.put(key.clone(), value));
            }

            // Verify cache size doesn't exceed max_size
            let stats = rt.block_on(cache.stats());
            prop_assert!(stats.entries <= max_size);
        }

        #[test]
        fn test_lru_cache_stats_property(
            operations in prop::collection::vec(
                prop_oneof![
                    (any::<String>(), any::<Vec<u8>>()).prop_map(|(k, v)| ("put", k, v)),
                    any::<String>().prop_map(|k| ("get", k, vec![])),
                    any::<String>().prop_map(|k| ("remove", k, vec![])),
                ],
                0..100
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = CacheConfig {
                max_entries: 1000,
                max_age: Duration::from_secs(3600),
                max_idle_time: Duration::from_secs(1800),
                auto_cleanup: true,
                cleanup_interval: Duration::from_secs(300),
            };

            let cache = LruCache::with_config(config);

            // Perform operations
            for (op, key, value) in operations {
                match op {
                    "put" => {
                        let value = Value::Bytes(value);
                        rt.block_on(cache.put(key.clone(), value));
                    }
                    "get" => {
                        let _ = rt.block_on(cache.get(&key));
                    }
                    "remove" => {
                        let _ = rt.block_on(cache.remove(&key));
                    }
                    _ => {}
                }
            }

            // Verify stats properties
            let stats = rt.block_on(cache.stats());
            // Note: entries, memory_usage, hits, and misses are u64, so >= 0 is always true

            // Verify hit rate is between 0 and 1
            let hit_rate = stats.hit_rate();
            prop_assert!((0.0..=1.0).contains(&hit_rate));
        }

        #[test]
        fn test_cache_entry_properties(
            key in any::<String>(),
            value in prop::collection::vec(any::<u8>(), 0..1000)
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = CacheConfig {
                max_entries: 1000,
                max_age: Duration::from_secs(3600),
                max_idle_time: Duration::from_secs(1800),
                auto_cleanup: true,
                cleanup_interval: Duration::from_secs(300),
            };

            let cache = LruCache::with_config(config);
            let value = Value::Bytes(value);

            // Put and get
            rt.block_on(cache.put(key.clone(), value));
            let retrieved = rt.block_on(cache.get(&key));
            prop_assert!(retrieved.is_some());

            // Verify age properties
            let stats = rt.block_on(cache.stats());
            prop_assert!(stats.entries > 0);
        }
    }
}
