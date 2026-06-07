//! Integration tests for F4KVS Core
//!
//! This module provides comprehensive integration tests that verify
//! the entire F4KVS Core system works correctly end-to-end.

use f4kvs_core::lockfree_cache::LockFreeCache;
use f4kvs_core::{
    BatchConfig, BatchOptimizer, Config, F4KVSCore, F4KvsError, MemoryPool, MemoryPoolConfig,
    Result, SimdBulkOps, SimdConfig, StorageMode, Value,
};
use std::sync::Arc;
use std::time::Duration;

/// Integration test suite
pub struct IntegrationTestSuite;

impl IntegrationTestSuite {
    /// Run all integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running F4KVS Core Integration Tests");
        println!("======================================");
        println!();

        // Test basic functionality
        Self::test_basic_functionality().await?;
        println!("✅ Basic functionality tests passed");

        // Test storage modes
        Self::test_storage_modes().await?;
        println!("✅ Storage modes tests passed");

        // Test batch operations
        Self::test_batch_operations().await?;
        println!("✅ Batch operations tests passed");

        // Test performance optimizations
        Self::test_performance_optimizations().await?;
        println!("✅ Performance optimizations tests passed");

        // Test error handling
        Self::test_error_handling().await?;
        println!("✅ Error handling tests passed");

        // Test concurrent operations
        Self::test_concurrent_operations().await?;
        println!("✅ Concurrent operations tests passed");

        // Test memory management
        Self::test_memory_management().await?;
        println!("✅ Memory management tests passed");

        // Test configuration
        Self::test_configuration().await?;
        println!("✅ Configuration tests passed");

        println!();
        println!("🎉 All integration tests passed!");

        Ok(())
    }

    /// Test basic functionality
    async fn test_basic_functionality() -> Result<()> {
        let engine = F4KVSCore::new()?;

        // Test put operation
        let value = Value::String("hello world".to_string());
        engine.put("test_key", &value).await?;

        // Test get operation
        let retrieved = engine.get("test_key").await?;
        assert_eq!(retrieved, Some(value.clone()));

        // Test exists operation
        let exists = engine.exists("test_key").await?;
        assert!(exists);

        // Test delete operation
        engine.delete("test_key").await?;

        // Test get after delete
        let retrieved_after_delete = engine.get("test_key").await?;
        assert_eq!(retrieved_after_delete, None);

        // Test exists after delete
        let exists_after_delete = engine.exists("test_key").await?;
        assert!(!exists_after_delete);

        Ok(())
    }

    /// Test different storage modes
    async fn test_storage_modes() -> Result<()> {
        // Test HashMap mode
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        let value = Value::String("hashmap_value".to_string());
        engine.put("hashmap_key", &value).await?;
        let retrieved = engine.get("hashmap_key").await?;
        assert_eq!(retrieved, Some(value));

        // Test BTreeMap mode
        let config = Config::new().with_storage_mode(StorageMode::BTreeMap);
        let engine = F4KVSCore::with_config(config)?;

        let value = Value::String("btreemap_value".to_string());
        engine.put("btreemap_key", &value).await?;
        let retrieved = engine.get("btreemap_key").await?;
        assert_eq!(retrieved, Some(value));

        Ok(())
    }

    /// Test batch operations
    async fn test_batch_operations() -> Result<()> {
        let engine = F4KVSCore::new()?;

        // Test batch put
        let items = vec![
            ("key1".to_string(), Value::String("value1".to_string())),
            ("key2".to_string(), Value::String("value2".to_string())),
            ("key3".to_string(), Value::String("value3".to_string())),
        ];
        engine.batch_put(items).await?;

        // Test batch get
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = engine.batch_get(keys).await?;
        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_some());

        // Test batch delete
        let keys_to_delete = vec!["key1".to_string(), "key2".to_string()];
        engine.batch_delete(keys_to_delete).await?;

        // Verify deletion
        let remaining_keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let remaining_results = engine.batch_get(remaining_keys).await?;
        assert_eq!(remaining_results[0], None); // key1 deleted
        assert_eq!(remaining_results[1], None); // key2 deleted
        assert!(remaining_results[2].is_some()); // key3 still exists

        Ok(())
    }

    /// Test performance optimizations
    async fn test_performance_optimizations() -> Result<()> {
        // Test SIMD operations
        let simd_config = SimdConfig::default();
        let simd_ops = SimdBulkOps::new(simd_config);

        let test_data = vec![1u8; 1024];
        let mut buffer = vec![0u8; 1024];
        let result = simd_ops.bulk_copy(&test_data, &mut buffer);
        assert!(result.is_ok());

        // Test memory pool
        let pool_config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(pool_config)?;
        let _layout = std::alloc::Layout::from_size_align(1024, 8).unwrap();
        let _ptr = pool.allocate()?;
        // ptr is a NonNull, so it's never null

        // Test lock-free cache
        let cache = LockFreeCache::new(100, 1000);
        assert!(cache.insert("test_key".to_string(), "test_value".to_string()));
        let retrieved = cache.get(&"test_key".to_string());
        assert_eq!(retrieved, Some("test_value".to_string()));

        // Test batch optimizer
        let batch_config = BatchConfig::default();
        let optimizer = BatchOptimizer::new(batch_config);

        let items = vec![
            ("key1".to_string(), vec![1, 2, 3, 4]),
            ("key2".to_string(), vec![5, 6, 7, 8]),
        ];
        let result = optimizer.batch_put_optimized(items)?;
        assert!(result.success_rate() > 0.0);

        Ok(())
    }

    /// Test error handling
    async fn test_error_handling() -> Result<()> {
        let engine = F4KVSCore::new()?;

        // Test with invalid key (too long)
        let long_key = "x".repeat(10000);
        let value = Value::String("test".to_string());
        let result = engine.put(&long_key, &value).await;
        assert!(result.is_err());

        // Test with invalid value (too large)
        let large_value = Value::String("x".repeat(11 * 1024 * 1024)); // 11MB, larger than default 10MB limit
        let result = engine.put("test_key", &large_value).await;
        assert!(result.is_err());

        // Test get with non-existent key
        let result = engine.get("non_existent_key").await?;
        assert_eq!(result, None);

        // Test delete with non-existent key
        let result = engine.delete("non_existent_key").await;
        assert!(result.is_ok()); // Delete should succeed even if key doesn't exist

        Ok(())
    }

    /// Test concurrent operations
    async fn test_concurrent_operations() -> Result<()> {
        let engine = Arc::new(F4KVSCore::new()?);
        let mut handles = Vec::new();

        // Spawn multiple concurrent operations
        for i in 0..10 {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                for j in 0..100 {
                    let key = format!("concurrent_key_{}_{}", i, j);
                    let value = Value::String(format!("value_{}_{}", i, j));

                    // Put operation
                    engine_clone.put(&key, &value).await?;

                    // Get operation
                    let retrieved = engine_clone.get(&key).await?;
                    assert_eq!(retrieved, Some(value));

                    // Delete operation
                    engine_clone.delete(&key).await?;
                }
                Ok::<(), F4KvsError>(())
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap()?;
        }

        Ok(())
    }

    /// Test memory management
    async fn test_memory_management() -> Result<()> {
        let engine = F4KVSCore::new()?;

        // Insert many items
        for i in 0..1000 {
            let key = format!("memory_key_{}", i);
            let value = Value::String(format!("value_{}", i));
            engine.put(&key, &value).await?;
        }

        // Check memory usage
        let stats = engine.stats().await?;
        assert!(stats.memory_usage > 0);
        assert_eq!(stats.key_count, 1000);

        // Delete half the items
        for i in 0..500 {
            let key = format!("memory_key_{}", i);
            engine.delete(&key).await?;
        }

        // Check memory usage after deletion
        let stats_after_delete = engine.stats().await?;
        assert_eq!(stats_after_delete.key_count, 500);
        assert!(stats_after_delete.memory_usage < stats.memory_usage);

        Ok(())
    }

    /// Test configuration
    async fn test_configuration() -> Result<()> {
        // Test custom configuration
        let config = Config {
            max_key_size: 100,
            max_value_size: 1000,
            operation_timeout: Duration::from_secs(5),
            strict_key_validation: true,
            storage_mode: StorageMode::HashMap,
            enable_monitoring: true,
            enable_memory_leak_detection: true,
        };

        let engine = F4KVSCore::with_config(config)?;

        // Test with valid key
        let value = Value::String("test".to_string());
        let result = engine.put("valid_key", &value).await;
        assert!(result.is_ok());

        // Test with key that's too long
        let long_key = "x".repeat(101);
        let result = engine.put(&long_key, &value).await;
        assert!(result.is_err());

        // Test with value that's too large
        let large_value = Value::String("x".repeat(1001));
        let result = engine.put("test_key", &large_value).await;
        assert!(result.is_err());

        Ok(())
    }
}

/// Performance benchmark integration test
pub struct PerformanceBenchmarkTest;

impl PerformanceBenchmarkTest {
    /// Run performance benchmark
    pub async fn run_benchmark() -> Result<()> {
        println!("📊 Running Performance Benchmark");
        println!("===============================");
        println!();

        let engine = F4KVSCore::new()?;
        let iterations = 10000;

        // Benchmark put operations
        let start_time = std::time::Instant::now();
        for i in 0..iterations {
            let key = format!("bench_key_{}", i);
            let value = Value::String(format!("value_{}", i));
            engine.put(&key, &value).await?;
        }
        let put_duration = start_time.elapsed();
        let put_ops_per_second = iterations as f64 / put_duration.as_secs_f64();

        // Benchmark get operations
        let start_time = std::time::Instant::now();
        for i in 0..iterations {
            let key = format!("bench_key_{}", i);
            let _ = engine.get(&key).await?;
        }
        let get_duration = start_time.elapsed();
        let get_ops_per_second = iterations as f64 / get_duration.as_secs_f64();

        // Benchmark delete operations
        let start_time = std::time::Instant::now();
        for i in 0..iterations {
            let key = format!("bench_key_{}", i);
            engine.delete(&key).await?;
        }
        let delete_duration = start_time.elapsed();
        let delete_ops_per_second = iterations as f64 / delete_duration.as_secs_f64();

        println!("Performance Results:");
        println!("  PUT operations:   {:.2} ops/sec", put_ops_per_second);
        println!("  GET operations:   {:.2} ops/sec", get_ops_per_second);
        println!("  DELETE operations: {:.2} ops/sec", delete_ops_per_second);
        println!();

        // Verify performance meets minimum requirements
        assert!(
            put_ops_per_second > 1000.0,
            "PUT operations too slow: {:.2} ops/sec",
            put_ops_per_second
        );
        assert!(
            get_ops_per_second > 1000.0,
            "GET operations too slow: {:.2} ops/sec",
            get_ops_per_second
        );
        assert!(
            delete_ops_per_second > 1000.0,
            "DELETE operations too slow: {:.2} ops/sec",
            delete_ops_per_second
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_suite() {
        IntegrationTestSuite::run_all_tests().await.unwrap();
    }

    #[tokio::test]
    async fn test_performance_benchmark() {
        PerformanceBenchmarkTest::run_benchmark().await.unwrap();
    }
}
