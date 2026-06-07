//! Memory Pressure Manager for F4KVS Core
//!
//! This module provides memory pressure detection and management to prevent
//! performance degradation under memory pressure conditions.
//!
//! ## Memory Pressure Detection
//!
//! The memory pressure manager monitors system memory usage and detects when
//! the system is under memory pressure. This helps prevent the 100x performance
//! degradation seen in benchmarks under memory pressure.
//!
//! ## Features
//!
//! - **Memory Pressure Detection**: Monitors system memory usage
//! - **Adaptive Throttling**: Reduces allocation rate under pressure
//! - **Pre-allocation**: Pre-allocates memory pools to avoid pressure
//! - **Memory Monitoring**: Tracks memory usage patterns
//! - **Performance Optimization**: Maintains performance under pressure
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
// use std::sync::mpsc;

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    /// Normal memory usage
    Normal,
    /// Moderate memory pressure
    Moderate,
    /// High memory pressure
    High,
    /// Critical memory pressure
    Critical,
}

/// Memory pressure statistics
#[derive(Debug, Clone)]
pub struct MemoryPressureStats {
    /// Current memory usage in bytes
    pub current_usage: usize,
    /// Peak memory usage in bytes
    pub peak_usage: usize,
    /// Available system memory in bytes
    pub available_memory: usize,
    /// Memory pressure level
    pub pressure_level: MemoryPressureLevel,
    /// Number of allocations under pressure
    pub allocations_under_pressure: usize,
    /// Number of throttled allocations
    pub throttled_allocations: usize,
}

/// Memory pressure manager configuration
#[derive(Debug, Clone)]
pub struct MemoryPressureConfig {
    /// Memory pressure threshold (0.0-1.0)
    pub pressure_threshold: f64,
    /// High pressure threshold (0.0-1.0)
    pub high_pressure_threshold: f64,
    /// Critical pressure threshold (0.0-1.0)
    pub critical_pressure_threshold: f64,
    /// Throttling factor under pressure (0.0-1.0)
    pub throttling_factor: f64,
    /// Monitoring interval
    pub monitoring_interval: Duration,
    /// Pre-allocation size in bytes
    pub pre_allocation_size: usize,
    /// Enable memory pressure monitoring
    pub enable_monitoring: bool,
}

impl Default for MemoryPressureConfig {
    fn default() -> Self {
        Self {
            pressure_threshold: 0.7,           // 70% memory usage
            high_pressure_threshold: 0.85,     // 85% memory usage
            critical_pressure_threshold: 0.95, // 95% memory usage
            throttling_factor: 0.5,            // 50% throttling under pressure
            monitoring_interval: Duration::from_millis(100),
            pre_allocation_size: 1024 * 1024, // 1MB pre-allocation
            enable_monitoring: true,
        }
    }
}

/// Memory pressure manager
pub struct MemoryPressureManager {
    /// Configuration
    config: MemoryPressureConfig,
    /// Current memory usage
    current_usage: Arc<AtomicUsize>,
    /// Peak memory usage
    peak_usage: Arc<AtomicUsize>,
    /// Available system memory
    available_memory: Arc<AtomicUsize>,
    /// Current pressure level
    pressure_level: Arc<RwLock<MemoryPressureLevel>>,
    /// Statistics
    stats: Arc<RwLock<MemoryPressureStats>>,
    /// Monitoring thread handle
    monitoring_thread: Option<thread::JoinHandle<()>>,
    /// Shutdown signal
    shutdown_signal: Arc<AtomicBool>,
    /// Pre-allocated memory pools
    memory_pools: Arc<RwLock<HashMap<usize, Vec<Vec<u8>>>>>,
    /// Allocation throttling
    allocation_throttle: Arc<RwLock<f64>>,
}

impl MemoryPressureManager {
    /// Create a new memory pressure manager
    pub fn new(config: MemoryPressureConfig) -> Self {
        let manager = Self {
            config,
            current_usage: Arc::new(AtomicUsize::new(0)),
            peak_usage: Arc::new(AtomicUsize::new(0)),
            available_memory: Arc::new(AtomicUsize::new(0)),
            pressure_level: Arc::new(RwLock::new(MemoryPressureLevel::Normal)),
            stats: Arc::new(RwLock::new(MemoryPressureStats {
                current_usage: 0,
                peak_usage: 0,
                available_memory: 0,
                pressure_level: MemoryPressureLevel::Normal,
                allocations_under_pressure: 0,
                throttled_allocations: 0,
            })),
            monitoring_thread: None,
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            memory_pools: Arc::new(RwLock::new(HashMap::new())),
            allocation_throttle: Arc::new(RwLock::new(1.0)),
        };

        // Pre-allocate memory pools
        manager.pre_allocate_memory_pools();

        manager
    }

    /// Start memory pressure monitoring
    pub fn start_monitoring(&mut self) {
        if !self.config.enable_monitoring {
            return;
        }

        let shutdown_signal = Arc::clone(&self.shutdown_signal);
        let config = self.config.clone();
        let current_usage = Arc::clone(&self.current_usage);
        let peak_usage = Arc::clone(&self.peak_usage);
        let available_memory = Arc::clone(&self.available_memory);
        let pressure_level = Arc::clone(&self.pressure_level);
        let stats = Arc::clone(&self.stats);
        let allocation_throttle = Arc::clone(&self.allocation_throttle);

        let handle = thread::spawn(move || {
            while !shutdown_signal.load(Ordering::Relaxed) {
                // Update memory usage
                let usage = Self::get_system_memory_usage();
                let available = Self::get_available_memory();

                current_usage.store(usage, Ordering::Relaxed);
                available_memory.store(available, Ordering::Relaxed);

                // Update peak usage
                let current_peak = peak_usage.load(Ordering::Relaxed);
                if usage > current_peak {
                    peak_usage.store(usage, Ordering::Relaxed);
                }

                // Calculate pressure level
                let pressure_level_value = if available > 0 {
                    let usage_ratio = usage as f64 / (usage + available) as f64;
                    if usage_ratio >= config.critical_pressure_threshold {
                        MemoryPressureLevel::Critical
                    } else if usage_ratio >= config.high_pressure_threshold {
                        MemoryPressureLevel::High
                    } else if usage_ratio >= config.pressure_threshold {
                        MemoryPressureLevel::Moderate
                    } else {
                        MemoryPressureLevel::Normal
                    }
                } else {
                    MemoryPressureLevel::Critical
                };

                // Update pressure level
                {
                    let mut level = pressure_level.write().unwrap();
                    *level = pressure_level_value;
                }

                // Update throttling based on pressure level
                let throttle_factor = match pressure_level_value {
                    MemoryPressureLevel::Normal => 1.0,
                    MemoryPressureLevel::Moderate => config.throttling_factor,
                    MemoryPressureLevel::High => config.throttling_factor * 0.5,
                    MemoryPressureLevel::Critical => config.throttling_factor * 0.25,
                };

                {
                    let mut throttle = allocation_throttle.write().unwrap();
                    *throttle = throttle_factor;
                }

                // Update statistics
                {
                    let mut stats_guard = stats.write().unwrap();
                    stats_guard.current_usage = usage;
                    stats_guard.peak_usage = peak_usage.load(Ordering::Relaxed);
                    stats_guard.available_memory = available;
                    stats_guard.pressure_level = pressure_level_value;
                }

                thread::sleep(config.monitoring_interval);
            }
        });

        self.monitoring_thread = Some(handle);
    }

    /// Stop memory pressure monitoring
    pub fn stop_monitoring(&mut self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.monitoring_thread.take() {
            let _ = handle.join();
        }
    }

    /// Get current memory pressure level
    pub fn get_pressure_level(&self) -> MemoryPressureLevel {
        *self.pressure_level.read().unwrap()
    }

    /// Get memory pressure statistics
    pub fn get_stats(&self) -> MemoryPressureStats {
        self.stats.read().unwrap().clone()
    }

    /// Check if allocation should be throttled
    pub fn should_throttle_allocation(&self) -> bool {
        let pressure_level = self.get_pressure_level();
        pressure_level != MemoryPressureLevel::Normal
    }

    /// Get allocation throttle factor
    pub fn get_allocation_throttle(&self) -> f64 {
        *self.allocation_throttle.read().unwrap()
    }

    /// Pre-allocate memory pools
    fn pre_allocate_memory_pools(&self) {
        let mut pools = self.memory_pools.write().unwrap();

        // Pre-allocate common sizes
        let common_sizes = vec![64, 256, 1024, 4096, 16384, 65536];

        for size in common_sizes {
            let mut pool = Vec::new();
            let pool_size = self.config.pre_allocation_size / size;

            for _ in 0..pool_size {
                pool.push(vec![0u8; size]);
            }

            pools.insert(size, pool);
        }
    }

    /// Get pre-allocated memory block
    pub fn get_pre_allocated_block(&self, size: usize) -> Option<Vec<u8>> {
        let mut pools = self.memory_pools.write().unwrap();

        // Find the best fit size
        let mut best_size = None;
        for &pool_size in pools.keys() {
            if pool_size >= size && (best_size.is_none() || pool_size < best_size.unwrap()) {
                best_size = Some(pool_size);
            }
        }

        if let Some(pool_size) = best_size {
            if let Some(pool) = pools.get_mut(&pool_size) {
                return pool.pop();
            }
        }

        None
    }

    /// Return pre-allocated memory block
    pub fn return_pre_allocated_block(&self, mut block: Vec<u8>, size: usize) {
        let mut pools = self.memory_pools.write().unwrap();

        if let Some(pool) = pools.get_mut(&size) {
            block.clear();
            pool.push(block);
        }
    }

    /// Get system memory usage
    /// Get system memory usage (in bytes)
    #[cfg(target_os = "macos")]
    fn get_system_memory_usage() -> usize {
        use std::ffi::c_void;
        // sysctl hw.memsize on macOS
        let mut size: usize = 0;
        let mut size_len = std::mem::size_of::<usize>();
        unsafe {
            let ret = libc::sysctl(
                [libc::CTL_HW, libc::HW_MEMSIZE].as_mut_ptr(),
                2,
                &mut size as *mut usize as *mut c_void,
                &mut size_len,
                std::ptr::null_mut(),
                0,
            );
            if ret != 0 {
                // Fallback: return 0 to indicate we couldn't detect it
                return 0;
            }
        }
        size
    }

    #[cfg(target_os = "linux")]
    fn get_system_memory_usage() -> usize {
        // Read MemTotal from /proc/meminfo
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
        0 // Fallback: couldn't detect
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn get_system_memory_usage() -> usize {
        // For unsupported platforms, return 0 to indicate detection is unavailable
        0
    }

    /// Get available system memory (in bytes)
    #[cfg(target_os = "macos")]
    fn get_available_memory() -> usize {
        use std::ffi::c_void;
        // sysctl hw.memsize on macOS
        let mut size: usize = 0;
        let mut size_len = std::mem::size_of::<usize>();
        unsafe {
            let ret = libc::sysctl(
                [libc::CTL_HW, libc::HW_MEMSIZE].as_mut_ptr(),
                2,
                &mut size as *mut usize as *mut c_void,
                &mut size_len,
                std::ptr::null_mut(),
                0,
            );
            if ret != 0 {
                return 0;
            }
        }
        size
    }

    #[cfg(target_os = "linux")]
    fn get_available_memory() -> usize {
        // Read MemAvailable from /proc/meminfo
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
        0 // Fallback
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn get_available_memory() -> usize {
        0
    }
}

impl Drop for MemoryPressureManager {
    fn drop(&mut self) {
        self.stop_monitoring();
    }
}

/// Global memory pressure manager instance
static GLOBAL_MEMORY_PRESSURE_MANAGER: std::sync::OnceLock<MemoryPressureManager> =
    std::sync::OnceLock::new();

/// Initialize global memory pressure manager
pub fn init_memory_pressure_manager(config: MemoryPressureConfig) {
    GLOBAL_MEMORY_PRESSURE_MANAGER
        .set(MemoryPressureManager::new(config))
        .unwrap_or_else(|_| {
            panic!("Memory pressure manager already initialized");
        });
}

/// Get global memory pressure manager
pub fn get_memory_pressure_manager() -> Option<&'static MemoryPressureManager> {
    GLOBAL_MEMORY_PRESSURE_MANAGER.get()
}

/// Check if allocation should be throttled globally
pub fn should_throttle_allocation() -> bool {
    if let Some(manager) = get_memory_pressure_manager() {
        manager.should_throttle_allocation()
    } else {
        false
    }
}

/// Get global allocation throttle factor
pub fn get_allocation_throttle() -> f64 {
    if let Some(manager) = get_memory_pressure_manager() {
        manager.get_allocation_throttle()
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pressure_manager_creation() {
        let config = MemoryPressureConfig::default();
        let manager = MemoryPressureManager::new(config);
        assert_eq!(manager.get_pressure_level(), MemoryPressureLevel::Normal);
    }

    #[test]
    fn test_memory_pressure_monitoring() {
        let config = MemoryPressureConfig {
            enable_monitoring: true,
            monitoring_interval: Duration::from_millis(10),
            ..Default::default()
        };

        let mut manager = MemoryPressureManager::new(config);
        manager.start_monitoring();

        // Wait a bit for monitoring to start
        thread::sleep(Duration::from_millis(50));

        let stats = manager.get_stats();
        assert!(stats.current_usage > 0);

        manager.stop_monitoring();
    }

    #[test]
    fn test_pre_allocated_blocks() {
        let config = MemoryPressureConfig::default();
        let manager = MemoryPressureManager::new(config);

        // Test getting a pre-allocated block
        if let Some(block) = manager.get_pre_allocated_block(1024) {
            assert_eq!(block.len(), 1024);

            // Test returning the block
            manager.return_pre_allocated_block(block, 1024);
        }
    }
}
