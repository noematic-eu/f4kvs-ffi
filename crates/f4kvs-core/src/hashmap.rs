#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use super::storage_traits::{StorageEngine, StorageStats};
use crate::{Result, Value};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory storage implementation using HashMap with row-level locking
pub struct HashMapStorage {
    /// Each key has its own lock for row-level concurrency control
    /// None means key doesn't exist, Some(lock) means it does (even if value is Null)
    data: Arc<RwLock<HashMap<String, Arc<RwLock<Value>>>>>,
    stats: Arc<RwLock<StorageStats>>,
}

impl Default for HashMapStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl HashMapStorage {
    /// Create a new HashMap storage instance
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
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

    /// Get a locked value for a specific key (returns None if key doesn't exist)
    async fn get_value_lock(&self, key: &str) -> Option<Arc<RwLock<Value>>> {
        let data = self.data.read().await;
        data.get(key).cloned()
    }

    /// Create or get a lock for a key (for write operations)
    async fn get_or_create_value_lock(&self, key: String) -> Arc<RwLock<Value>> {
        let mut data = self.data.write().await;
        if let Some(existing_lock) = data.get(&key) {
            existing_lock.clone()
        } else {
            // For new keys, we need to determine the initial value
            // For now, we'll use Null as placeholder, but the put method will handle this
            let new_lock = Arc::new(RwLock::new(Value::Null));
            data.insert(key, new_lock.clone());
            new_lock
        }
    }

    /// Create with initial data (for testing)
    pub fn with_data(data: HashMap<String, Value>) -> Self {
        let key_count = data.len() as u64;
        let memory_usage = data
            .iter()
            .map(|(k, v)| k.len() + v.memory_size())
            .sum::<usize>() as u64;

        // Convert HashMap<String, Value> to HashMap<String, Arc<RwLock<Value>>>
        let locked_data: HashMap<String, Arc<RwLock<Value>>> = data
            .into_iter()
            .map(|(k, v)| (k, Arc::new(RwLock::new(v))))
            .collect();

        Self {
            data: Arc::new(RwLock::new(locked_data)),
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

        // Update peak memory usage
        if stats.memory_usage > stats.peak_memory_usage {
            stats.peak_memory_usage = stats.memory_usage;
        }
    }

    /// Get current memory usage for validation/testing
    pub async fn get_current_memory_usage(&self) -> u64 {
        let stats = self.stats.read().await;
        stats.memory_usage
    }
}

#[async_trait]
impl StorageEngine for HashMapStorage {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        // Try to get the value lock for this key
        if let Some(value_lock) = self.get_value_lock(key).await {
            // Read the value using the row lock
            let value = {
                let locked_value = value_lock.read().await;
                // Key exists, so return whatever value it has (including Null)
                Some(locked_value.clone())
            };

            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.get_operations += 1;
            })
            .await;

            Ok(value)
        } else {
            // Key doesn't exist
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.get_operations += 1;
            })
            .await;

            Ok(None)
        }
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        // Check if key exists before acquiring lock
        let key_existed = {
            let data = self.data.read().await;
            data.contains_key(key)
        };

        // Use row-level locking - get or create a lock for this specific key
        let value_lock = self.get_or_create_value_lock(key.to_string()).await;

        // Lock this specific key's value
        let old_value = {
            let mut locked_value = value_lock.write().await;
            let old = locked_value.clone();
            *locked_value = value.clone();
            old
        };

        // Update stats
        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.put_operations += 1;
            if !key_existed {
                stats.key_count += 1;
            }
        })
        .await;

        // Update memory usage incrementally
        self.update_memory_usage(
            key,
            if key_existed { Some(&old_value) } else { None },
            Some(value),
        )
        .await;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Check if key exists before acquiring lock
        let key_existed = {
            let data = self.data.read().await;
            data.contains_key(key)
        };

        if key_existed {
            // Get the value lock and read the old value
            let old_value = {
                let value_lock = self.get_value_lock(key).await.ok_or_else(|| {
                    crate::F4KvsError::KeyNotFound {
                        key: key.to_string(),
                    }
                })?;
                let locked_value = value_lock.read().await;
                locked_value.clone()
            };

            // Remove the key from the hashmap
            let mut data = self.data.write().await;
            data.remove(key);
            drop(data);

            // Update stats
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.delete_operations += 1;
                stats.key_count -= 1;
            })
            .await;

            // Update memory usage incrementally
            self.update_memory_usage(key, Some(&old_value), None).await;
        } else {
            // Key doesn't exist, still count as an operation
            self.update_stats(|stats| {
                stats.total_operations += 1;
                stats.delete_operations += 1;
            })
            .await;
        }

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let exists = if let Some(value_lock) = self.get_value_lock(key).await {
            // Check if the value is not Null (meaning the key exists)
            let locked_value = value_lock.read().await;
            !matches!(*locked_value, Value::Null)
        } else {
            false
        };

        self.update_stats(|stats| stats.total_operations += 1).await;
        Ok(exists)
    }

    async fn keys(&self) -> Result<Vec<String>> {
        let data = self.data.read().await;
        let keys = data.keys().cloned().collect();
        drop(data);

        self.update_stats(|stats| stats.total_operations += 1).await;
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

            // For values, we need to read each locked value
            let mut total_value_size = 0usize;
            for value_lock in data.values() {
                let locked_value = value_lock.read().await;
                total_value_size += locked_value.memory_size();
            }

            let mut result = stats.clone();
            result.average_key_size = total_key_size as f64 / stats.key_count as f64;
            result.average_value_size = total_value_size as f64 / stats.key_count as f64;

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
        if items.is_empty() {
            return Ok(());
        }

        // Sort items by key to acquire locks in consistent order (prevent deadlocks)
        let mut sorted_items = items;
        sorted_items.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Check which keys exist before acquiring locks
        let mut existing_keys = std::collections::HashSet::new();
        let mut seen_in_batch = std::collections::HashSet::new();
        {
            let data = self.data.read().await;
            for (key, _) in &sorted_items {
                if data.contains_key(key) {
                    existing_keys.insert(key.clone());
                }
            }
        }

        // Acquire locks for all keys in sorted order
        let mut locks = Vec::new();
        let mut old_values = Vec::new();
        let mut new_keys = 0u64;

        for (key, value) in &sorted_items {
            let value_lock = self.get_or_create_value_lock(key.clone()).await;
            let old_value = {
                let mut locked_value = value_lock.write().await;
                let old = locked_value.clone();
                *locked_value = value.clone();
                old
            };

            // A key is "new" if it didn't exist before this batch AND we haven't seen it in this batch yet
            if !existing_keys.contains(key) && !seen_in_batch.contains(key) {
                new_keys += 1;
                seen_in_batch.insert(key.clone());
            }

            locks.push(value_lock);
            old_values.push(old_value);
        }

        // Update stats
        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.key_count += new_keys;
        })
        .await;

        // Update memory usage incrementally
        for ((key, value), old_value) in sorted_items.iter().zip(old_values.iter()) {
            let is_new_key = matches!(old_value, Value::Null);
            self.update_memory_usage(
                key,
                if is_new_key { None } else { Some(old_value) },
                Some(value),
            )
            .await;
        }

        Ok(())
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let mut values = Vec::with_capacity(keys.len());

        for key in &keys {
            if let Some(value_lock) = self.get_value_lock(key).await {
                let value = {
                    let locked_value = value_lock.read().await;
                    if matches!(*locked_value, Value::Null) {
                        None
                    } else {
                        Some(locked_value.clone())
                    }
                };
                values.push(value);
            } else {
                values.push(None);
            }
        }

        self.update_stats(|stats| stats.total_operations += 1).await;
        Ok(values)
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        // Sort keys to acquire locks in consistent order
        let mut sorted_keys = keys;
        sorted_keys.sort();

        // Check which keys exist before acquiring locks
        let (existing_keys, old_values) = {
            let data = self.data.read().await;
            let mut existing_keys = Vec::new();
            let mut old_values = Vec::new();
            for key in &sorted_keys {
                if data.contains_key(key) {
                    existing_keys.push(key.clone());
                    if let Some(value_lock) = data.get(key) {
                        let locked_value =
                            value_lock
                                .try_read()
                                .map_err(|_| crate::F4KvsError::Internal {
                                    message: "lock contention during batch_delete".into(),
                                })?;
                        old_values.push((key.clone(), locked_value.clone()));
                    }
                }
            }
            (existing_keys, old_values)
        };

        // Remove existing keys from hashmap
        if !existing_keys.is_empty() {
            let mut data = self.data.write().await;
            for key in &existing_keys {
                data.remove(key);
            }
            drop(data);
        }

        let deleted_count = existing_keys.len() as u64;

        // Update stats
        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.key_count = stats.key_count.saturating_sub(deleted_count);
        })
        .await;

        // Update memory usage
        for (key, old_value) in old_values {
            self.update_memory_usage(&key, Some(&old_value), None).await;
        }

        Ok(())
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let data = self.data.read().await;
        let keys: Vec<String> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
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
        let data = self.data.read().await;
        let mut keys: Vec<String> = data
            .keys()
            .filter(|k| k.as_str() >= start && k.as_str() < end)
            .cloned()
            .collect();
        keys.sort(); // HashMap doesn't maintain order, so sort for consistency
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
        let mut pairs = Vec::new();

        // Collect keys that match prefix
        let matching_keys: Vec<String> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        drop(data);

        // Read values for matching keys
        for key in matching_keys {
            if let Some(value_lock) = self.get_value_lock(&key).await {
                let value = {
                    let locked_value = value_lock.read().await;
                    if matches!(*locked_value, Value::Null) {
                        continue; // Skip non-existent keys
                    }
                    locked_value.clone()
                };
                pairs.push((key, value));
            }
        }

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(pairs)
    }

    async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        let data = self.data.read().await;

        // Collect keys that match range
        let mut matching_keys: Vec<String> = data
            .keys()
            .filter(|k| k.as_str() >= start && k.as_str() < end)
            .cloned()
            .collect();

        matching_keys.sort(); // Sort by key for consistency
        drop(data);

        // Read values for matching keys
        let mut pairs = Vec::new();
        for key in matching_keys {
            if let Some(value_lock) = self.get_value_lock(&key).await {
                let value = {
                    let locked_value = value_lock.read().await;
                    if matches!(*locked_value, Value::Null) {
                        continue; // Skip non-existent keys
                    }
                    locked_value.clone()
                };
                pairs.push((key, value));
            }
        }

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(pairs)
    }

    async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        let data = self.data.read().await;
        let count = data.keys().filter(|k| k.starts_with(prefix)).count() as u64;
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(count)
    }

    async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        let data = self.data.read().await;
        let count = data
            .keys()
            .filter(|k| k.as_str() >= start && k.as_str() < end)
            .count() as u64;
        drop(data);

        self.update_stats(|stats| {
            stats.total_operations += 1;
            stats.scan_operations += 1;
        })
        .await;
        Ok(count)
    }

    async fn flush(&self) -> Result<()> {
        // For in-memory HashMap storage, flush is a no-op
        // All data is already in memory and will be lost on process exit
        // This is expected behavior for in-memory storage
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_new_storage() {
        let storage = HashMapStorage::new();
        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.key_count, 0);
        assert_eq!(stats.memory_usage, 0);
        assert_eq!(stats.total_operations, 0);
    }

    #[tokio::test]
    async fn test_with_data() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), Value::String("value1".to_string()));
        data.insert("key2".to_string(), Value::Int64(42));

        let storage = HashMapStorage::with_data(data);
        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.key_count, 2);
        assert!(stats.memory_usage > 0);
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let storage = HashMapStorage::new();

        // Test put
        let value = Value::String("test".to_string());
        storage.put("key1", &value).await.unwrap();

        // Test get
        let retrieved = storage.get("key1").await.unwrap();
        assert_eq!(retrieved, Some(value.clone()));

        // Test exists
        assert!(storage.exists("key1").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Test count
        assert_eq!(storage.count().await.unwrap(), 1);

        // Test keys
        let keys = storage.keys().await.unwrap();
        assert_eq!(keys, vec!["key1"]);
    }

    #[tokio::test]
    async fn test_delete() {
        let storage = HashMapStorage::new();
        let value = Value::String("test".to_string());

        // Put a value
        storage.put("key1", &value).await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 1);

        // Delete it
        storage.delete("key1").await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 0);
        assert!(!storage.exists("key1").await.unwrap());

        // Try to delete non-existent key (should not error)
        storage.delete("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_clear() {
        let storage = HashMapStorage::new();
        let value1 = Value::String("test1".to_string());
        let value2 = Value::Int64(42);

        storage.put("key1", &value1).await.unwrap();
        storage.put("key2", &value2).await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 2);

        storage.clear().await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 0);
        assert_eq!(storage.keys().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_batch_put() {
        let storage = HashMapStorage::new();
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::Int64(42)),
            ("key3".to_string(), Value::Bool(true)),
        ];

        storage.batch_put(items).await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 3);

        // Verify all values
        assert_eq!(
            storage.get("key1").await.unwrap(),
            Some(Value::String("value1".to_string()))
        );
        assert_eq!(storage.get("key2").await.unwrap(), Some(Value::Int64(42)));
        assert_eq!(storage.get("key3").await.unwrap(), Some(Value::Bool(true)));
    }

    #[tokio::test]
    async fn test_batch_get() {
        let storage = HashMapStorage::new();
        let value1 = Value::String("value1".to_string());
        let value2 = Value::Int64(42);

        storage.put("key1", &value1).await.unwrap();
        storage.put("key2", &value2).await.unwrap();

        let keys = vec![
            "key1".to_string(),
            "key2".to_string(),
            "nonexistent".to_string(),
        ];
        let results = storage.batch_get(keys).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(Value::String("value1".to_string())));
        assert_eq!(results[1], Some(Value::Int64(42)));
        assert_eq!(results[2], None);
    }

    #[tokio::test]
    async fn test_batch_delete() {
        let storage = HashMapStorage::new();
        let value1 = Value::String("value1".to_string());
        let value2 = Value::Int64(42);
        let value3 = Value::Bool(true);

        storage.put("key1", &value1).await.unwrap();
        storage.put("key2", &value2).await.unwrap();
        storage.put("key3", &value3).await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 3);

        let keys_to_delete = vec![
            "key1".to_string(),
            "key3".to_string(),
            "nonexistent".to_string(),
        ];
        storage.batch_delete(keys_to_delete).await.unwrap();

        assert_eq!(storage.count().await.unwrap(), 1);
        assert!(!storage.exists("key1").await.unwrap());
        assert!(storage.exists("key2").await.unwrap());
        assert!(!storage.exists("key3").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_usage_tracking() {
        let storage = HashMapStorage::new();
        let initial_memory = storage.get_current_memory_usage().await;

        // Add a value
        let value = Value::String("test".to_string());
        storage.put("key1", &value).await.unwrap();
        let memory_after_put = storage.get_current_memory_usage().await;
        assert!(memory_after_put > initial_memory);

        // Update the value
        let new_value = Value::String("updated test".to_string());
        storage.put("key1", &new_value).await.unwrap();
        let memory_after_update = storage.get_current_memory_usage().await;
        assert!(memory_after_update > memory_after_put);

        // Delete the value
        storage.delete("key1").await.unwrap();
        let memory_after_delete = storage.get_current_memory_usage().await;
        assert_eq!(memory_after_delete, initial_memory);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let storage = HashMapStorage::new();
        let initial_stats = storage.stats().await.unwrap();

        // Perform operations
        let value = Value::String("test".to_string());
        storage.put("key1", &value).await.unwrap();
        storage.get("key1").await.unwrap();
        storage.exists("key1").await.unwrap();
        storage.keys().await.unwrap();
        storage.delete("key1").await.unwrap();

        let final_stats = storage.stats().await.unwrap();
        assert_eq!(
            final_stats.total_operations,
            initial_stats.total_operations + 5
        );
        assert_eq!(final_stats.key_count, 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let storage = Arc::new(HashMapStorage::new());
        let mut handles = vec![];

        // Spawn multiple tasks that perform operations concurrently
        for i in 0..10 {
            let storage_clone = Arc::clone(&storage);
            let handle = tokio::spawn(async move {
                let key = format!("key{}", i);
                let value = Value::String(format!("value{}", i));

                storage_clone.put(&key, &value).await.unwrap();
                let retrieved = storage_clone.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));

                storage_clone.exists(&key).await.unwrap();
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        assert_eq!(storage.count().await.unwrap(), 10);
        let keys = storage.keys().await.unwrap();
        assert_eq!(keys.len(), 10);
    }

    #[tokio::test]
    async fn test_large_values() {
        let storage = HashMapStorage::new();
        let large_string = "x".repeat(10000);
        let large_value = Value::String(large_string);

        storage.put("large_key", &large_value).await.unwrap();
        let retrieved = storage.get("large_key").await.unwrap();
        assert_eq!(retrieved, Some(large_value));

        let stats = storage.stats().await.unwrap();
        assert!(stats.memory_usage > 10000);
    }

    #[tokio::test]
    async fn test_different_value_types() {
        let storage = HashMapStorage::new();
        let test_values = vec![
            ("string", Value::String("test".to_string())),
            ("int64", Value::Int64(-42)),
            ("uint64", Value::UInt64(42)),
            ("float64", Value::Float64(std::f64::consts::PI)),
            ("bool", Value::Bool(true)),
            ("bytes", Value::Bytes(vec![1, 2, 3, 4])),
            ("json", Value::Json(serde_json::json!({"key": "value"}))),
            ("null", Value::Null),
        ];

        for (key, value) in test_values {
            storage.put(key, &value).await.unwrap();
            let retrieved = storage.get(key).await.unwrap();
            assert_eq!(retrieved, Some(value));
        }

        assert_eq!(storage.count().await.unwrap(), 8);
    }

    #[tokio::test]
    async fn test_overwrite_key() {
        let storage = HashMapStorage::new();
        let value1 = Value::String("first".to_string());
        let value2 = Value::Int64(42);

        // Put initial value
        storage.put("key1", &value1).await.unwrap();
        assert_eq!(storage.get("key1").await.unwrap(), Some(value1.clone()));

        // Overwrite with different value
        storage.put("key1", &value2).await.unwrap();
        assert_eq!(storage.get("key1").await.unwrap(), Some(value2));
        assert_eq!(storage.count().await.unwrap(), 1); // Still only one key
    }

    #[tokio::test]
    async fn test_empty_batch_operations() {
        let storage = HashMapStorage::new();

        // Empty batch operations should not error
        storage.batch_put(vec![]).await.unwrap();
        storage.batch_get(vec![]).await.unwrap();
        storage.batch_delete(vec![]).await.unwrap();

        assert_eq!(storage.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_memory_usage_underflow_protection() {
        let storage = HashMapStorage::new();
        let value = Value::String("test".to_string());

        // Put and then delete
        storage.put("key1", &value).await.unwrap();
        storage.delete("key1").await.unwrap();

        // Memory usage should not underflow
        let _stats = storage.stats().await.unwrap();
        // Note: memory_usage is u64, so >= 0 is always true

        // Try to delete non-existent key (should not cause underflow)
        storage.delete("nonexistent").await.unwrap();
        let _stats_after = storage.stats().await.unwrap();
        // Note: memory_usage is u64, so >= 0 is always true
    }

    #[tokio::test]
    async fn test_empty_collection_handling() {
        let storage = HashMapStorage::new();

        // Operations on empty storage
        assert_eq!(storage.count().await.unwrap(), 0);
        assert_eq!(storage.keys().await.unwrap().len(), 0);
        assert_eq!(storage.get("nonexistent").await.unwrap(), None);
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Scan operations on empty storage
        assert_eq!(storage.scan_prefix("prefix").await.unwrap().len(), 0);
        assert_eq!(storage.scan_range("a", "z").await.unwrap().len(), 0);
        assert_eq!(storage.count_prefix("prefix").await.unwrap(), 0);
        assert_eq!(storage.count_range("a", "z").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_large_dataset_operations() {
        let storage = HashMapStorage::new();

        // Insert large number of keys
        let count = 1000;
        for i in 0..count {
            let key = format!("key_{}", i);
            let value = Value::String(format!("value_{}", i));
            storage.put(&key, &value).await.unwrap();
        }

        assert_eq!(storage.count().await.unwrap(), count);
        let keys = storage.keys().await.unwrap();
        assert_eq!(keys.len(), count as usize);

        // Verify all keys exist
        for i in 0..count {
            let key = format!("key_{}", i);
            assert!(storage.exists(&key).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_key_collision_scenarios() {
        let storage = HashMapStorage::new();

        // HashMap handles collisions internally, but we test overwriting
        let value1 = Value::String("first".to_string());
        let value2 = Value::String("second".to_string());
        let value3 = Value::Int64(42);

        // Put same key multiple times
        storage.put("collision_key", &value1).await.unwrap();
        assert_eq!(
            storage.get("collision_key").await.unwrap(),
            Some(value1.clone())
        );

        storage.put("collision_key", &value2).await.unwrap();
        assert_eq!(
            storage.get("collision_key").await.unwrap(),
            Some(value2.clone())
        );

        storage.put("collision_key", &value3).await.unwrap();
        assert_eq!(storage.get("collision_key").await.unwrap(), Some(value3));

        // Should still only have one key
        assert_eq!(storage.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_statistics_tracking_accuracy() {
        let storage = HashMapStorage::new();

        // Initial stats
        let initial_stats = storage.stats().await.unwrap();
        assert_eq!(initial_stats.key_count, 0);
        assert_eq!(initial_stats.total_operations, 0);
        assert_eq!(initial_stats.get_operations, 0);
        assert_eq!(initial_stats.put_operations, 0);
        assert_eq!(initial_stats.delete_operations, 0);
        assert_eq!(initial_stats.scan_operations, 0);

        // Perform operations
        let value = Value::String("test".to_string());
        storage.put("key1", &value).await.unwrap();
        storage.put("key2", &value).await.unwrap();
        storage.get("key1").await.unwrap();
        storage.get("key2").await.unwrap();
        storage.get("key3").await.unwrap(); // Non-existent
        storage.exists("key1").await.unwrap();
        storage.keys().await.unwrap();
        storage.scan_prefix("key").await.unwrap();
        storage.delete("key1").await.unwrap();

        // Verify stats
        let final_stats = storage.stats().await.unwrap();
        assert_eq!(final_stats.key_count, 1);
        assert_eq!(final_stats.put_operations, 2);
        assert_eq!(final_stats.get_operations, 3);
        assert_eq!(final_stats.delete_operations, 1);
        assert_eq!(final_stats.scan_operations, 1);
        assert!(final_stats.total_operations > 0);
    }

    #[tokio::test]
    async fn test_average_key_and_value_size_calculation() {
        let storage = HashMapStorage::new();

        // Add keys with different sizes
        storage
            .put("short", &Value::String("x".to_string()))
            .await
            .unwrap();
        storage
            .put("medium_key", &Value::String("medium_value".to_string()))
            .await
            .unwrap();
        storage
            .put(
                "very_long_key_name",
                &Value::String("very_long_value_content".to_string()),
            )
            .await
            .unwrap();

        let stats = storage.stats().await.unwrap();
        assert!(stats.average_key_size > 0.0);
        assert!(stats.average_value_size > 0.0);
        assert!(stats.average_key_size < 20.0); // Reasonable upper bound
        assert!(stats.average_value_size < 40.0); // Reasonable upper bound accounting for String overhead
    }

    #[tokio::test]
    async fn test_peak_memory_usage_tracking() {
        let storage = HashMapStorage::new();

        // Add data
        let value1 = Value::String("test1".to_string());
        storage.put("key1", &value1).await.unwrap();
        let stats1 = storage.stats().await.unwrap();
        let peak1 = stats1.peak_memory_usage;

        // Add more data
        let value2 = Value::String("test2".to_string());
        storage.put("key2", &value2).await.unwrap();
        let stats2 = storage.stats().await.unwrap();
        let peak2 = stats2.peak_memory_usage;

        // Peak should increase or stay the same
        assert!(peak2 >= peak1);

        // Delete data
        storage.delete("key2").await.unwrap();
        let stats3 = storage.stats().await.unwrap();
        let peak3 = stats3.peak_memory_usage;

        // Peak should remain at highest point
        assert!(peak3 >= peak2);
    }

    #[tokio::test]
    async fn test_scan_prefix_edge_cases() {
        let storage = HashMapStorage::new();

        // Add keys with various prefixes
        storage
            .put("prefix_key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_key2", &Value::String("value2".to_string()))
            .await
            .unwrap();
        storage
            .put("other_key", &Value::String("value3".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix", &Value::String("value4".to_string()))
            .await
            .unwrap();

        // Scan with prefix
        let prefix_keys = storage.scan_prefix("prefix").await.unwrap();
        assert_eq!(prefix_keys.len(), 3); // prefix_key1, prefix_key2, prefix

        // Scan with empty prefix (should return all keys)
        let all_keys = storage.scan_prefix("").await.unwrap();
        assert_eq!(all_keys.len(), 4);

        // Scan with non-matching prefix
        let no_keys = storage.scan_prefix("nonexistent").await.unwrap();
        assert_eq!(no_keys.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_range_edge_cases() {
        let storage = HashMapStorage::new();

        // Add keys in various ranges
        storage
            .put("a_key", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("m_key", &Value::String("value2".to_string()))
            .await
            .unwrap();
        storage
            .put("z_key", &Value::String("value3".to_string()))
            .await
            .unwrap();

        // Scan range
        let range_keys = storage.scan_range("a", "n").await.unwrap();
        assert!(range_keys.len() >= 2); // a_key and m_key

        // Scan with empty range
        let empty_range = storage.scan_range("z", "a").await.unwrap();
        assert_eq!(empty_range.len(), 0);

        // Scan with same start and end
        let same_range = storage.scan_range("a", "a").await.unwrap();
        assert_eq!(same_range.len(), 0);
    }

    #[tokio::test]
    async fn test_count_prefix_and_range() {
        let storage = HashMapStorage::new();

        // Add keys
        for i in 0..10 {
            let key = format!("prefix_{}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }
        for i in 0..5 {
            let key = format!("other_{}", i);
            storage
                .put(&key, &Value::String(format!("value_{}", i)))
                .await
                .unwrap();
        }

        // Count prefix
        let prefix_count = storage.count_prefix("prefix").await.unwrap();
        assert_eq!(prefix_count, 10);

        // Count range
        let range_count = storage.count_range("prefix_0", "prefix_9").await.unwrap();
        assert!(range_count >= 8); // At least 8 keys in range
    }

    #[tokio::test]
    async fn test_batch_operations_with_duplicates() {
        let storage = HashMapStorage::new();

        // Batch put with duplicate keys
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::String("value2".to_string())),
            (
                "key1".to_string(),
                Value::String("value1_updated".to_string()),
            ), // Duplicate
        ];

        storage.batch_put(items).await.unwrap();

        // Should have 2 keys (duplicate overwrites)
        assert_eq!(storage.count().await.unwrap(), 2);
        assert_eq!(
            storage.get("key1").await.unwrap(),
            Some(Value::String("value1_updated".to_string()))
        );
    }

    #[tokio::test]
    async fn test_scan_prefix_pairs() {
        let storage = HashMapStorage::new();

        storage
            .put("prefix_key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("prefix_key2", &Value::String("value2".to_string()))
            .await
            .unwrap();
        storage
            .put("other_key", &Value::String("value3".to_string()))
            .await
            .unwrap();

        let pairs = storage.scan_prefix_pairs("prefix").await.unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.iter().any(|(k, _)| k == "prefix_key1"));
        assert!(pairs.iter().any(|(k, _)| k == "prefix_key2"));
    }

    #[tokio::test]
    async fn test_scan_range_pairs() {
        let storage = HashMapStorage::new();

        storage
            .put("a_key", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage
            .put("m_key", &Value::String("value2".to_string()))
            .await
            .unwrap();
        storage
            .put("z_key", &Value::String("value3".to_string()))
            .await
            .unwrap();

        let pairs = storage.scan_range_pairs("a", "n").await.unwrap();
        assert!(pairs.len() >= 2);
        // Pairs should be sorted by key
        for i in 1..pairs.len() {
            assert!(pairs[i - 1].0 <= pairs[i].0);
        }
    }

    #[tokio::test]
    async fn test_flush_operation() {
        let storage = HashMapStorage::new();

        // Flush on empty storage
        storage.flush().await.unwrap();

        // Add data and flush
        storage
            .put("key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        storage.flush().await.unwrap();

        // Data should still be there (in-memory storage)
        assert_eq!(storage.count().await.unwrap(), 1);
    }
}
