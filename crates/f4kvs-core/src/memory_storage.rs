//! Memory storage implementation for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::{BTreeMapStorage, HashMapStorage};
use crate::config::StorageMode;
use crate::{Result, StorageEngine, StorageStats, Value};
use async_trait::async_trait;

/// Memory storage that can use either HashMap or BTreeMap
pub struct MemoryStorage {
    inner: Box<dyn StorageEngine>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStorage {
    /// Create a new memory storage instance with BTreeMap (default for compatibility)
    pub fn new() -> Self {
        Self::with_mode(StorageMode::BTreeMap)
    }

    /// Create a new memory storage instance with specified mode
    pub fn with_mode(mode: StorageMode) -> Self {
        let inner: Box<dyn StorageEngine> = match mode {
            StorageMode::BTreeMap => Box::new(BTreeMapStorage::new()),
            StorageMode::HashMap => Box::new(HashMapStorage::new()),
        };
        Self { inner }
    }

    /// Get current memory usage for validation/testing
    pub async fn get_current_memory_usage(&self) -> u64 {
        match self.stats().await {
            Ok(stats) => stats.memory_usage,
            Err(_) => 0, // Return 0 if stats retrieval fails
        }
    }
}

#[async_trait]
impl StorageEngine for MemoryStorage {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        self.inner.get(key).await
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.inner.put(key, value).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.inner.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.inner.exists(key).await
    }

    async fn keys(&self) -> Result<Vec<String>> {
        self.inner.keys().await
    }

    async fn count(&self) -> Result<u64> {
        self.inner.count().await
    }

    async fn stats(&self) -> Result<StorageStats> {
        self.inner.stats().await
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear().await
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        self.inner.batch_put(items).await
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        self.inner.batch_get(keys).await
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        self.inner.batch_delete(keys).await
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
#[cfg(feature = "proptest")]
mod proptest_tests {
    use super::*;
    #[cfg(feature = "proptest")]
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn test_memory_storage_operations_property(
            operations in prop::collection::vec(
                prop_oneof![
                    (any::<String>(), any::<Vec<u8>>()).prop_map(|(k, v)| ("put", k, v)),
                    any::<String>().prop_map(|k| ("get", k, vec![])),
                    any::<String>().prop_map(|k| ("delete", k, vec![])),
                    any::<String>().prop_map(|k| ("exists", k, vec![])),
                ],
                0..100
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = MemoryStorage::new();

            let mut expected_keys = std::collections::HashSet::new();

            for (op, key, value) in operations {
                match op {
                    "put" => {
                        let value = Value::Bytes(value);
                        rt.block_on(storage.put(&key, &value)).unwrap();
                        expected_keys.insert(key.clone());
                    }
                    "get" => {
                        let result = rt.block_on(storage.get(&key));
                        if expected_keys.contains(&key) {
                            prop_assert!(result.is_ok() && result.unwrap().is_some());
                        } else {
                            prop_assert!(result.is_ok() && result.unwrap().is_none());
                        }
                    }
                    "delete" => {
                        let result = rt.block_on(storage.delete(&key));
                        expected_keys.remove(&key);
                        prop_assert!(result.is_ok());
                    }
                    "exists" => {
                        let exists = rt.block_on(storage.exists(&key));
                        prop_assert_eq!(exists.unwrap(), expected_keys.contains(&key));
                    }
                    _ => {}
                }
            }
        }

        #[test]
        fn test_memory_storage_scan_property(
            data in prop::collection::vec(
                (any::<String>(), any::<Vec<u8>>()),
                0..50
            ),
            prefix in any::<String>()
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = MemoryStorage::new();

            // Insert test data
            for (key, value) in &data {
                let value = Value::Bytes(value.clone());
                rt.block_on(storage.put(key, &value)).unwrap();
            }

            // Test prefix scan
            let scanned_keys = rt.block_on(storage.scan_prefix(&prefix)).unwrap();
            for key in &scanned_keys {
                prop_assert!(key.starts_with(&prefix));
            }

            // Test that all keys with the prefix are included
            for (key, _) in &data {
                if key.starts_with(&prefix) {
                    prop_assert!(scanned_keys.contains(key));
                }
            }
        }

        #[test]
        fn test_memory_storage_range_scan_property(
            data in prop::collection::vec(
                (any::<String>(), any::<Vec<u8>>()),
                0..50
            ),
            start in any::<String>(),
            end in any::<String>()
        ) {
            // Skip invalid ranges where start >= end
            if start >= end {
                return Ok(());
            }
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = MemoryStorage::new();

            // Insert test data
            for (key, value) in &data {
                let value = Value::Bytes(value.clone());
                rt.block_on(storage.put(key, &value)).unwrap();
            }

            // Test range scan
            let scanned_keys = rt.block_on(storage.scan_range(&start, &end)).unwrap();
            for key in &scanned_keys {
                prop_assert!(key >= &start && key < &end);
            }
        }

        #[test]
        fn test_memory_storage_stats_property(
            operations in prop::collection::vec(
                (any::<String>(), any::<Vec<u8>>()),
                0..100
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = MemoryStorage::new();

            // Insert test data
            for (key, value) in &operations {
                let value = Value::Bytes(value.clone());
                rt.block_on(storage.put(key, &value)).unwrap();
            }

            // Test stats
            let _stats = rt.block_on(storage.stats()).unwrap();
            // Note: key_count and memory_usage are u64, so >= 0 is always true
        }
    }
}
