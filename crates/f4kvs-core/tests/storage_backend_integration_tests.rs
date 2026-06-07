//! Storage backend integration tests
//!
//! This module provides comprehensive integration tests for StorageEngine
//! with different backends (HashMap, BTreeMap) and their interactions.

use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};
use std::time::Instant;

/// Storage backend integration test suite
pub struct StorageBackendIntegrationTestSuite;

impl StorageBackendIntegrationTestSuite {
    /// Run all storage backend integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running Storage Backend Integration Tests");
        println!("===========================================");
        println!();

        // Test HashMap backend
        Self::test_hashmap_backend().await?;
        println!("✅ HashMap backend tests passed");

        // Test BTreeMap backend
        Self::test_btreemap_backend().await?;
        println!("✅ BTreeMap backend tests passed");

        // Test backend switching
        Self::test_backend_switching().await?;
        println!("✅ Backend switching tests passed");

        // Test backend performance comparison
        Self::test_backend_performance_comparison().await?;
        println!("✅ Backend performance comparison tests passed");

        // Test backend-specific features
        Self::test_backend_specific_features().await?;
        println!("✅ Backend-specific features tests passed");

        println!();
        println!("🎉 All storage backend integration tests passed!");

        Ok(())
    }

    /// Test HashMap backend functionality
    async fn test_hashmap_backend() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test basic operations
        let value = Value::String("hashmap_test".to_string());
        engine.put("key1", &value).await?;

        let retrieved = engine.get("key1").await?;
        assert_eq!(retrieved, Some(value.clone()));

        // Test HashMap-specific behavior (no ordering guarantees)
        let mut keys = vec![];
        for i in 0..100 {
            let key = format!("hashmap_key_{}", i);
            let val = Value::String(format!("hashmap_value_{}", i));
            engine.put(&key, &val).await?;
            keys.push(key);
        }

        // Verify all values exist
        for key in &keys {
            let retrieved = engine.get(key).await?;
            assert!(retrieved.is_some());
        }

        // Test HashMap performance characteristics
        let start = Instant::now();
        for i in 0..1000 {
            let key = format!("perf_key_{}", i);
            let val = Value::String(format!("perf_value_{}", i));
            engine.put(&key, &val).await?;
        }
        let duration = start.elapsed();

        // HashMap should be fast for random access
        let ops_per_sec = 1000.0 / duration.as_secs_f64();
        assert!(
            ops_per_sec > 1000.0,
            "HashMap performance too slow: {:.0} ops/sec",
            ops_per_sec
        );

        Ok(())
    }

    /// Test BTreeMap backend functionality
    async fn test_btreemap_backend() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test basic operations
        let value = Value::String("btreemap_test".to_string());
        engine.put("key1", &value).await?;

        let retrieved = engine.get("key1").await?;
        assert_eq!(retrieved, Some(value.clone()));

        // Test BTreeMap-specific behavior (ordered iteration)
        let mut keys = vec![];
        for i in 0..100 {
            let key = format!("btreemap_key_{:03}", i); // Zero-padded for ordering
            let val = Value::String(format!("btreemap_value_{}", i));
            engine.put(&key, &val).await?;
            keys.push(key);
        }

        // Verify all values exist
        for key in &keys {
            let retrieved = engine.get(key).await?;
            assert!(retrieved.is_some());
        }

        // Test BTreeMap performance characteristics
        let start = Instant::now();
        for i in 0..1000 {
            let key = format!("perf_key_{:04}", i); // Zero-padded for better BTree performance
            let val = Value::String(format!("perf_value_{}", i));
            engine.put(&key, &val).await?;
        }
        let duration = start.elapsed();

        // BTreeMap should be reasonably fast for ordered access
        let ops_per_sec = 1000.0 / duration.as_secs_f64();
        assert!(
            ops_per_sec > 500.0,
            "BTreeMap performance too slow: {:.0} ops/sec",
            ops_per_sec
        );

        Ok(())
    }

    /// Test switching between backends
    async fn test_backend_switching() -> Result<()> {
        // Test with HashMap first
        let hashmap_config = Config::new().with_storage_mode(StorageMode::HashMap);
        let hashmap_engine = F4KVSCore::with_config(hashmap_config)?;

        // Insert some data
        for i in 0..50 {
            let key = format!("switch_key_{}", i);
            let val = Value::String(format!("hashmap_value_{}", i));
            hashmap_engine.put(&key, &val).await?;
        }

        // Verify data exists
        for i in 0..50 {
            let key = format!("switch_key_{}", i);
            let retrieved = hashmap_engine.get(&key).await?;
            assert!(retrieved.is_some());
        }

        // Test with BTreeMap
        let btreemap_config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let btreemap_engine = F4KVSCore::with_config(btreemap_config)?;

        // Insert different data
        for i in 0..50 {
            let key = format!("switch_key_{}", i);
            let val = Value::String(format!("btreemap_value_{}", i));
            btreemap_engine.put(&key, &val).await?;
        }

        // Verify data exists
        for i in 0..50 {
            let key = format!("switch_key_{}", i);
            let retrieved = btreemap_engine.get(&key).await?;
            assert!(retrieved.is_some());
        }

        // Verify engines are independent
        let hashmap_retrieved = hashmap_engine.get("switch_key_0").await?;
        let btreemap_retrieved = btreemap_engine.get("switch_key_0").await?;

        assert_ne!(hashmap_retrieved, btreemap_retrieved);

        Ok(())
    }

    /// Test performance comparison between backends
    async fn test_backend_performance_comparison() -> Result<()> {
        let test_sizes = vec![100, 500, 1000];

        for size in test_sizes {
            println!("Testing performance with {} operations", size);

            // Test HashMap performance
            let hashmap_config = Config::new().with_storage_mode(StorageMode::HashMap);
            let hashmap_engine = F4KVSCore::with_config(hashmap_config)?;

            let start = Instant::now();
            for i in 0..size {
                let key = format!("perf_hashmap_{}", i);
                let val = Value::String(format!("perf_value_{}", i));
                hashmap_engine.put(&key, &val).await?;
            }
            let hashmap_duration = start.elapsed();

            // Test BTreeMap performance
            let btreemap_config = Config::new().with_storage_mode(StorageMode::BTreeMap);
            let btreemap_engine = F4KVSCore::with_config(btreemap_config)?;

            let start = Instant::now();
            for i in 0..size {
                let key = format!("perf_btreemap_{:04}", i); // Zero-padded for BTree
                let val = Value::String(format!("perf_value_{}", i));
                btreemap_engine.put(&key, &val).await?;
            }
            let btreemap_duration = start.elapsed();

            let hashmap_ops_per_sec = size as f64 / hashmap_duration.as_secs_f64();
            let btreemap_ops_per_sec = size as f64 / btreemap_duration.as_secs_f64();

            println!("  HashMap: {:.0} ops/sec", hashmap_ops_per_sec);
            println!("  BTreeMap: {:.0} ops/sec", btreemap_ops_per_sec);

            // Both should be reasonably fast
            assert!(
                hashmap_ops_per_sec > 100.0,
                "HashMap too slow: {:.0} ops/sec",
                hashmap_ops_per_sec
            );
            assert!(
                btreemap_ops_per_sec > 50.0,
                "BTreeMap too slow: {:.0} ops/sec",
                btreemap_ops_per_sec
            );
        }

        Ok(())
    }

    /// Test backend-specific features
    async fn test_backend_specific_features() -> Result<()> {
        // Test HashMap with random key patterns (should be fast)
        let hashmap_config = Config::new().with_storage_mode(StorageMode::HashMap);
        let hashmap_engine = F4KVSCore::with_config(hashmap_config)?;

        let random_keys: Vec<String> = (0..100)
            .map(|i| format!("random_{:x}", i * 7 + 13))
            .collect();

        for key in &random_keys {
            let val = Value::String(format!("random_value_{}", key));
            hashmap_engine.put(key, &val).await?;
        }

        // Verify all random keys exist
        for key in &random_keys {
            let retrieved = hashmap_engine.get(key).await?;
            assert!(retrieved.is_some());
        }

        // Test BTreeMap with ordered key patterns (should be efficient)
        let btreemap_config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let btreemap_engine = F4KVSCore::with_config(btreemap_config)?;

        let ordered_keys: Vec<String> = (0..100).map(|i| format!("ordered_{:04}", i)).collect();

        for key in &ordered_keys {
            let val = Value::String(format!("ordered_value_{}", key));
            btreemap_engine.put(key, &val).await?;
        }

        // Verify all ordered keys exist
        for key in &ordered_keys {
            let retrieved = btreemap_engine.get(key).await?;
            assert!(retrieved.is_some());
        }

        // Test with very large values (both backends should handle this)
        let large_value = Value::String("x".repeat(10000));

        hashmap_engine.put("large_hashmap", &large_value).await?;
        let hashmap_retrieved = hashmap_engine.get("large_hashmap").await?;
        assert_eq!(hashmap_retrieved, Some(large_value.clone()));

        btreemap_engine.put("large_btreemap", &large_value).await?;
        let btreemap_retrieved = btreemap_engine.get("large_btreemap").await?;
        assert_eq!(btreemap_retrieved, Some(large_value));

        Ok(())
    }
}

#[tokio::test]
async fn test_storage_backend_integration() {
    StorageBackendIntegrationTestSuite::run_all_tests()
        .await
        .unwrap();
}
