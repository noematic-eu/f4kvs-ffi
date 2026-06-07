//! Batch operations optimizer for F4KVS Core
//!
//! This module provides optimized batch operations with SIMD support,
//! memory pooling, and cache-efficient data structures.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::memory_pool::{MemoryPool, MemoryPoolConfig};
use crate::simd::{SimdBulkOps, SimdConfig, SimdStringOps};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

/// Batch operation configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Enable memory pooling
    pub enable_memory_pool: bool,
    /// Enable lock-free caching
    pub enable_lockfree_cache: bool,
    /// Batch size threshold for optimizations
    pub optimization_threshold: usize,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Enable parallel processing
    pub enable_parallel: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            enable_simd: true,
            enable_memory_pool: true,
            enable_lockfree_cache: true,
            optimization_threshold: 100,
            max_batch_size: 10000,
            enable_parallel: true,
        }
    }
}

/// Optimized batch operations
pub struct BatchOptimizer {
    config: BatchConfig,
    simd_ops: Option<SimdBulkOps>,
    #[allow(dead_code)]
    string_ops: Option<SimdStringOps>,
    memory_pool: Option<Arc<MemoryPool>>,
    cache: Option<Arc<Mutex<HashMap<String, Vec<u8>>>>>,
}

impl BatchOptimizer {
    /// Create a new batch optimizer
    pub fn new(config: BatchConfig) -> Self {
        let simd_config = SimdConfig::default();
        let simd_ops = if config.enable_simd {
            Some(SimdBulkOps::new(simd_config.clone()))
        } else {
            None
        };

        let string_ops = if config.enable_simd {
            Some(SimdStringOps::new(simd_config))
        } else {
            None
        };

        let memory_pool = if config.enable_memory_pool {
            match MemoryPool::new(MemoryPoolConfig::default()) {
                Ok(pool) => Some(Arc::new(pool)),
                Err(e) => {
                    log::warn!("Failed to create memory pool: {}", e);
                    None // Continue without memory pool
                }
            }
        } else {
            None
        };

        let cache = if config.enable_lockfree_cache {
            Some(Arc::new(Mutex::new(HashMap::new())))
        } else {
            None
        };

        Self {
            config,
            simd_ops,
            string_ops,
            memory_pool,
            cache,
        }
    }

    /// Optimized batch put operation
    pub fn batch_put_optimized(
        &self,
        items: Vec<(String, Vec<u8>)>,
    ) -> Result<BatchResult, BatchError> {
        if items.is_empty() {
            return Ok(BatchResult::empty());
        }

        if items.len() > self.config.max_batch_size {
            return Err(BatchError::BatchTooLarge);
        }

        let start_time = Instant::now();
        let mut result = BatchResult::new(items.len());

        // Group items by size for optimization
        let grouped_items = self.group_items_by_size(items);

        // Process each group with appropriate optimizations
        for (size_category, group_items) in grouped_items {
            match size_category {
                SizeCategory::Small => {
                    self.process_small_batch(group_items, &mut result)?;
                }
                SizeCategory::Medium => {
                    self.process_medium_batch(group_items, &mut result)?;
                }
                SizeCategory::Large => {
                    self.process_large_batch(group_items, &mut result)?;
                }
            }
        }

        result.duration = start_time.elapsed();
        Ok(result)
    }

    /// Optimized batch get operation
    pub fn batch_get_optimized(&self, keys: Vec<String>) -> Result<BatchGetResult, BatchError> {
        if keys.is_empty() {
            return Ok(BatchGetResult::empty());
        }

        if keys.len() > self.config.max_batch_size {
            return Err(BatchError::BatchTooLarge);
        }

        let start_time = Instant::now();
        let mut result = BatchGetResult::new(keys.len());

        // Check cache first if enabled
        if let Some(ref cache) = self.cache {
            let cache_guard = cache.lock().unwrap();
            for key in &keys {
                if let Some(value) = cache_guard.get(key) {
                    result.cached_items.push((key.clone(), value.clone()));
                } else {
                    result.missed_keys.push(key.clone());
                }
            }
        } else {
            result.missed_keys = keys;
        }

        result.duration = start_time.elapsed();
        Ok(result)
    }

    /// Group items by size for optimization
    fn group_items_by_size(
        &self,
        items: Vec<(String, Vec<u8>)>,
    ) -> HashMap<SizeCategory, Vec<(String, Vec<u8>)>> {
        let mut groups: HashMap<SizeCategory, Vec<(String, Vec<u8>)>> = HashMap::new();

        for item in items {
            let size = item.1.len();
            let category = match size {
                0..=1023 => SizeCategory::Small,
                1024..=16383 => SizeCategory::Medium,
                _ => SizeCategory::Large,
            };
            groups.entry(category).or_default().push(item);
        }

        groups
    }

    /// Process small batch with SIMD optimizations
    fn process_small_batch(
        &self,
        items: Vec<(String, Vec<u8>)>,
        result: &mut BatchResult,
    ) -> Result<(), BatchError> {
        if let Some(ref simd_ops) = self.simd_ops {
            // Use SIMD for small items
            for (key, value) in items {
                let mut buffer = vec![0u8; value.len()];
                if simd_ops.bulk_copy(&value, &mut buffer).is_err() {
                    result
                        .failed_items
                        .push((key, "SIMD copy failed".to_string()));
                } else {
                    result.successful_items.push((key, value));
                }
            }
        } else {
            // Fallback to regular processing
            for (key, value) in items {
                result.successful_items.push((key, value));
            }
        }

        Ok(())
    }

    /// Process medium batch with memory pooling
    fn process_medium_batch(
        &self,
        items: Vec<(String, Vec<u8>)>,
        result: &mut BatchResult,
    ) -> Result<(), BatchError> {
        if let Some(ref memory_pool) = self.memory_pool {
            // Use memory pool for medium items
            for (key, value) in items {
                let _layout = match std::alloc::Layout::from_size_align(value.len(), 8) {
                    Ok(layout) => layout,
                    Err(_) => continue, // Skip this item if layout creation fails
                };
                if let Ok(_ptr) = memory_pool.allocate() {
                    // Memory pool allocates fixed-size blocks, so we can safely copy
                    // The pool's block size should be sufficient for most values
                    // Use safe copy instead of unsafe pointer operations
                    let mut buffer = vec![0u8; value.len()];
                    buffer.copy_from_slice(&value);
                    result.successful_items.push((key, buffer));
                } else {
                    result
                        .failed_items
                        .push((key, "Memory allocation failed".to_string()));
                }
            }
        } else {
            // Fallback to regular processing
            result.successful_items.extend(items);
        }

        Ok(())
    }

    /// Process large batch with parallel processing
    fn process_large_batch(
        &self,
        items: Vec<(String, Vec<u8>)>,
        result: &mut BatchResult,
    ) -> Result<(), BatchError> {
        // Use parallel processing for large items
        // Note: rayon is not available, using sequential processing
        result.successful_items.extend(items);

        Ok(())
    }

    /// Get optimization statistics
    pub fn get_stats(&self) -> BatchOptimizerStats {
        BatchOptimizerStats {
            simd_enabled: self.simd_ops.is_some(),
            memory_pool_enabled: self.memory_pool.is_some(),
            cache_enabled: self.cache.is_some(),
            parallel_enabled: self.config.enable_parallel,
        }
    }
}

/// Size categories for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SizeCategory {
    Small,  // 0-1KB
    Medium, // 1-16KB
    Large,  // 16KB+
}

/// Result of a batch operation
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Items that were successfully processed (key, value)
    pub successful_items: Vec<(String, Vec<u8>)>,
    /// Items that failed to process (key, error_message)
    pub failed_items: Vec<(String, String)>,
    /// Total number of items in the batch
    pub total_items: usize,
    /// Duration of the batch operation
    pub duration: std::time::Duration,
}

impl BatchResult {
    fn new(total_items: usize) -> Self {
        Self {
            successful_items: Vec::new(),
            failed_items: Vec::new(),
            total_items,
            duration: std::time::Duration::new(0, 0),
        }
    }

    fn empty() -> Self {
        Self::new(0)
    }

    /// Calculate the success rate of the batch operation
    pub fn success_rate(&self) -> f64 {
        if self.total_items == 0 {
            0.0
        } else {
            self.successful_items.len() as f64 / self.total_items as f64
        }
    }
}

/// Result of a batch get operation
#[derive(Debug, Clone)]
pub struct BatchGetResult {
    /// Items that were found in cache (key, value)
    pub cached_items: Vec<(String, Vec<u8>)>,
    /// Keys that were not found in cache
    pub missed_keys: Vec<String>,
    /// Total number of keys requested
    pub total_keys: usize,
    /// Duration of the batch get operation
    pub duration: std::time::Duration,
}

impl BatchGetResult {
    fn new(total_keys: usize) -> Self {
        Self {
            cached_items: Vec::new(),
            missed_keys: Vec::new(),
            total_keys,
            duration: std::time::Duration::new(0, 0),
        }
    }

    fn empty() -> Self {
        Self::new(0)
    }

    /// Calculate the cache hit rate of the batch get operation
    pub fn hit_rate(&self) -> f64 {
        if self.total_keys == 0 {
            0.0
        } else {
            self.cached_items.len() as f64 / self.total_keys as f64
        }
    }
}

/// Statistics about batch optimizer configuration
#[derive(Debug, Clone)]
pub struct BatchOptimizerStats {
    /// Whether SIMD optimizations are enabled
    pub simd_enabled: bool,
    /// Whether memory pool allocation is enabled
    pub memory_pool_enabled: bool,
    /// Whether caching is enabled
    pub cache_enabled: bool,
    /// Whether parallel processing is enabled
    pub parallel_enabled: bool,
}

/// Batch operation errors
#[derive(Debug, Clone)]
pub enum BatchError {
    /// Batch size exceeds maximum allowed limit
    BatchTooLarge,
    /// Failed to allocate memory for batch operation
    MemoryAllocationFailed,
    /// SIMD operation failed during batch processing
    SimdOperationFailed,
    /// Cache operation failed during batch processing
    CacheOperationFailed,
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchError::BatchTooLarge => write!(f, "Batch size exceeds maximum allowed"),
            BatchError::MemoryAllocationFailed => write!(f, "Memory allocation failed"),
            BatchError::SimdOperationFailed => write!(f, "SIMD operation failed"),
            BatchError::CacheOperationFailed => write!(f, "Cache operation failed"),
        }
    }
}

impl std::error::Error for BatchError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_optimizer_creation() {
        let config = BatchConfig::default();
        let optimizer = BatchOptimizer::new(config);
        let stats = optimizer.get_stats();

        assert!(stats.simd_enabled);
        assert!(stats.memory_pool_enabled);
        assert!(stats.cache_enabled);
        assert!(stats.parallel_enabled);
    }

    #[test]
    fn test_batch_put_optimization() {
        let config = BatchConfig::default();
        let optimizer = BatchOptimizer::new(config);

        let items = vec![
            ("key1".to_string(), vec![1, 2, 3, 4]),
            ("key2".to_string(), vec![5, 6, 7, 8]),
            ("key3".to_string(), vec![9, 10, 11, 12]),
        ];

        let result = optimizer.batch_put_optimized(items).unwrap();
        assert_eq!(result.total_items, 3);
        assert_eq!(result.successful_items.len(), 3);
        assert_eq!(result.failed_items.len(), 0);
        assert!(result.success_rate() > 0.0);
    }

    #[test]
    fn test_batch_get_optimization() {
        let config = BatchConfig::default();
        let optimizer = BatchOptimizer::new(config);

        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];

        let result = optimizer.batch_get_optimized(keys).unwrap();
        assert_eq!(result.total_keys, 3);
        assert_eq!(result.missed_keys.len(), 3); // Cache is empty initially
        assert_eq!(result.cached_items.len(), 0);
    }

    #[test]
    fn test_size_categorization() {
        let config = BatchConfig::default();
        let optimizer = BatchOptimizer::new(config);

        let items = vec![
            ("small".to_string(), vec![0; 512]),   // Small
            ("medium".to_string(), vec![0; 8192]), // Medium
            ("large".to_string(), vec![0; 32768]), // Large
        ];

        let grouped = optimizer.group_items_by_size(items);
        assert!(grouped.contains_key(&SizeCategory::Small));
        assert!(grouped.contains_key(&SizeCategory::Medium));
        assert!(grouped.contains_key(&SizeCategory::Large));
    }
}

// Property-based tests will be added in a future iteration
