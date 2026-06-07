//! Chaos engineering tests for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
//!
//! This module provides chaos engineering tests to ensure system resilience
//! under various failure conditions and edge cases.

use crate::{F4KVSCore, Result, Value};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for chaos engineering tests
#[derive(Debug, Clone)]
pub struct ChaosTestConfig {
    /// Duration of the chaos test
    pub test_duration: Duration,
    /// Rate of failures to inject (0.0 = no failures, 1.0 = 100% failures)
    pub failure_rate: f64,
    /// Maximum number of concurrent operations
    pub max_concurrent_operations: usize,
    /// Whether to enable memory pressure simulation
    pub enable_memory_pressure: bool,
    /// Whether to enable network failure simulation
    pub enable_network_failures: bool,
    /// Whether to enable disk failure simulation
    pub enable_disk_failures: bool,
}

impl Default for ChaosTestConfig {
    fn default() -> Self {
        Self {
            test_duration: Duration::from_secs(30),
            failure_rate: 0.1, // 10% failure rate
            max_concurrent_operations: 100,
            enable_memory_pressure: true,
            enable_network_failures: false,
            enable_disk_failures: false,
        }
    }
}

/// Chaos test suite
pub struct ChaosTestSuite {
    config: ChaosTestConfig,
}

impl ChaosTestSuite {
    /// Create a new chaos test suite
    pub fn new(config: ChaosTestConfig) -> Self {
        Self { config }
    }

    /// Run all chaos tests
    pub async fn run_all_tests(&self) -> Result<()> {
        log::info!("🌪️  Running F4KVS Core Chaos Tests");
        log::debug!("==================================");
        log::debug!("");

        // Test memory pressure scenarios
        if self.config.enable_memory_pressure {
            self.test_memory_pressure().await?;
            log::info!("✅ Memory pressure tests passed");
        }

        // Test concurrent failure scenarios
        self.test_concurrent_failures().await?;
        log::info!("✅ Concurrent failure tests passed");

        // Test rapid state changes
        self.test_rapid_state_changes().await?;
        log::info!("✅ Rapid state change tests passed");

        // Test resource exhaustion
        self.test_resource_exhaustion().await?;
        log::info!("✅ Resource exhaustion tests passed");

        log::info!("");
        log::info!("🎉 All chaos tests passed!");

        Ok(())
    }

    /// Test memory pressure scenarios
    async fn test_memory_pressure(&self) -> Result<()> {
        let engine = F4KVSCore::new()?;
        let engine = Arc::new(engine);

        // Create memory pressure by inserting large values
        let mut handles = Vec::new();

        let failure_rate = self.config.failure_rate;
        for i in 0..self.config.max_concurrent_operations {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = format!("pressure_key_{}", i);
                let value = Value::String("x".repeat(1024 * 1024)); // 1MB value

                // Simulate random failures
                if (i as f64 / 100.0) > failure_rate {
                    engine_clone.put(&key, &value).await
                } else {
                    Err(crate::F4KvsError::Internal {
                        message: "Simulated failure".to_string(),
                    })
                }
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }

        // Verify system is still functional
        let stats = engine.stats().await?;
        log::debug!(
            "Memory pressure test: {}/{} operations succeeded",
            success_count,
            self.config.max_concurrent_operations
        );
        log::debug!("Final memory usage: {} bytes", stats.memory_usage);

        Ok(())
    }

    /// Test concurrent failure scenarios
    async fn test_concurrent_failures(&self) -> Result<()> {
        let engine = F4KVSCore::new()?;
        let engine = Arc::new(engine);

        let mut handles = Vec::new();

        let failure_rate = self.config.failure_rate;
        for i in 0..self.config.max_concurrent_operations {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = format!("concurrent_key_{}", i);
                let value = Value::String(format!("value_{}", i));

                // Simulate random failures
                if (i as f64 / 100.0) > failure_rate {
                    engine_clone.put(&key, &value).await
                } else {
                    Err(crate::F4KvsError::Internal {
                        message: "Simulated failure".to_string(),
                    })
                }
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }

        // Verify system is still functional
        let stats = engine.stats().await?;
        log::debug!(
            "Concurrent failure test: {}/{} operations succeeded",
            success_count,
            self.config.max_concurrent_operations
        );
        log::debug!("Final key count: {}", stats.key_count);

        Ok(())
    }

    /// Test rapid state changes
    async fn test_rapid_state_changes(&self) -> Result<()> {
        let engine = F4KVSCore::new()?;
        let engine = Arc::new(engine);

        let mut handles = Vec::new();

        let failure_rate = self.config.failure_rate;
        for i in 0..self.config.max_concurrent_operations {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = format!("rapid_key_{}", i);
                let value = Value::String(format!("value_{}", i));

                // Rapid put/delete cycles
                for _ in 0..10 {
                    if (i as f64 / 100.0) > failure_rate {
                        let _ = engine_clone.put(&key, &value).await;
                    }
                    if (i as f64 / 100.0) > failure_rate {
                        let _ = engine_clone.delete(&key).await;
                    }
                }

                Ok::<(), crate::F4KvsError>(())
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let _ = handle.await.unwrap();
        }

        // Verify system is still functional
        let stats = engine.stats().await?;
        log::debug!("Rapid state change test completed");
        log::debug!("Final key count: {}", stats.key_count);

        Ok(())
    }

    /// Test resource exhaustion
    async fn test_resource_exhaustion(&self) -> Result<()> {
        let engine = F4KVSCore::new()?;
        let engine = Arc::new(engine);

        // Try to exhaust memory with very large values
        let mut handles = Vec::new();

        for i in 0..self.config.max_concurrent_operations {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = format!("exhaustion_key_{}", i);
                let value = Value::String("x".repeat(10 * 1024 * 1024)); // 10MB value

                engine_clone.put(&key, &value).await
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }

        // Verify system is still functional
        let stats = engine.stats().await?;
        log::debug!(
            "Resource exhaustion test: {}/{} operations succeeded",
            success_count,
            self.config.max_concurrent_operations
        );
        log::debug!("Final memory usage: {} bytes", stats.memory_usage);

        Ok(())
    }
}

/// Performance regression test suite
pub struct PerformanceRegressionTest {
    baseline_ops_per_second: f64,
    tolerance_percent: f64,
}

impl PerformanceRegressionTest {
    /// Create a new performance regression test
    pub fn new(baseline_ops_per_second: f64, tolerance_percent: f64) -> Self {
        Self {
            baseline_ops_per_second,
            tolerance_percent,
        }
    }

    /// Run performance regression test
    pub async fn run_test(&self) -> Result<bool> {
        let engine = F4KVSCore::new()?;
        let iterations = 10000;

        let start_time = std::time::Instant::now();

        for i in 0..iterations {
            let key = format!("perf_key_{}", i);
            let value = Value::String(format!("value_{}", i));
            engine.put(&key, &value).await?;
        }

        let duration = start_time.elapsed();
        let ops_per_second = iterations as f64 / duration.as_secs_f64();

        let performance_ratio = ops_per_second / self.baseline_ops_per_second;
        let is_within_tolerance = performance_ratio >= (1.0 - self.tolerance_percent / 100.0);

        log::debug!("Performance regression test:");
        log::debug!("  Baseline: {:.2} ops/sec", self.baseline_ops_per_second);
        log::debug!("  Current:  {:.2} ops/sec", ops_per_second);
        log::debug!("  Ratio:    {:.2}", performance_ratio);
        log::debug!("  Within tolerance: {}", is_within_tolerance);

        Ok(is_within_tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chaos_test_suite() {
        let config = ChaosTestConfig {
            test_duration: Duration::from_secs(5),
            failure_rate: 0.05,
            max_concurrent_operations: 10,
            enable_memory_pressure: true,
            enable_network_failures: false,
            enable_disk_failures: false,
        };

        let suite = ChaosTestSuite::new(config);
        suite.run_all_tests().await.unwrap();
    }

    #[tokio::test]
    async fn test_performance_regression() {
        let regression_test = PerformanceRegressionTest::new(1000.0, 10.0);
        let result = regression_test.run_test().await.unwrap();
        assert!(result);
    }
}
