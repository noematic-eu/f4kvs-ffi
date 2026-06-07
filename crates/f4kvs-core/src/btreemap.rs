#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::storage_traits::{StorageEngine, StorageStats};
use crate::{Result, Value};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory storage implementation using BTreeMap
pub struct BTreeMapStorage {
    data: Arc<RwLock<BTreeMap<String, Value>>>,
    stats: Arc<RwLock<StorageStats>>,
}

impl Default for BTreeMapStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl BTreeMapStorage {
    /// Create a new BTreeMap storage instance
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(BTreeMap::new())),
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

    /// Create with initial data (for testing)
    pub fn with_data(data: BTreeMap<String, Value>) -> Self {
        let key_count = data.len() as u64;
        let memory_usage = data
            .iter()
            .map(|(k, v)| k.len() + v.memory_size())
            .sum::<usize>() as u64;

        Self {
            data: Arc::new(RwLock::new(data)),
            stats: Arc::new(RwLock::new(StorageStats {
                key_count,
                memory_usage,
                total_operations: 0,
                get_operations: 0,
                put_operations: 0,
                delete_operations: 0,
                scan_operations: 0,
                average_key_size: 0.0,
                average_value_size: 0.0,
                peak_memory_usage: memory_usage,
                cache_hits: 0,
                cache_misses: 0,
            })),
        }
    }

    async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut StorageStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut stats);
    }

    /// Update memory usage incrementally (O(1) operation)
    async fn update_memory_usage(
        &self,
        key: &str,
        old_value: Option<&Value>,
        new_value: Option<&Value>,
    ) {
        let old_size = old_value.map_or(0, |v| v.memory_size() as u64);
        let new_size = new_value.map_or(0, |v| v.memory_size() as u64);
        let key_size = key.len() as u64;

        // Calculate the total memory change
        // For a key-value pair, we need: key length + value memory size
        let old_total = if old_value.is_some() {
            key_size + old_size
        } else {
            0
        };
        let new_total = if new_value.is_some() {
            key_size + new_size
        } else {
            0
        };
        let total_delta = new_total as i64 - old_total as i64;

        let mut stats = self.stats.write().await;
        if total_delta > 0 {
            // Use saturating_add to prevent integer overflow
            stats.memory_usage = stats.memory_usage.saturating_add(total_delta as u64);
        } else if total_delta < 0 {
            stats.memory_usage = stats.memory_usage.saturating_sub((-total_delta) as u64);
        }
    }

    /// Get current memory usage for validation/testing
    pub async fn get_current_memory_usage(&self) -> u64 {
        let stats = self.stats.read().await;
        stats.memory_usage
    }
}

#[async_trait]
impl StorageEngine for BTreeMapStorage {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        let data = self.data.read().await;
        let result = data.get(key).cloned();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.get_operations += 1;
        })
        .await;
        Ok(result)
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        let mut data = self.data.write().await;

        let is_new_key = !data.contains_key(key);
        let old_value = data.insert(key.to_string(), value.clone());

        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.put_operations += 1;
            if is_new_key {
                stats.key_count += 1;
            }
        })
        .await;

        // Update memory usage incrementally
        self.update_memory_usage(key, old_value.as_ref(), Some(value))
            .await;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().await;
        let old_value = data.remove(key);
        let existed = old_value.is_some();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.delete_operations += 1;
            if existed {
                stats.key_count -= 1;
            }
        })
        .await;

        if existed {
            // Update memory usage incrementally
            self.update_memory_usage(key, old_value.as_ref(), None)
                .await;
        }

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().await;
        let exists = data.contains_key(key);
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(exists)
    }

    async fn keys(&self) -> Result<Vec<String>> {
        let data = self.data.read().await;
        let keys = data.keys().cloned().collect();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(keys)
    }

    async fn count(&self) -> Result<u64> {
        let stats = self.stats.read().await;
        Ok(stats.key_count)
    }

    async fn stats(&self) -> Result<StorageStats> {
        let stats = self.stats.read().await;

        // Calculate averages if we have data
        if stats.key_count > 0 {
            let data = self.data.read().await;
            let total_key_size: usize = data.keys().map(|k| k.len()).sum();
            let total_value_size: usize = data.values().map(|v| v.memory_size()).sum();

            let mut result = stats.clone();
            result.average_key_size = total_key_size as f64 / stats.key_count as f64;
            result.average_value_size = total_value_size as f64 / stats.key_count as f64;

            // Set peak memory usage to current memory usage if it's higher
            if stats.memory_usage > result.peak_memory_usage {
                result.peak_memory_usage = stats.memory_usage;
            }

            Ok(result)
        } else {
            Ok(stats.clone())
        }
    }

    async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        drop(data);

        let mut stats = self.stats.write().await;
        stats.key_count = 0;
        stats.memory_usage = 0;
        stats.total_operations += 1;

        Ok(())
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        let item_count = items.len();
        let mut data = self.data.write().await;

        // Pre-allocate capacity if possible to reduce reallocations
        // Track memory changes incrementally
        let mut memory_change: i64 = 0;

        // Optimize: reduce cloning by moving values directly
        for (key, value) in items {
            let key_len = key.len();
            let value_size = value.memory_size();
            let old_value = data.insert(key, value);
            if let Some(old_v) = old_value {
                memory_change -= (key_len + old_v.memory_size()) as i64;
            }
            memory_change += (key_len + value_size) as i64;
        }
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.key_count += item_count as u64;
        })
        .await;

        // Update memory usage incrementally
        if memory_change != 0 {
            self.update_stats(|stats| {
                if memory_change > 0 {
                    stats.memory_usage += memory_change as u64;
                } else {
                    stats.memory_usage = stats.memory_usage.saturating_sub((-memory_change) as u64);
                }
            })
            .await;
        }

        Ok(())
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let data = self.data.read().await;
        // Pre-allocate result vector to avoid reallocations
        let mut values = Vec::with_capacity(keys.len());
        for key in keys {
            values.push(data.get(&key).cloned());
        }
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(values)
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        let mut data = self.data.write().await;
        let mut deleted_count = 0;
        let mut memory_change: i64 = 0;

        for key in keys {
            if let Some(old_value) = data.remove(&key) {
                deleted_count += 1;
                memory_change -= (key.len() + old_value.memory_size()) as i64;
            }
        }
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.key_count -= deleted_count;
        })
        .await;

        // Update memory usage incrementally
        if memory_change != 0 {
            self.update_stats(|stats| {
                stats.memory_usage = stats.memory_usage.saturating_sub((-memory_change) as u64);
            })
            .await;
        }

        Ok(())
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let data = self.data.read().await;
        let keys: Vec<String> = data
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(keys)
    }

    async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        // Validate range: start must be <= end
        if start > end {
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.scan_operations += 1;
            })
            .await;
            return Ok(Vec::new());
        }

        let data = self.data.read().await;
        // Use exclusive end: range(start..end) excludes the end key
        let keys: Vec<String> = data
            .range(start.to_string()..end.to_string())
            .map(|(k, _)| k.clone())
            .collect();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(keys)
    }

    async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        let data = self.data.read().await;
        let pairs: Vec<(String, Value)> = data
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(pairs)
    }

    async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        // Validate range: start must be <= end
        if start > end {
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.scan_operations += 1;
            })
            .await;
            return Ok(Vec::new());
        }

        let data = self.data.read().await;
        let pairs: Vec<(String, Value)> = data
            .range(start.to_string()..end.to_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(pairs)
    }

    async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        let data = self.data.read().await;
        let count = data
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .count() as u64;
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(count)
    }

    async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        // Validate range: start must be <= end
        if start > end {
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.scan_operations += 1;
            })
            .await;
            return Ok(0);
        }

        let data = self.data.read().await;
        let count = data.range(start.to_string()..end.to_string()).count() as u64;
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(count)
    }

    async fn flush(&self) -> Result<()> {
        // For in-memory BTreeMap storage, flush is a no-op
        // All data is already in memory and will be lost on process exit
        // This is expected behavior for in-memory storage
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[tokio::test]
    async fn test_new_storage() {
        let storage = BTreeMapStorage::new();
        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.key_count, 0);
        assert_eq!(stats.memory_usage, 0);
    }

    #[tokio::test]
    async fn test_ordered_key_operations() {
        let storage = BTreeMapStorage::new();

        // Insert keys in non-ordered fashion
        storage
            .put("z_key", &Value::String("z_value".to_string()))
            .await
            .unwrap();
        storage
            .put("a_key", &Value::String("a_value".to_string()))
            .await
            .unwrap();
        storage
            .put("m_key", &Value::String("m_value".to_string()))
            .await
            .unwrap();

        // Keys should be returned in sorted order
        let keys = storage.keys().await.unwrap();
        assert_eq!(keys, vec!["a_key", "m_key", "z_key"]);
    }

    #[tokio::test]
    async fn test_range_query_correctness() {
        let storage = BTreeMapStorage::new();

        // Insert keys
        for i in 0..10 {
            let key = format!("key_{:02}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        // Range query
        let range_keys = storage.scan_range("key_03", "key_07").await.unwrap();
        assert_eq!(range_keys.len(), 4); // key_03, key_04, key_05, key_06
        assert_eq!(range_keys[0], "key_03");
        assert_eq!(range_keys[3], "key_06");
    }

    #[tokio::test]
    async fn test_prefix_scan_operations() {
        let storage = BTreeMapStorage::new();

        // Add keys with various prefixes
        storage
            .put("prefix_a_key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_a_key2", &Value::String("value2".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_b_key1", &Value::String("value3".to_string()))
            .await
            .unwrap();
        storage
            .put("other_key", &Value::String("value4".to_string()))
            .await
            .unwrap();

        // Scan with prefix
        let prefix_keys = storage.scan_prefix("prefix_a").await.unwrap();
        assert_eq!(prefix_keys.len(), 2);
        assert_eq!(prefix_keys[0], "prefix_a_key1");
        assert_eq!(prefix_keys[1], "prefix_a_key2");
    }

    #[tokio::test]
    async fn test_empty_range_handling() {
        let storage = BTreeMapStorage::new();

        // Empty range (start > end)
        let empty_range = storage.scan_range("z", "a").await.unwrap();
        assert_eq!(empty_range.len(), 0);

        // Empty range (same start and end)
        let same_range = storage.scan_range("a", "a").await.unwrap();
        assert_eq!(same_range.len(), 0);
    }

    #[tokio::test]
    async fn test_boundary_key_scenarios() {
        let storage = BTreeMapStorage::new();

        // Add boundary keys
        storage
            .put("a", &Value::String("first".to_string()))
            .await
            .unwrap();
        storage
            .put("z", &Value::String("last".to_string()))
            .await
            .unwrap();
        storage
            .put("m", &Value::String("middle".to_string()))
            .await
            .unwrap();

        // Range including boundaries
        let all_keys = storage.scan_range("a", "z").await.unwrap();
        assert_eq!(all_keys.len(), 2); // a and m (z is exclusive)

        // Range including last key
        let with_z = storage.scan_range("a", "{").await.unwrap(); // Use char after z
        assert_eq!(with_z.len(), 3);
    }

    #[tokio::test]
    async fn test_range_query_performance() {
        let storage = BTreeMapStorage::new();

        // Insert many keys
        for i in 0..1000 {
            let key = format!("key_{:04}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        // Range query should be efficient
        let start = std::time::Instant::now();
        let range_keys = storage.scan_range("key_0100", "key_0200").await.unwrap();
        let duration = start.elapsed();

        assert_eq!(range_keys.len(), 100);
        assert!(duration.as_millis() < 100); // Should be fast
    }

    #[tokio::test]
    async fn test_ordered_scan_prefix_pairs() {
        let storage = BTreeMapStorage::new();

        storage
            .put("prefix_key3", &Value::String("value3".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_key2", &Value::String("value2".to_string()))
            .await
            .unwrap();

        let pairs = storage.scan_prefix_pairs("prefix").await.unwrap();
        assert_eq!(pairs.len(), 3);
        // Should be in sorted order
        assert_eq!(pairs[0].0, "prefix_key1");
        assert_eq!(pairs[1].0, "prefix_key2");
        assert_eq!(pairs[2].0, "prefix_key3");
    }

    #[tokio::test]
    async fn test_ordered_scan_range_pairs() {
        let storage = BTreeMapStorage::new();

        for i in 0..10 {
            let key = format!("key_{:02}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        let pairs = storage.scan_range_pairs("key_03", "key_07").await.unwrap();
        assert_eq!(pairs.len(), 4);
        // Should be in sorted order
        for i in 0..4 {
            assert_eq!(pairs[i].0, format!("key_{:02}", i + 3));
        }
    }

    #[tokio::test]
    async fn test_count_prefix_accuracy() {
        let storage = BTreeMapStorage::new();

        for i in 0..50 {
            storage
                .put(
                    &format!("prefix_{}", i),
                    &Value::String(format!("value_{}", i)),
                )
                .await
                .unwrap();
        }
        for i in 0..30 {
            storage
                .put(
                    &format!("other_{}", i),
                    &Value::String(format!("value_{}", i)),
                )
                .await
                .unwrap();
        }

        let prefix_count = storage.count_prefix("prefix").await.unwrap();
        assert_eq!(prefix_count, 50);

        let other_count = storage.count_prefix("other").await.unwrap();
        assert_eq!(other_count, 30);
    }

    #[tokio::test]
    async fn test_count_range_accuracy() {
        let storage = BTreeMapStorage::new();

        for i in 0..100 {
            let key = format!("key_{:03}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        let range_count = storage.count_range("key_010", "key_050").await.unwrap();
        assert_eq!(range_count, 40); // key_010 to key_049 (exclusive end)
    }

    #[tokio::test]
    async fn test_large_ordered_dataset_handling() {
        let storage = BTreeMapStorage::new();

        // Insert large ordered dataset
        for i in 0..10000 {
            let key = format!("key_{:05}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        // Verify ordering is maintained
        let keys = storage.keys().await.unwrap();
        assert_eq!(keys.len(), 10000);
        for i in 1..keys.len() {
            assert!(keys[i - 1] < keys[i]);
        }
    }

    #[tokio::test]
    async fn test_memory_efficiency_validation() {
        let storage = BTreeMapStorage::new();

        let initial_memory = storage.get_current_memory_usage().await;

        // Add data
        for i in 0..100 {
            let key = format!("key_{}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        let memory_after = storage.get_current_memory_usage().await;
        assert!(memory_after > initial_memory);

        // Delete half
        for i in 0..50 {
            let key = format!("key_{}", i);
            storage.delete(&key).await.unwrap();
        }

        let memory_after_delete = storage.get_current_memory_usage().await;
        assert!(memory_after_delete < memory_after);
    }

    #[tokio::test]
    async fn test_concurrent_range_operations() {
        let storage = Arc::new(BTreeMapStorage::new());

        // Add data
        for i in 0..100 {
            let key = format!("key_{:03}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        // Spawn concurrent range queries
        let mut handles = Vec::new();
        for i in 0..10 {
            let storage_clone = Arc::clone(&storage);
            let handle = tokio::spawn(async move {
                let start_key = format!("key_{:03}", i * 10);
                let end_key = format!("key_{:03}", (i + 1) * 10);
                storage_clone.scan_range(&start_key, &end_key).await
            });
            handles.push(handle);
        }

        // Wait for all queries
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            let keys = result.unwrap();
            assert!(!keys.is_empty());
        }
    }
}
