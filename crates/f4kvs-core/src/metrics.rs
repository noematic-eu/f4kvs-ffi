//! Performance metrics and monitoring for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{Result, StorageEngine, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total operations performed
    pub total_operations: u64,
    /// Operations per second
    pub operations_per_second: f64,
    /// Average operation latency in microseconds
    pub average_latency_us: f64,
    /// P50 latency in microseconds
    pub p50_latency_us: f64,
    /// P95 latency in microseconds
    pub p95_latency_us: f64,
    /// P99 latency in microseconds
    pub p99_latency_us: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Peak memory usage in bytes
    pub peak_memory_usage: u64,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Active connections
    pub active_connections: u32,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

/// Types of operations that can be measured
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// Get operation
    Get,
    /// Put operation
    Put,
    /// Delete operation
    Delete,
    /// Exists operation
    Exists,
    /// Scan operation
    Scan,
    /// Batch operation
    Batch,
    /// Clear operation
    Clear,
}

/// Operation timing data
#[derive(Debug, Clone)]
struct OperationTiming {
    #[allow(dead_code)]
    operation_type: OperationType,
    duration: Duration,
    #[allow(dead_code)]
    success: bool,
    #[allow(dead_code)]
    timestamp: Instant,
}

/// Metrics collector
pub struct MetricsCollector {
    timings: Arc<RwLock<Vec<OperationTiming>>>,
    start_time: Instant,
    operation_counts: Arc<RwLock<HashMap<OperationType, u64>>>,
    error_counts: Arc<RwLock<HashMap<OperationType, u64>>>,
    memory_samples: Arc<RwLock<Vec<u64>>>,
    max_timings: usize,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::with_max_timings(10000)
    }

    /// Create a new metrics collector with custom max timings
    pub fn with_max_timings(max_timings: usize) -> Self {
        Self {
            timings: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            operation_counts: Arc::new(RwLock::new(HashMap::new())),
            error_counts: Arc::new(RwLock::new(HashMap::new())),
            memory_samples: Arc::new(RwLock::new(Vec::new())),
            max_timings,
        }
    }

    /// Record an operation timing
    pub async fn record_operation(
        &self,
        operation_type: OperationType,
        duration: Duration,
        success: bool,
    ) {
        let timing = OperationTiming {
            operation_type,
            duration,
            success,
            timestamp: Instant::now(),
        };

        // Record timing
        {
            let mut timings = self.timings.write().await;
            timings.push(timing.clone());

            // Keep only recent timings to prevent memory growth
            if timings.len() > self.max_timings {
                let current_len = timings.len();
                let keep_count = self.max_timings;
                timings.drain(0..current_len - keep_count);
            }
        }

        // Update operation counts
        {
            let mut counts = self.operation_counts.write().await;
            *counts.entry(operation_type).or_insert(0) += 1;
        }

        // Update error counts
        if !success {
            let mut errors = self.error_counts.write().await;
            *errors.entry(operation_type).or_insert(0) += 1;
        }
    }

    /// Record memory usage sample
    pub async fn record_memory_usage(&self, usage: u64) {
        let mut samples = self.memory_samples.write().await;
        samples.push(usage);

        // Keep only recent samples
        if samples.len() > 1000 {
            let current_len = samples.len();
            let keep_count = 1000;
            samples.drain(0..current_len - keep_count);
        }
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let timings = self.timings.read().await;
        let operation_counts = self.operation_counts.read().await;
        let error_counts = self.error_counts.read().await;
        let memory_samples = self.memory_samples.read().await;

        let total_operations: u64 = operation_counts.values().sum();
        let total_errors: u64 = error_counts.values().sum();
        let uptime = self.start_time.elapsed();

        // Calculate operations per second
        let operations_per_second = if uptime.as_secs() > 0 {
            total_operations as f64 / uptime.as_secs() as f64
        } else {
            0.0
        };

        // Calculate latencies
        let mut durations: Vec<u64> = timings
            .iter()
            .map(|t| t.duration.as_micros() as u64)
            .collect();
        durations.sort();

        let average_latency_us = if !durations.is_empty() {
            durations.iter().sum::<u64>() as f64 / durations.len() as f64
        } else {
            0.0
        };

        let p50_latency_us = self.percentile(&durations, 50.0);
        let p95_latency_us = self.percentile(&durations, 95.0);
        let p99_latency_us = self.percentile(&durations, 99.0);

        // Calculate memory usage
        let memory_usage = memory_samples.last().copied().unwrap_or(0);
        let peak_memory_usage = memory_samples.iter().max().copied().unwrap_or(0);

        // Calculate error rate
        let error_rate = if total_operations > 0 {
            total_errors as f64 / total_operations as f64
        } else {
            0.0
        };

        PerformanceMetrics {
            total_operations,
            operations_per_second,
            average_latency_us,
            p50_latency_us,
            p95_latency_us,
            p99_latency_us,
            memory_usage,
            peak_memory_usage,
            cache_hit_rate: 0.0, // Will be set by cache layer
            error_rate,
            active_connections: 1, // Single-threaded for now
            uptime_seconds: uptime.as_secs(),
        }
    }

    /// Calculate percentile from sorted data
    fn percentile(&self, data: &[u64], percentile: f64) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let index = ((percentile / 100.0) * (data.len() - 1) as f64) as usize;
        data[index] as f64
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        {
            let mut timings = self.timings.write().await;
            timings.clear();
        }
        {
            let mut counts = self.operation_counts.write().await;
            counts.clear();
        }
        {
            let mut errors = self.error_counts.write().await;
            errors.clear();
        }
        {
            let mut samples = self.memory_samples.write().await;
            samples.clear();
        }
    }

    /// Get operation counts by type
    pub async fn get_operation_counts(&self) -> HashMap<OperationType, u64> {
        self.operation_counts.read().await.clone()
    }

    /// Get error counts by type
    pub async fn get_error_counts(&self) -> HashMap<OperationType, u64> {
        self.error_counts.read().await.clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance monitoring wrapper for storage engines
pub struct MonitoredStorageEngine {
    inner: Arc<dyn StorageEngine>,
    metrics: Arc<MetricsCollector>,
}

impl MonitoredStorageEngine {
    /// Create a new monitored storage engine
    pub fn new(inner: Arc<dyn StorageEngine>) -> Self {
        Self {
            inner,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Get performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.get_metrics().await
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        self.metrics.reset().await;
    }

    /// Get operation counts
    pub async fn get_operation_counts(&self) -> HashMap<OperationType, u64> {
        self.metrics.get_operation_counts().await
    }

    /// Get error counts
    pub async fn get_error_counts(&self) -> HashMap<OperationType, u64> {
        self.metrics.get_error_counts().await
    }
}

#[async_trait::async_trait]
impl StorageEngine for MonitoredStorageEngine {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        let start = Instant::now();
        let result = self.inner.get(key).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Get, duration, result.is_ok())
            .await;

        result
    }

    async fn put(&self, key: &str, value: &Value) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.put(key, value).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Put, duration, result.is_ok())
            .await;

        result
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.delete(key).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Delete, duration, result.is_ok())
            .await;

        result
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let start = Instant::now();
        let result = self.inner.exists(key).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Get, duration, result.is_ok())
            .await;

        result
    }

    async fn keys(&self) -> Result<Vec<String>> {
        let start = Instant::now();
        let result = self.inner.keys().await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn count(&self) -> Result<u64> {
        let start = Instant::now();
        let result = self.inner.count().await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn stats(&self) -> Result<crate::StorageStats> {
        let start = Instant::now();
        let result = self.inner.stats().await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Get, duration, result.is_ok())
            .await;

        result
    }

    async fn clear(&self) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.clear().await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Clear, duration, result.is_ok())
            .await;

        result
    }

    async fn batch_put(&self, items: Vec<(String, Value)>) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.batch_put(items).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Batch, duration, result.is_ok())
            .await;

        result
    }

    async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Value>>> {
        let start = Instant::now();
        let result = self.inner.batch_get(keys).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Batch, duration, result.is_ok())
            .await;

        result
    }

    async fn batch_delete(&self, keys: Vec<String>) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.batch_delete(keys).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Batch, duration, result.is_ok())
            .await;

        result
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let start = Instant::now();
        let result = self.inner.scan_prefix(prefix).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<String>> {
        let start_time = Instant::now();
        let result = self.inner.scan_range(start, end).await;
        let duration = start_time.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn scan_prefix_pairs(&self, prefix: &str) -> Result<Vec<(String, Value)>> {
        let start = Instant::now();
        let result = self.inner.scan_prefix_pairs(prefix).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn scan_range_pairs(&self, start: &str, end: &str) -> Result<Vec<(String, Value)>> {
        let start_time = Instant::now();
        let result = self.inner.scan_range_pairs(start, end).await;
        let duration = start_time.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn count_prefix(&self, prefix: &str) -> Result<u64> {
        let start = Instant::now();
        let result = self.inner.count_prefix(prefix).await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn count_range(&self, start: &str, end: &str) -> Result<u64> {
        let start_time = Instant::now();
        let result = self.inner.count_range(start, end).await;
        let duration = start_time.elapsed();

        self.metrics
            .record_operation(OperationType::Scan, duration, result.is_ok())
            .await;

        result
    }

    async fn flush(&self) -> Result<()> {
        let start = Instant::now();
        let result = self.inner.flush().await;
        let duration = start.elapsed();

        self.metrics
            .record_operation(OperationType::Put, duration, result.is_ok())
            .await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStorage, StorageMode};

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        // Record some operations
        collector
            .record_operation(OperationType::Get, Duration::from_micros(100), true)
            .await;
        collector
            .record_operation(OperationType::Put, Duration::from_micros(200), true)
            .await;
        collector
            .record_operation(OperationType::Get, Duration::from_micros(150), false)
            .await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.total_operations, 3);
        assert_eq!(metrics.error_rate, 1.0 / 3.0);
        assert!(metrics.average_latency_us > 0.0);
    }

    #[tokio::test]
    async fn test_monitored_storage_engine() {
        let storage = Arc::new(MemoryStorage::with_mode(StorageMode::HashMap));
        let monitored = MonitoredStorageEngine::new(storage);

        // Perform some operations
        monitored
            .put("key1", &Value::String("value1".to_string()))
            .await
            .unwrap();
        monitored.get("key1").await.unwrap();

        // Wait a bit for metrics to be calculated
        tokio::time::sleep(Duration::from_millis(10)).await;

        let metrics = monitored.get_metrics().await;
        assert_eq!(metrics.total_operations, 2);
        assert!(metrics.operations_per_second >= 0.0);
    }

    #[tokio::test]
    async fn test_percentile_calculation() {
        let collector = MetricsCollector::new();

        // Add some test data
        for i in 1..=100 {
            collector
                .record_operation(OperationType::Get, Duration::from_micros(i), true)
                .await;
        }

        let metrics = collector.get_metrics().await;
        assert!(metrics.p50_latency_us > 0.0);
        assert!(metrics.p95_latency_us > 0.0);
        assert!(metrics.p99_latency_us > 0.0);
    }
}
