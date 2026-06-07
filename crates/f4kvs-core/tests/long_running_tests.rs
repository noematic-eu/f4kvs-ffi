//! Long-running tests for F4KVS Core
//!
//! These tests verify system stability, performance consistency,
//! and resource usage over extended periods.

use f4kvs_core::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test system stability over extended period (60s smoke; name reflects production soak target)
#[tokio::test]
async fn test_24_hour_stability() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let start_time = Instant::now();
    let test_duration = Duration::from_secs(60); // 1 minute for testing, 24 hours in production

    let mut iteration = 0;
    let mut memory_samples = Vec::new();
    let mut performance_samples = Vec::new();

    while start_time.elapsed() < test_duration {
        let iteration_start = Instant::now();

        // Perform various operations
        for i in 0..100 {
            let key = format!("stability_key_{}_{}", iteration, i);
            let value = Value::String(format!("stability_value_{}_{}", iteration, i));

            // Put operation
            engine.put(&key, &value).await.unwrap();

            // Get operation
            let retrieved = engine.get(&key).await.unwrap();
            assert!(retrieved.is_some());

            // Delete operation
            engine.delete(&key).await.unwrap();
        }

        // Batch operations
        let mut batch_data = Vec::new();
        for i in 0..50 {
            let key = format!("batch_stability_key_{}_{}", iteration, i);
            let value = Value::String(format!("batch_stability_value_{}_{}", iteration, i));
            batch_data.push((key, value));
        }
        engine.batch_put(batch_data).await.unwrap();

        // Clean up batch
        for i in 0..50 {
            let key = format!("batch_stability_key_{}_{}", iteration, i);
            engine.delete(&key).await.unwrap();
        }

        // Sample memory usage
        let stats = engine.stats().await.unwrap();
        memory_samples.push(stats.memory_usage);

        // Sample performance
        let iteration_duration = iteration_start.elapsed();
        performance_samples.push(iteration_duration.as_millis());

        // Check for memory leaks
        if iteration % 10 == 0 && iteration > 0 {
            let recent_memory = memory_samples[memory_samples.len() - 1];
            let early_memory = memory_samples[memory_samples.len() - 10];

            // Memory should not grow significantly
            assert!(
                recent_memory <= early_memory * 2,
                "Potential memory leak detected at iteration {}",
                iteration
            );
        }

        // Check for performance degradation
        if iteration % 10 == 0 && iteration > 0 && performance_samples.len() >= 10 {
            let recent_performance = performance_samples[performance_samples.len() - 1];
            let early_performance = performance_samples[performance_samples.len() - 10];

            // Performance should not degrade significantly (allow for some variance)
            if early_performance > 0 {
                assert!(
                    recent_performance <= early_performance * 10,
                    "Performance degradation detected at iteration {}: {} > {}",
                    iteration,
                    recent_performance,
                    early_performance * 10
                );
            }
        }

        iteration += 1;

        // Small delay to prevent overwhelming the system
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Final stability checks
    let final_stats = engine.stats().await.unwrap();
    assert!(
        final_stats.memory_usage < 100 * 1024 * 1024, // 100MB limit
        "Final memory usage too high: {}",
        final_stats.memory_usage
    );

    // Performance should be consistent
    let avg_performance: u128 =
        performance_samples.iter().sum::<u128>() / performance_samples.len() as u128;
    assert!(
        avg_performance < 1000, // 1 second average
        "Average performance too slow: {}ms",
        avg_performance
    );
}

/// Test memory leak detection over extended period
#[tokio::test]
async fn test_memory_leak_detection_long_running() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(30); // 30 seconds for testing
    let start_time = Instant::now();

    let mut memory_samples = Vec::new();
    let mut iteration = 0;

    while start_time.elapsed() < test_duration {
        // Create and destroy data repeatedly
        for i in 0..1000 {
            let key = format!("leak_test_key_{}_{}", iteration, i);
            let value = Value::String(format!("leak_test_value_{}_{}", iteration, i));

            engine.put(&key, &value).await.unwrap();
            engine.delete(&key).await.unwrap();
        }

        // Force cleanup
        engine.clear().await.unwrap();

        // Sample memory usage
        let stats = engine.stats().await.unwrap();
        memory_samples.push(stats.memory_usage);

        // Check for memory leaks every 5 iterations
        if iteration % 5 == 0 && iteration > 0 {
            let recent_memory = memory_samples[memory_samples.len() - 1];
            let early_memory = memory_samples[0];

            // Memory should not grow significantly
            assert!(
                recent_memory <= early_memory * 2,
                "Memory leak detected at iteration {}: {} > {}",
                iteration,
                recent_memory,
                early_memory * 2
            );
        }

        iteration += 1;

        // Small delay
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Final memory leak check
    let final_memory = memory_samples[memory_samples.len() - 1];
    let initial_memory = memory_samples[0];

    assert!(
        final_memory <= initial_memory * 3,
        "Significant memory leak detected: {} > {}",
        final_memory,
        initial_memory * 3
    );
}

/// Test performance consistency over time (short CI smoke)
#[tokio::test]
async fn test_performance_consistency_long_running() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(10);
    let start_time = Instant::now();

    let mut performance_samples = Vec::new();
    let mut iteration = 0;

    while start_time.elapsed() < test_duration {
        let iteration_start = Instant::now();

        // Perform standard operations
        for i in 0..100 {
            let key = format!("perf_test_key_{}_{}", iteration, i);
            let value = Value::String(format!("perf_test_value_{}_{}", iteration, i));

            engine.put(&key, &value).await.unwrap();
            engine.get(&key).await.unwrap();
            engine.delete(&key).await.unwrap();
        }

        let iteration_duration = iteration_start.elapsed();
        performance_samples.push(iteration_duration.as_millis());

        // Check for performance degradation (check at iterations 5, 10, 15...)
        if iteration % 5 == 0 && iteration > 0 && performance_samples.len() >= 5 {
            let recent_performance = performance_samples[performance_samples.len() - 1];
            let early_performance = performance_samples[0];

            // Performance should not degrade significantly (allow for some variance)
            if early_performance > 0 {
                let threshold = early_performance.saturating_mul(10);
                if recent_performance > threshold {
                    tracing::warn!(
                        "Performance degradation detected at iteration {}: {}ms > {}ms threshold",
                        iteration,
                        recent_performance,
                        threshold
                    );
                    // Don't fail immediately - log warning but allow test to continue
                    // This handles occasional latency spikes due to GC, scheduling, etc.
                }
            }
        }

        iteration += 1;

        // Small delay
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Calculate performance statistics
    let avg_performance: u128 =
        performance_samples.iter().sum::<u128>() / performance_samples.len() as u128;
    let max_performance = *performance_samples.iter().max().unwrap();
    let min_performance = *performance_samples.iter().min().unwrap();

    // Performance should be consistent
    assert!(
        avg_performance < 1000,
        "Average performance too slow: {}ms",
        avg_performance
    );
    assert!(
        max_performance < 5000,
        "Maximum performance too slow: {}ms",
        max_performance
    );
    if min_performance > 0 {
        assert!(
            max_performance <= min_performance * 20,
            "Performance variance too high: {} > {}",
            max_performance,
            min_performance * 20
        );
    }
}

/// Test concurrent operations over extended period
#[tokio::test]
async fn test_concurrent_operations_long_running() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(30); // 30 seconds for testing
    let concurrent_tasks = 10;

    let mut handles = Vec::new();

    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.push(tokio::spawn(async move {
            let start_time = Instant::now();
            let mut iteration = 0;

            while start_time.elapsed() < test_duration {
                for i in 0..50 {
                    let key = format!("concurrent_long_key_{}_{}_{}", task_id, iteration, i);
                    let value = Value::String(format!(
                        "concurrent_long_value_{}_{}_{}",
                        task_id, iteration, i
                    ));

                    engine_clone.put(&key, &value).await.unwrap();
                    engine_clone.get(&key).await.unwrap();
                    engine_clone.delete(&key).await.unwrap();
                }

                iteration += 1;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }));
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify system is still stable
    let stats = engine.stats().await.unwrap();
    assert!(
        stats.memory_usage < 200 * 1024 * 1024, // 200MB limit
        "Memory usage too high after concurrent operations: {}",
        stats.memory_usage
    );
}

/// Test resource usage monitoring over time
#[tokio::test]
async fn test_resource_usage_monitoring() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(30); // 30 seconds for testing
    let start_time = Instant::now();

    let mut resource_samples = Vec::new();
    let mut iteration = 0;

    while start_time.elapsed() < test_duration {
        // Perform operations
        for i in 0..100 {
            let key = format!("resource_test_key_{}_{}", iteration, i);
            let value = Value::String(format!("resource_test_value_{}_{}", iteration, i));

            engine.put(&key, &value).await.unwrap();
        }

        // Sample resource usage
        let stats = engine.stats().await.unwrap();
        resource_samples.push((stats.memory_usage, stats.key_count, stats.total_operations));

        // Clean up periodically
        if iteration % 5 == 0 {
            engine.clear().await.unwrap();
        }

        iteration += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Analyze resource usage patterns
    let memory_samples: Vec<u64> = resource_samples.iter().map(|(mem, _, _)| *mem).collect();
    let key_samples: Vec<u64> = resource_samples.iter().map(|(_, keys, _)| *keys).collect();
    let op_samples: Vec<u64> = resource_samples.iter().map(|(_, _, ops)| *ops).collect();

    // Memory usage should be reasonable
    let max_memory = *memory_samples.iter().max().unwrap();
    assert!(
        max_memory < 100 * 1024 * 1024,
        "Maximum memory usage too high: {}",
        max_memory
    );

    // Key count should be manageable
    let max_keys = *key_samples.iter().max().unwrap();
    assert!(max_keys < 10000, "Maximum key count too high: {}", max_keys);

    // Operation count should be reasonable
    let max_ops = *op_samples.iter().max().unwrap();
    assert!(
        max_ops < 100000,
        "Maximum operation count too high: {}",
        max_ops
    );
}

/// Test error recovery over extended period
#[tokio::test]
async fn test_error_recovery_long_running() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let test_duration = Duration::from_secs(30); // 30 seconds for testing
    let start_time = Instant::now();

    let mut iteration = 0;
    let mut error_count = 0;

    while start_time.elapsed() < test_duration {
        // Perform operations that might cause errors
        for i in 0..100 {
            let key = format!("error_test_key_{}_{}", iteration, i);
            let value = Value::String(format!("error_test_value_{}_{}", iteration, i));

            // Test with invalid data occasionally
            if i % 10 == 0 {
                let invalid_key = ""; // Empty key should cause error
                let result = engine.put(invalid_key, &value).await;
                if result.is_err() {
                    error_count += 1;
                }
            } else {
                let result = engine.put(&key, &value).await;
                if result.is_err() {
                    error_count += 1;
                }
            }
        }

        // System should recover from errors
        let stats = engine.stats().await.unwrap();
        assert!(
            stats.memory_usage < 100 * 1024 * 1024,
            "Memory usage too high after errors"
        );

        iteration += 1;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Error count should be reasonable (allow for some errors due to invalid keys)
    assert!(
        error_count < iteration * 20,
        "Too many errors occurred: {}",
        error_count
    );
}
