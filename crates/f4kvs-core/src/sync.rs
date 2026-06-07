//! Synchronous API for F4KVS Core
//!
//! This module provides synchronous wrappers around the async F4KVS Core API,
//! making it easier to use in non-async environments like embedded systems,
//! simple scripts, or when you prefer a blocking API.
//!
//! ## Usage
//!
//! ```rust
//! use f4kvs_core::sync::F4KVSCoreSync;
//! use f4kvs_core::{Config, Value, StorageMode};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a synchronous F4KVS instance
//!     let engine = F4KVSCoreSync::new()?;
//!
//!     // All operations are now synchronous
//!     engine.put("key1", &Value::String("hello".to_string()))?;
//!     let value = engine.get("key1")?;
//!     println!("Retrieved: {:?}", value);
//!
//!     Ok(())
//! }
//! ```
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::engine::F4KVSCore;
use crate::{Config, EngineInfo, F4KvsError, Result, StorageMode, StorageStats, Value};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Synchronous wrapper around F4KVS Core
///
/// This struct provides a synchronous API by maintaining an internal Tokio runtime
/// and blocking on async operations. It's designed for use in non-async environments
/// or when you prefer a simpler blocking API.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` and can be safely shared across threads.
/// Each operation will use the internal runtime to execute the async operations.
pub struct F4KVSCoreSync {
    engine: F4KVSCore,
    runtime: Arc<Runtime>,
}

impl F4KVSCoreSync {
    /// Create a new synchronous F4KVS core instance with default configuration
    pub fn new() -> Result<Self> {
        let runtime =
            Arc::new(Runtime::new().map_err(|e| {
                F4KvsError::internal(format!("Failed to create Tokio runtime: {}", e))
            })?);

        let engine = runtime.block_on(async { F4KVSCore::new() })?;

        Ok(Self { engine, runtime })
    }

    /// Create a new synchronous F4KVS core instance with custom configuration
    pub fn with_config(config: Config) -> Result<Self> {
        let runtime =
            Arc::new(Runtime::new().map_err(|e| {
                F4KvsError::internal(format!("Failed to create Tokio runtime: {}", e))
            })?);

        let engine = runtime.block_on(async { F4KVSCore::with_config(config) })?;

        Ok(Self { engine, runtime })
    }

    /// Create a new synchronous F4KVS core instance with custom storage mode
    pub fn with_storage_mode(storage_mode: StorageMode) -> Result<Self> {
        let config = Config::new().with_storage_mode(storage_mode);
        Self::with_config(config)
    }

    /// Get a value by key (synchronous)
    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        self.runtime.block_on(async { self.engine.get(key).await })
    }

    /// Put a key-value pair (synchronous)
    pub fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.runtime
            .block_on(async { self.engine.put(key, value).await })
    }

    /// Delete a key (synchronous)
    pub fn delete(&self, key: &str) -> Result<()> {
        self.runtime
            .block_on(async { self.engine.delete(key).await })
    }

    /// Check if a key exists (synchronous)
    pub fn exists(&self, key: &str) -> Result<bool> {
        self.runtime
            .block_on(async { self.engine.exists(key).await })
    }

    /// Get all keys (synchronous)
    pub fn keys(&self) -> Result<Vec<String>> {
        self.runtime.block_on(async { self.engine.keys().await })
    }

    /// Get count of keys (synchronous)
    pub fn count(&self) -> Result<u64> {
        self.runtime.block_on(async { self.engine.count().await })
    }

    /// Batch put multiple key-value pairs (synchronous)
    pub fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        self.runtime
            .block_on(async { self.engine.batch_put(items).await })
    }

    /// Batch get multiple values by keys (synchronous)
    pub fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        self.runtime
            .block_on(async { self.engine.batch_get(keys).await })
    }

    /// Batch delete multiple keys (synchronous)
    pub fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        self.runtime
            .block_on(async { self.engine.batch_delete(keys).await })
    }

    /// Get storage statistics (synchronous)
    pub fn stats(&self) -> Result<StorageStats> {
        self.runtime.block_on(async { self.engine.stats().await })
    }

    /// Clear all data (synchronous)
    pub fn clear(&self) -> Result<()> {
        self.runtime.block_on(async { self.engine.clear().await })
    }

    /// Health check (synchronous)
    pub fn health_check(&self) -> Result<bool> {
        self.runtime
            .block_on(async { self.engine.health_check().await })
    }

    /// Get current configuration
    pub fn config(&self) -> &Config {
        self.engine.config()
    }

    /// Get engine information
    pub fn info(&self) -> EngineInfo {
        self.engine.info()
    }

    /// Get a reference to the underlying async engine
    ///
    /// This allows you to use the async API directly if needed, while still
    /// having access to the synchronous wrapper.
    pub fn async_engine(&self) -> &F4KVSCore {
        &self.engine
    }

    /// Convert to async engine, consuming the sync wrapper
    ///
    /// This is useful when you want to transition from sync to async usage.
    pub fn into_async(self) -> F4KVSCore {
        self.engine
    }
}

impl Default for F4KVSCoreSync {
    fn default() -> Self {
        Self::new().expect("Failed to create default F4KVS Core Sync instance")
    }
}

// Thread safety implementations
unsafe impl Send for F4KVSCoreSync {}
unsafe impl Sync for F4KVSCoreSync {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn test_basic_sync_operations() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Test put and get
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .expect("Failed to put test key");
        let value = engine.get("test_key").expect("Failed to get test key");
        assert_eq!(value, Some(Value::String("test_value".to_string())));

        // Test exists
        assert!(engine
            .exists("test_key")
            .expect("Failed to check key existence"));
        assert!(!engine
            .exists("nonexistent")
            .expect("Failed to check nonexistent key"));

        // Test delete
        engine
            .delete("test_key")
            .expect("Failed to delete test key");
        assert!(!engine
            .exists("test_key")
            .expect("Failed to check deleted key"));
    }

    #[test]
    fn test_sync_validation() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Test empty key validation
        let result = engine.put("", &Value::String("value".to_string()));
        assert!(result.is_err());

        // Test key length validation
        let long_key = "a".repeat(engine.config().max_key_size + 1);
        let result = engine.put(&long_key, &Value::String("value".to_string()));
        assert!(result.is_err());

        // Test value size validation
        let large_value = Value::String("a".repeat(engine.config().max_value_size + 1));
        let result = engine.put("key", &large_value);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_health_check() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");
        assert!(engine.health_check().expect("Health check failed"));
    }

    #[test]
    fn test_sync_stats_and_info() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Test initial stats
        let stats = engine.stats().expect("Failed to get initial stats");
        assert_eq!(stats.key_count, 0);

        // Add some data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .expect("Failed to put key2");

        // Check updated stats
        let stats = engine.stats().expect("Failed to get updated stats");
        assert_eq!(stats.key_count, 2);
        assert!(stats.memory_usage > 0);

        // Test info
        let info = engine.info();
        assert_eq!(info.name, "F4KVS-Core");
        assert!(info.features.contains(&"async".to_string()));
    }

    #[test]
    fn test_sync_keys_and_count() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Add test data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .expect("Failed to put key2");
        engine
            .put("key3", &Value::Bool(true))
            .expect("Failed to put key3");

        // Test count
        assert_eq!(engine.count().expect("Failed to get count"), 3);

        // Test keys
        let keys = engine.keys().expect("Failed to get keys");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[test]
    fn test_sync_clear() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Add test data
        engine
            .put("key1", &Value::String("value1".to_string()))
            .expect("Failed to put key1");
        engine
            .put("key2", &Value::Int64(42))
            .expect("Failed to put key2");

        assert_eq!(engine.count().expect("Failed to get count"), 2);

        // Clear all data
        engine.clear().expect("Failed to clear data");
        assert_eq!(engine.count().expect("Failed to get count after clear"), 0);
        assert!(engine
            .keys()
            .expect("Failed to get keys after clear")
            .is_empty());
    }

    #[test]
    fn test_sync_batch_operations() {
        let engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Test batch put
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::String("value2".to_string())),
            ("key3".to_string(), Value::Int64(42)),
        ];

        engine.batch_put(items).expect("Failed to batch put");

        // Test batch get
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = engine.batch_get(keys).expect("Failed to batch get");

        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_some());

        // Test batch delete
        let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
        engine
            .batch_delete(keys_to_delete)
            .expect("Failed to batch delete");

        // Verify deletion
        let remaining = engine.get("key3").expect("Failed to get remaining key");
        assert!(remaining.is_some());

        let deleted = engine.get("key1").expect("Failed to get deleted key");
        assert!(deleted.is_none());
    }

    #[test]
    fn test_sync_storage_mode_configuration() {
        // Test BTreeMap mode
        let engine = F4KVSCoreSync::with_storage_mode(StorageMode::BTreeMap)
            .expect("Failed to create BTreeMap engine");

        // Test basic operations work
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .expect("Failed to put");
        let value = engine.get("test_key").expect("Failed to get");
        assert_eq!(value, Some(Value::String("test_value".to_string())));

        // Test HashMap mode
        let engine = F4KVSCoreSync::with_storage_mode(StorageMode::HashMap)
            .expect("Failed to create HashMap engine");

        // Test basic operations work
        engine
            .put("test_key", &Value::String("test_value".to_string()))
            .expect("Failed to put");
        let value = engine.get("test_key").expect("Failed to get");
        assert_eq!(value, Some(Value::String("test_value".to_string())));
    }

    #[test]
    fn test_sync_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let engine = Arc::new(F4KVSCoreSync::new().expect("Failed to create sync engine"));

        // Spawn multiple threads that perform operations
        let mut handles = vec![];
        for thread_id in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = format!("thread{}_key{}", thread_id, i);
                    let value = Value::String(format!("thread{}_value{}", thread_id, i));

                    engine_clone.put(&key, &value).expect("Failed to put");
                    let retrieved = engine_clone.get(&key).expect("Failed to get");
                    assert_eq!(retrieved, Some(value));
                }
                format!("Thread {} completed", thread_id)
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            let result = handle.join().expect("Thread panicked");
            log::debug!("{}", result);
        }

        // Verify final state
        let count = engine.count().expect("Failed to get count");
        assert_eq!(count, 1000); // 10 threads * 100 keys each
    }

    #[test]
    fn test_sync_into_async() {
        let sync_engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Add some data using sync API
        sync_engine
            .put("test_key", &Value::String("test_value".to_string()))
            .expect("Failed to put");

        // Convert to async engine
        let async_engine = sync_engine.into_async();

        // Use async engine (we need a runtime for this)
        let rt = Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            let value = async_engine.get("test_key").await.expect("Failed to get");
            assert_eq!(value, Some(Value::String("test_value".to_string())));
        });
    }

    #[test]
    fn test_sync_async_engine_access() {
        let sync_engine = F4KVSCoreSync::new().expect("Failed to create sync engine");

        // Add some data using sync API
        sync_engine
            .put("test_key", &Value::String("test_value".to_string()))
            .expect("Failed to put");

        // Access the async engine directly
        let async_engine = sync_engine.async_engine();

        // Use async engine (we need a runtime for this)
        let rt = Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            let value = async_engine.get("test_key").await.expect("Failed to get");
            assert_eq!(value, Some(Value::String("test_value".to_string())));
        });
    }
}
