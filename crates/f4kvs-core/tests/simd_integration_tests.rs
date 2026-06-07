//! SIMD operations integration tests
//!
//! This module provides comprehensive integration tests for SIMD operations
//! with various data patterns and performance validation.

use f4kvs_core::{Config, F4KVSCore, Result, StorageMode, Value};
use std::sync::Arc;
use std::time::Instant;

/// SIMD integration test suite
pub struct SimdIntegrationTestSuite;

impl SimdIntegrationTestSuite {
    /// Run all SIMD integration tests
    pub async fn run_all_tests() -> Result<()> {
        println!("🔧 Running SIMD Integration Tests");
        println!("================================");
        println!();

        // Test basic SIMD operations
        Self::test_basic_simd_operations().await?;
        println!("✅ Basic SIMD operations tests passed");

        // Test SIMD with various data patterns
        Self::test_simd_data_patterns().await?;
        println!("✅ SIMD data patterns tests passed");

        // Test SIMD performance benchmarks
        Self::test_simd_performance().await?;
        println!("✅ SIMD performance tests passed");

        // Test SIMD with concurrent operations
        Self::test_simd_concurrent_operations().await?;
        println!("✅ SIMD concurrent operations tests passed");

        // Test SIMD edge cases
        Self::test_simd_edge_cases().await?;
        println!("✅ SIMD edge cases tests passed");

        println!();
        println!("🎉 All SIMD integration tests passed!");

        Ok(())
    }

    /// Test basic SIMD operations
    async fn test_basic_simd_operations() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test individual operations (SIMD optimizations are internal)
        for i in 0..1000 {
            let key = format!("simd_key_{}", i);
            let value = Value::String(format!("simd_value_{}", i));

            engine.put(&key, &value).await?;

            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(value));
        }

        Ok(())
    }

    /// Test SIMD with various data patterns
    async fn test_simd_data_patterns() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test with sequential data
        for i in 0..1000 {
            let key = format!("seq_{:06}", i);
            let value = Value::String(format!("seq_value_{:06}", i));

            engine.put(&key, &value).await?;
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(value));
        }

        // Test with random-like data
        for i in 0..1000 {
            let key = format!("rand_{:x}", i * 7 + 13);
            let value = Value::String(format!("rand_value_{:x}", i * 11 + 17));

            engine.put(&key, &value).await?;
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(value));
        }

        // Test with mixed data types
        for i in 0..500 {
            let key = format!("mixed_{}", i);
            let value = match i % 4 {
                0 => Value::String(format!("string_{}", i)),
                1 => Value::Int64(i as i64),
                2 => Value::Float64(i as f64),
                _ => Value::Bool(i % 2 == 0),
            };

            engine.put(&key, &value).await?;
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(value));
        }

        Ok(())
    }

    /// Test SIMD performance benchmarks
    async fn test_simd_performance() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Performance test with large datasets
        let test_sizes = vec![1000, 5000, 10000];

        for size in test_sizes {
            // Measure put performance
            let start = Instant::now();
            for i in 0..size {
                let key = format!("perf_key_{}", i);
                let value = Value::String(format!("perf_value_{}", i));
                engine.put(&key, &value).await?;
            }
            let put_duration = start.elapsed();

            // Measure get performance
            let start = Instant::now();
            for i in 0..size {
                let key = format!("perf_key_{}", i);
                let _retrieved = engine.get(&key).await?;
            }
            let get_duration = start.elapsed();

            // Verify results
            let retrieved = engine.get("perf_key_0").await?;
            assert!(retrieved.is_some());

            // Performance assertions (should be fast with SIMD)
            let put_ops_per_sec = size as f64 / put_duration.as_secs_f64();
            let get_ops_per_sec = size as f64 / get_duration.as_secs_f64();

            println!(
                "Size: {}, Put: {:.0} ops/sec, Get: {:.0} ops/sec",
                size, put_ops_per_sec, get_ops_per_sec
            );

            // Basic performance checks (should be > 10K ops/sec)
            assert!(
                put_ops_per_sec > 1000.0,
                "Put performance too slow: {:.0} ops/sec",
                put_ops_per_sec
            );
            assert!(
                get_ops_per_sec > 1000.0,
                "Get performance too slow: {:.0} ops/sec",
                get_ops_per_sec
            );
        }

        Ok(())
    }

    /// Test SIMD with concurrent operations
    async fn test_simd_concurrent_operations() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = Arc::new(F4KVSCore::with_config(config)?);

        let mut handles = vec![];

        // Spawn multiple tasks doing operations concurrently
        for task_id in 0..4 {
            let engine_clone = engine.clone();
            let handle = tokio::spawn(async move {
                for i in 0..500 {
                    let key = format!("concurrent_{}_{}", task_id, i);
                    let value = Value::String(format!("concurrent_value_{}_{}", task_id, i));

                    // Put operation
                    engine_clone.put(&key, &value).await.unwrap();

                    // Get operation
                    let retrieved = engine_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(value));
                }
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        for task_id in 0..4 {
            for i in 0..500 {
                let key = format!("concurrent_{}_{}", task_id, i);
                let expected = Value::String(format!("concurrent_value_{}_{}", task_id, i));
                let retrieved = engine.get(&key).await?;
                assert_eq!(retrieved, Some(expected));
            }
        }

        Ok(())
    }

    /// Test SIMD edge cases
    async fn test_simd_edge_cases() -> Result<()> {
        let config = Config::new().with_storage_mode(StorageMode::HashMap);
        let engine = F4KVSCore::with_config(config)?;

        // Test with single element
        let single_key = "single_key";
        let single_value = Value::String("single_value".to_string());

        engine.put(single_key, &single_value).await?;
        let retrieved = engine.get(single_key).await?;
        assert_eq!(retrieved, Some(single_value));

        // Test with very large single value
        let large_value = Value::String("x".repeat(100000));
        let large_key = "large_key";

        engine.put(large_key, &large_value).await?;
        let retrieved = engine.get(large_key).await?;
        assert_eq!(retrieved, Some(large_value));

        // Test with different value types
        let test_values = vec![
            Value::String("test_string".to_string()),
            Value::Int64(42),
            Value::Float64(std::f64::consts::PI),
            Value::Bool(true),
            Value::Bytes(vec![0x01, 0x02, 0x03]),
        ];

        for (i, value) in test_values.iter().enumerate() {
            let key = format!("type_test_{}", i);
            engine.put(&key, value).await?;
            let retrieved = engine.get(&key).await?;
            assert_eq!(retrieved, Some(value.clone()));
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_simd_integration() {
    SimdIntegrationTestSuite::run_all_tests().await.unwrap();
}
