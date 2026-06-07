//! Memory leak detection utilities for F4KVS Core
//!
//! This module provides utilities to detect potential memory leaks in production.
//! It tracks memory usage over time and can alert when memory growth patterns
//! suggest a leak.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

/// Memory leak detection configuration
#[derive(Debug, Clone)]
pub struct MemoryLeakDetectionConfig {
    /// Enable memory leak detection
    pub enabled: bool,
    /// Sample interval in seconds
    pub sample_interval: Duration,
    /// Number of samples to keep for trend analysis
    pub sample_history_size: usize,
    /// Memory growth threshold (percentage) to trigger warning
    pub growth_threshold_percent: f64,
    /// Minimum memory usage (bytes) before leak detection is active
    pub min_memory_threshold: u64,
    /// Time window (seconds) to analyze for leaks
    pub analysis_window: Duration,
}

impl Default for MemoryLeakDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_interval: Duration::from_secs(60),
            sample_history_size: 100,
            growth_threshold_percent: 10.0, // 10% growth triggers warning
            min_memory_threshold: 1024 * 1024, // 1MB minimum
            analysis_window: Duration::from_secs(3600), // 1 hour window
        }
    }
}

/// Memory sample point
#[derive(Debug, Clone)]
struct MemorySample {
    timestamp: Instant,
    memory_usage: u64,
}

/// Memory leak detector
pub struct MemoryLeakDetector {
    config: MemoryLeakDetectionConfig,
    samples: Arc<RwLock<VecDeque<MemorySample>>>,
    #[allow(dead_code)]
    is_running: Arc<AtomicBool>,
    last_sample_time: Arc<RwLock<Instant>>,
    leak_detected: Arc<AtomicBool>,
}

impl MemoryLeakDetector {
    /// Create a new memory leak detector
    pub fn new(config: MemoryLeakDetectionConfig) -> Self {
        Self {
            config,
            samples: Arc::new(RwLock::new(VecDeque::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            last_sample_time: Arc::new(RwLock::new(Instant::now())),
            leak_detected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a disabled memory leak detector (no-op)
    pub fn disabled() -> Self {
        Self::new(MemoryLeakDetectionConfig {
            enabled: false,
            ..Default::default()
        })
    }

    /// Record a memory usage sample
    pub async fn record_sample(&self, memory_usage: u64) {
        if !self.config.enabled {
            return;
        }

        let now = Instant::now();
        let sample = MemorySample {
            timestamp: now,
            memory_usage,
        };

        let mut samples = self.samples.write().await;
        samples.push_back(sample);

        // Keep only the configured number of samples
        while samples.len() > self.config.sample_history_size {
            samples.pop_front();
        }

        *self.last_sample_time.write().await = now;

        // Check for leaks
        if samples.len() >= 10 {
            // Need at least 10 samples to detect trends
            self.check_for_leaks(&samples).await;
        }
    }

    /// Check for memory leaks in the sample history
    async fn check_for_leaks(&self, samples: &VecDeque<MemorySample>) {
        if samples.len() < 10 {
            return;
        }

        let now = Instant::now();
        let window_start = now - self.config.analysis_window;

        // Filter samples within the analysis window
        let recent_samples: Vec<&MemorySample> = samples
            .iter()
            .filter(|s| s.timestamp >= window_start)
            .collect();

        if recent_samples.len() < 5 {
            return; // Not enough samples in the window
        }

        // Calculate memory growth rate
        let first_memory = recent_samples[0].memory_usage;
        let last_memory = recent_samples[recent_samples.len() - 1].memory_usage;

        if first_memory < self.config.min_memory_threshold {
            return; // Memory usage too low to be concerned
        }

        if first_memory == 0 {
            return; // Avoid division by zero
        }

        let growth_percent =
            ((last_memory as f64 - first_memory as f64) / first_memory as f64) * 100.0;

        if growth_percent > self.config.growth_threshold_percent {
            let leak_detected = self.leak_detected.load(Ordering::Relaxed);
            if !leak_detected {
                warn!(
                    memory_usage = last_memory,
                    growth_percent = growth_percent,
                    "Potential memory leak detected: memory grew by {:.2}% over analysis window",
                    growth_percent
                );
                self.leak_detected.store(true, Ordering::Relaxed);
            }
        } else {
            // Reset leak flag if memory growth is normal
            self.leak_detected.store(false, Ordering::Relaxed);
        }
    }

    /// Get current memory leak status
    pub async fn is_leak_detected(&self) -> bool {
        self.leak_detected.load(Ordering::Relaxed)
    }

    /// Get memory growth statistics
    pub async fn get_growth_stats(&self) -> Option<MemoryGrowthStats> {
        let samples = self.samples.read().await;
        if samples.len() < 2 {
            return None;
        }

        let first = samples.front()?;
        let last = samples.back()?;

        let time_delta = last.timestamp.duration_since(first.timestamp);
        let memory_delta = last.memory_usage as i64 - first.memory_usage as i64;

        let growth_percent = if first.memory_usage > 0 {
            (memory_delta as f64 / first.memory_usage as f64) * 100.0
        } else {
            0.0
        };

        Some(MemoryGrowthStats {
            initial_memory: first.memory_usage,
            current_memory: last.memory_usage,
            memory_delta,
            growth_percent,
            time_window: time_delta,
            sample_count: samples.len(),
        })
    }

    /// Reset leak detection state
    pub async fn reset(&self) {
        self.samples.write().await.clear();
        self.leak_detected.store(false, Ordering::Relaxed);
        *self.last_sample_time.write().await = Instant::now();
    }
}

/// Memory growth statistics
#[derive(Debug, Clone)]
pub struct MemoryGrowthStats {
    /// Initial memory usage (bytes)
    pub initial_memory: u64,
    /// Current memory usage (bytes)
    pub current_memory: u64,
    /// Memory delta (bytes, can be negative)
    pub memory_delta: i64,
    /// Growth percentage
    pub growth_percent: f64,
    /// Time window analyzed
    pub time_window: Duration,
    /// Number of samples analyzed
    pub sample_count: usize,
}

/// Simple memory usage tracker
pub struct MemoryUsageTracker {
    current_usage: Arc<AtomicU64>,
    peak_usage: Arc<AtomicU64>,
}

impl MemoryUsageTracker {
    /// Create a new memory usage tracker
    pub fn new() -> Self {
        Self {
            current_usage: Arc::new(AtomicU64::new(0)),
            peak_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update current memory usage
    pub fn update(&self, usage: u64) {
        self.current_usage.store(usage, Ordering::Relaxed);
        let mut peak = self.peak_usage.load(Ordering::Relaxed);
        while usage > peak {
            match self.peak_usage.compare_exchange_weak(
                peak,
                usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new_peak) => peak = new_peak,
            }
        }
    }

    /// Get current memory usage
    pub fn current(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Get peak memory usage
    pub fn peak(&self) -> u64 {
        self.peak_usage.load(Ordering::Relaxed)
    }

    /// Reset peak usage
    pub fn reset_peak(&self) {
        let current = self.current_usage.load(Ordering::Relaxed);
        self.peak_usage.store(current, Ordering::Relaxed);
    }
}

impl Default for MemoryUsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_leak_detector() {
        let config = MemoryLeakDetectionConfig {
            enabled: true,
            sample_interval: Duration::from_millis(100),
            sample_history_size: 50,
            growth_threshold_percent: 10.0,
            min_memory_threshold: 1000,
            analysis_window: Duration::from_secs(10),
        };

        let detector = MemoryLeakDetector::new(config);

        // Record samples showing growth
        for i in 0..20 {
            let memory = 1000 + (i * 100) as u64; // Growing memory
            detector.record_sample(memory).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Should detect leak
        let leak_detected = detector.is_leak_detected().await;
        assert!(
            leak_detected,
            "Should detect memory leak with growing memory"
        );

        // Get growth stats
        let stats = detector.get_growth_stats().await;
        assert!(stats.is_some());
        if let Some(stats) = stats {
            assert!(stats.growth_percent > 0.0);
        }
    }

    #[test]
    fn test_memory_usage_tracker() {
        let tracker = MemoryUsageTracker::new();

        tracker.update(1000);
        assert_eq!(tracker.current(), 1000);
        assert_eq!(tracker.peak(), 1000);

        tracker.update(2000);
        assert_eq!(tracker.current(), 2000);
        assert_eq!(tracker.peak(), 2000);

        tracker.update(1500);
        assert_eq!(tracker.current(), 1500);
        assert_eq!(tracker.peak(), 2000); // Peak should remain

        tracker.reset_peak();
        assert_eq!(tracker.peak(), 1500); // Peak reset to current
    }
}
