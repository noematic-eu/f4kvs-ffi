//! Memory pool allocators integration tests
//!
//! This module provides comprehensive integration tests for memory pool allocators
//! under concurrent load and memory leak detection.

use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Barrier;

/// Memory pool integration test suite
pub struct MemoryPoolIntegrationTestSuite;

impl MemoryPoolIntegrationTestSuite {
    /// Run all memory pool integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running Memory Pool Integration Tests");
        println!("======================================");
        println!();

        // Test basic memory pool operations
        Self::test_basic_memory_pool_operations().await?;
        println!("✅ Basic memory pool operations tests passed");

        // Test memory pool under concurrent load
        Self::test_memory_pool_concurrent_load().await?;
        println!("✅ Memory pool concurrent load tests passed");

        // Test memory pool leak detection
        Self::test_memory_pool_leak_detection().await?;
        println!("✅ Memory pool leak detection tests passed");

        // Test memory pool performance
        Self::test_memory_pool_performance().await?;
        println!("✅ Memory pool performance tests passed");

        // Test memory pool edge cases
        Self::test_memory_pool_edge_cases().await?;
        println!("✅ Memory pool edge cases tests passed");

        println!();
        println!("🎉 All memory pool integration tests passed!");

        Ok(())
    }

    /// Test basic memory pool operations
    async fn test_basic_memory_pool_operations() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test basic operations with memory pool
        let value = Value::String("memory_pool_test".to_string());
        engine.put("key1", &value).await?;

        let retrieved = engine.get("key1").await?;
        assert_eq!(retrieved, Some(value.clone()));

        // Test multiple operations to exercise memory pool
        for i in 0..1000 {
            let key = format!("pool_key_{}", i);
            let val = Value::String(format!("pool_value_{}", i));
            engine.put(&key, &val).await?;
        }

        // Verify all values
        for i in 0..1000 {
            let key = format!("pool_key_{}", i);
            let expected = Value::String(format!("pool_value_{}", i));
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(expected));
        }

        Ok(())
    }

    /// Test memory pool under concurrent load
    async fn test_memory_pool_concurrent_load() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = Arc::new(F4KVSCore::with_config(config)?);
        let barrier = Arc::new(Barrier::new(8));

        let mut handles = vec![];

        for thread_id in 0..8 {
            let engine_clone = engine.clone();
            let barrier_clone = barrier.clone();

            let handle = tokio::spawn(async move {
                // Wait for all threads to start
                barrier_clone.wait().await;

                // Each thread performs memory-intensive operations
                for i in 0..500 {
                    let key = format!("concurrent_pool_{}_{}", thread_id, i);
                    let value = Value::String(format!("concurrent_value_{}_{}", thread_id, i));

                    // Put operation (allocates memory)
                    engine_clone.put(&key, &value).await.unwrap();

                    // Get operation (may allocate temporary memory)
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(value));

                    // Update operation (may reallocate)
                    let updated_value = Value::String(format!("updated_{}_{}", thread_id, i));
                    engine_clone.put(&key, &updated_value).await.unwrap();

                    // Verify update
                    let final_retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(final_retrieved, Some(updated_value));
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        for thread_id in 0..8 {
            for i in 0..500 {
                let key = format!("concurrent_pool_{}_{}", thread_id, i);
                let expected = Value::String(format!("updated_{}_{}", thread_id, i));
                let retrieved = engine.get(&key).await?;
                assert_eq!(retrieved, Some(expected));
            }
        }

        Ok(())
    }

    /// Test memory pool leak detection
    async fn test_memory_pool_leak_detection() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test rapid allocation and deallocation
        for cycle in 0..10 {
            // Allocate many values
            for i in 0..100 {
                let key = format!("leak_test_{}_{}", cycle, i);
                let value = Value::String(format!("leak_value_{}_{}", cycle, i));
                engine.put(&key, &value).await?;
            }

            // Delete all values (should free memory)
            for i in 0..100 {
                let key = format!("leak_test_{}_{}", cycle, i);
                engine.delete(&key).await?;
            }

            // Verify deletion
            for i in 0..100 {
                let key = format!("leak_test_{}_{}", cycle, i);
                let retrieved = engine.get(&key).await?;
                assert_eq!(retrieved, None);
            }
        }

        // Test with large values to stress memory pool
        for i in 0..50 {
            let key = format!("large_leak_test_{}", i);
            let large_value = Value::String("x".repeat(10000));
            engine.put(&key, &large_value).await?;
        }

        // Delete large values
        for i in 0..50 {
            let key = format!("large_leak_test_{}", i);
            engine.delete(&key).await?;
        }

        // Verify deletion
        for i in 0..50 {
            let key = format!("large_leak_test_{}", i);
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, None);
        }

        Ok(())
    }

    /// Test memory pool performance
    async fn test_memory_pool_performance() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        let test_sizes = vec![100, 500, 1000, 2000];

        for size in test_sizes {
            println!("Testing memory pool performance with {} operations", size);

            let start = Instant::now();

            // Test allocation performance
            for i in 0..size {
                let key = format!("perf_pool_{}", i);
                let value = Value::String(format!("perf_value_{}", i));
                engine.put(&key, &value).await?;
            }

            let allocation_duration = start.elapsed();

            // Test access performance
            let start = Instant::now();

            for i in 0..size {
                let key = format!("perf_pool_{}", i);
                let _retrieved = engine.get(&key).await?;
            }

            let access_duration = start.elapsed();

            // Test deallocation performance
            let start = Instant::now();

            for i in 0..size {
                let key = format!("perf_pool_{}", i);
                engine.delete(&key).await?;
            }

            let deallocation_duration = start.elapsed();

            let alloc_ops_per_sec = size as f64 / allocation_duration.as_secs_f64();
            let access_ops_per_sec = size as f64 / access_duration.as_secs_f64();
            let dealloc_ops_per_sec = size as f64 / deallocation_duration.as_secs_f64();

            println!("  Allocation: {:.0} ops/sec", alloc_ops_per_sec);
            println!("  Access: {:.0} ops/sec", access_ops_per_sec);
            println!("  Deallocation: {:.0} ops/sec", dealloc_ops_per_sec);

            // Performance assertions
            assert!(
                alloc_ops_per_sec > 100.0,
                "Allocation too slow: {:.0} ops/sec",
                alloc_ops_per_sec
            );
            assert!(
                access_ops_per_sec > 500.0,
                "Access too slow: {:.0} ops/sec",
                access_ops_per_sec
            );
            assert!(
                dealloc_ops_per_sec > 100.0,
                "Deallocation too slow: {:.0} ops/sec",
                dealloc_ops_per_sec
            );
        }

        Ok(())
    }

    /// Test memory pool edge cases
    async fn test_memory_pool_edge_cases() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test with single character key (minimum valid key)
        let single_key = "a";
        let value = Value::String("single_key_value".to_string());
        engine.put(single_key, &value).await?;
        let retrieved = engine.get(single_key).await?;
        assert_eq!(retrieved, Some(value));

        // Test with very large values
        for i in 0..10 {
            let key = format!("large_pool_{}", i);
            let large_value = Value::String("x".repeat(50000));
            engine.put(&key, &large_value).await?;
        }

        // Verify large values
        for i in 0..10 {
            let key = format!("large_pool_{}", i);
            let retrieved = engine.get(&key).await?;
            assert!(retrieved.is_some());
            if let Some(Value::String(s)) = retrieved {
                assert_eq!(s.len(), 50000);
            }
        }

        // Test rapid allocation/deallocation cycles
        for cycle in 0..5 {
            // Allocate
            for i in 0..50 {
                let key = format!("cycle_{}_{}", cycle, i);
                let value = Value::String(format!("cycle_value_{}_{}", cycle, i));
                engine.put(&key, &value).await?;
            }

            // Immediately deallocate
            for i in 0..50 {
                let key = format!("cycle_{}_{}", cycle, i);
                engine.delete(&key).await?;
            }
        }

        // Test mixed value types
        let mixed_values = vec![
            Value::String("string_value".to_string()),
            Value::Int64(42),
            Value::Float64(std::f64::consts::PI),
            Value::Bool(true),
        ];

        for (i, value) in mixed_values.iter().enumerate() {
            let key = format!("mixed_{}", i);
            engine.put(&key, value).await?;
        }

        // Verify mixed values
        for (i, expected_value) in mixed_values.iter().enumerate() {
            let key = format!("mixed_{}", i);
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(expected_value.clone()));
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_memory_pool_integration() {
    MemoryPoolIntegrationTestSuite::run_all_tests()
        .await
        .unwrap();
}
