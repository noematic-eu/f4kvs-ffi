//! Compression integration for F4KVS Core
//!
//! This module provides high-level compression integration with the F4KVS storage system,
//! including automatic compression selection, statistics collection, and performance monitoring.

use crate::compression::{
    CompressionAlgorithm, CompressionConfig, CompressionError, CompressionStats,
};
use crate::compression_impls::CompressionAlgorithmFactory;
use crate::compression_traits::{
    DefaultCompressionStrategy,
    DefaultStatsCollector,
    // CompressionAlgorithmImpl,
    // CompressionStrategy,
    // CompressionStatsCollector,
    TraitBasedCompressionManager,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// High-level compression manager for F4KVS
pub struct F4KvsCompressionManager {
    manager: Arc<Mutex<TraitBasedCompressionManager>>,
    config: CompressionConfig,
}

impl F4KvsCompressionManager {
    /// Create a new F4KVS compression manager
    pub fn new(config: CompressionConfig) -> Self {
        let algorithms = CompressionAlgorithmFactory::create_all();
        let strategy = Box::new(DefaultCompressionStrategy::new(
            config.min_size,
            config.max_ratio,
            true, // Enable entropy check
        ));
        let stats_collector = Box::new(DefaultStatsCollector::new());

        let manager = TraitBasedCompressionManager::new(algorithms, strategy, stats_collector);

        Self {
            manager: Arc::new(Mutex::new(manager)),
            config,
        }
    }

    /// Compress data using the best available algorithm
    pub fn compress(
        &self,
        data: &[u8],
    ) -> Result<(Vec<u8>, CompressionAlgorithm), CompressionError> {
        let mut manager = self.manager.lock().map_err(|_| {
            CompressionError::CompressionFailed("Failed to acquire lock".to_string())
        })?;
        manager.compress(data)
    }

    /// Decompress data
    pub fn decompress(
        &self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
        original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        let mut manager = self.manager.lock().map_err(|_| {
            CompressionError::CompressionFailed("Failed to acquire lock".to_string())
        })?;
        manager.decompress(data, algorithm, original_size)
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStats {
        if let Ok(manager) = self.manager.lock() {
            manager.get_stats()
        } else {
            // Return default stats if lock acquisition fails
            CompressionStats::default()
        }
    }

    /// Reset compression statistics
    pub fn reset_stats(&self) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.reset_stats();
        }
        // Silently ignore lock acquisition failures for reset
    }

    /// Get configuration
    pub fn config(&self) -> &CompressionConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: CompressionConfig) {
        self.config = config;
        // Note: In a real implementation, we'd need to update the strategy as well
    }

    /// Get available compression algorithms
    pub fn available_algorithms(&self) -> Vec<CompressionAlgorithm> {
        CompressionAlgorithmFactory::available_algorithms()
    }

    /// Test compression performance
    pub fn benchmark_compression(
        &self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
    ) -> Result<CompressionBenchmarkResult, CompressionError> {
        let algorithm_impl = CompressionAlgorithmFactory::create(algorithm);

        if !algorithm_impl.is_available() {
            return Err(CompressionError::UnsupportedAlgorithm);
        }

        let start_time = std::time::Instant::now();
        let compressed = algorithm_impl.compress(data, algorithm_impl.recommended_level(data))?;
        let compression_time = start_time.elapsed();

        let start_time = std::time::Instant::now();
        let _decompressed = algorithm_impl.decompress(&compressed, Some(data.len()))?;
        let decompression_time = start_time.elapsed();

        let ratio = compressed.len() as f64 / data.len() as f64;
        let compression_speed = data.len() as f64 / compression_time.as_secs_f64();
        let decompression_speed = compressed.len() as f64 / decompression_time.as_secs_f64();

        Ok(CompressionBenchmarkResult {
            algorithm,
            original_size: data.len(),
            compressed_size: compressed.len(),
            ratio,
            compression_time,
            decompression_time,
            compression_speed,
            decompression_speed,
        })
    }
}

/// Result of a compression benchmark test
#[derive(Debug, Clone)]
pub struct CompressionBenchmarkResult {
    /// Compression algorithm used
    pub algorithm: CompressionAlgorithm,
    /// Original data size in bytes
    pub original_size: usize,
    /// Compressed data size in bytes
    pub compressed_size: usize,
    /// Compression ratio (compressed_size / original_size)
    pub ratio: f64,
    /// Time taken to compress the data
    pub compression_time: std::time::Duration,
    /// Time taken to decompress the data
    pub decompression_time: std::time::Duration,
    /// Compression speed in bytes per second
    pub compression_speed: f64,
    /// Decompression speed in bytes per second
    pub decompression_speed: f64,
}

/// Compression performance analyzer
pub struct CompressionAnalyzer {
    manager: Arc<F4KvsCompressionManager>,
}

impl CompressionAnalyzer {
    /// Create a new compression analyzer
    pub fn new(manager: Arc<F4KvsCompressionManager>) -> Self {
        Self { manager }
    }

    /// Analyze compression performance for different algorithms
    pub fn analyze_performance(
        &self,
        data: &[u8],
    ) -> Result<CompressionAnalysisResult, CompressionError> {
        let available_algorithms = self.manager.available_algorithms();
        let mut results = Vec::new();

        for &algorithm in &available_algorithms {
            if algorithm == CompressionAlgorithm::None {
                continue;
            }

            match self.manager.benchmark_compression(data, algorithm) {
                Ok(result) => results.push(result),
                Err(_) => continue, // Skip unsupported algorithms
            }
        }

        // Find best algorithm based on different criteria
        let best_ratio = results
            .iter()
            .min_by(|a, b| {
                a.ratio
                    .partial_cmp(&b.ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.algorithm);

        let best_speed = results
            .iter()
            .max_by(|a, b| {
                a.compression_speed
                    .partial_cmp(&b.compression_speed)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.algorithm);

        let best_balanced = results
            .iter()
            .min_by(|a, b| {
                // Balance between ratio and speed
                let score_a = a.ratio + (1.0 / (a.compression_speed / 1000.0));
                let score_b = b.ratio + (1.0 / (b.compression_speed / 1000.0));
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.algorithm);

        Ok(CompressionAnalysisResult {
            results,
            best_ratio,
            best_speed,
            best_balanced,
            recommended: best_balanced.unwrap_or(CompressionAlgorithm::None),
        })
    }
}

/// Result of compression analysis across multiple algorithms
#[derive(Debug, Clone)]
pub struct CompressionAnalysisResult {
    /// Benchmark results for all tested algorithms
    pub results: Vec<CompressionBenchmarkResult>,
    /// Algorithm with the best compression ratio
    pub best_ratio: Option<CompressionAlgorithm>,
    /// Algorithm with the best compression speed
    pub best_speed: Option<CompressionAlgorithm>,
    /// Algorithm with the best balance of ratio and speed
    pub best_balanced: Option<CompressionAlgorithm>,
    /// Recommended algorithm based on analysis
    pub recommended: CompressionAlgorithm,
}

/// Compression configuration builder
pub struct CompressionConfigBuilder {
    config: CompressionConfig,
}

impl CompressionConfigBuilder {
    /// Create a new compression configuration builder
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    /// Set compression algorithm
    pub fn algorithm(mut self, algorithm: CompressionAlgorithm) -> Self {
        self.config.algorithm = algorithm;
        self
    }

    /// Set compression level
    pub fn level(mut self, level: u8) -> Self {
        self.config.level = level;
        self
    }

    /// Set minimum size threshold
    pub fn min_size(mut self, min_size: usize) -> Self {
        self.config.min_size = min_size;
        self
    }

    /// Set maximum compression ratio threshold
    pub fn max_ratio(mut self, max_ratio: f64) -> Self {
        self.config.max_ratio = max_ratio;
        self
    }

    /// Enable or disable statistics
    pub fn enable_stats(mut self, enable: bool) -> Self {
        self.config.enable_stats = enable;
        self
    }

    /// Build the configuration
    pub fn build(self) -> CompressionConfig {
        self.config
    }
}

impl Default for CompressionConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression utilities for F4KVS
pub struct F4KvsCompressionUtils;

impl F4KvsCompressionUtils {
    /// Create a compression manager optimized for F4KVS
    pub fn create_optimized_manager() -> F4KvsCompressionManager {
        let config = CompressionConfigBuilder::new()
            .algorithm(CompressionAlgorithm::Lz4)
            .level(3)
            .min_size(128) // Higher threshold for F4KVS
            .max_ratio(0.7) // Stricter ratio for F4KVS
            .enable_stats(true)
            .build();

        F4KvsCompressionManager::new(config)
    }

    /// Create a compression manager for high compression
    pub fn create_high_compression_manager() -> F4KvsCompressionManager {
        let config = CompressionConfigBuilder::new()
            .algorithm(CompressionAlgorithm::Zstd)
            .level(6)
            .min_size(64)
            .max_ratio(0.5)
            .enable_stats(true)
            .build();

        F4KvsCompressionManager::new(config)
    }

    /// Create a compression manager for high speed
    pub fn create_high_speed_manager() -> F4KvsCompressionManager {
        let config = CompressionConfigBuilder::new()
            .algorithm(CompressionAlgorithm::Snappy)
            .level(1)
            .min_size(256)
            .max_ratio(0.8)
            .enable_stats(true)
            .build();

        F4KvsCompressionManager::new(config)
    }

    /// Get compression recommendations for different use cases
    pub fn get_recommendations() -> HashMap<&'static str, CompressionConfig> {
        let mut recommendations = HashMap::new();

        // General purpose
        recommendations.insert(
            "general",
            CompressionConfigBuilder::new()
                .algorithm(CompressionAlgorithm::Lz4)
                .level(3)
                .min_size(128)
                .max_ratio(0.7)
                .enable_stats(true)
                .build(),
        );

        // High compression
        recommendations.insert(
            "high_compression",
            CompressionConfigBuilder::new()
                .algorithm(CompressionAlgorithm::Zstd)
                .level(6)
                .min_size(64)
                .max_ratio(0.5)
                .enable_stats(true)
                .build(),
        );

        // High speed
        recommendations.insert(
            "high_speed",
            CompressionConfigBuilder::new()
                .algorithm(CompressionAlgorithm::Snappy)
                .level(1)
                .min_size(256)
                .max_ratio(0.8)
                .enable_stats(true)
                .build(),
        );

        // Archive storage
        recommendations.insert(
            "archive",
            CompressionConfigBuilder::new()
                .algorithm(CompressionAlgorithm::Gzip)
                .level(9)
                .min_size(32)
                .max_ratio(0.3)
                .enable_stats(true)
                .build(),
        );

        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_config_builder() {
        let config = CompressionConfigBuilder::new()
            .algorithm(CompressionAlgorithm::Zstd)
            .level(6)
            .min_size(128)
            .max_ratio(0.5)
            .enable_stats(true)
            .build();

        assert_eq!(config.algorithm, CompressionAlgorithm::Zstd);
        assert_eq!(config.level, 6);
        assert_eq!(config.min_size, 128);
        assert_eq!(config.max_ratio, 0.5);
        assert!(config.enable_stats);
    }

    #[test]
    fn test_f4kvs_compression_utils() {
        let manager = F4KvsCompressionUtils::create_optimized_manager();
        assert_eq!(manager.config().algorithm, CompressionAlgorithm::Lz4);
        assert_eq!(manager.config().min_size, 128);
    }

    #[test]
    fn test_compression_recommendations() {
        let recommendations = F4KvsCompressionUtils::get_recommendations();

        assert!(recommendations.contains_key("general"));
        assert!(recommendations.contains_key("high_compression"));
        assert!(recommendations.contains_key("high_speed"));
        assert!(recommendations.contains_key("archive"));

        let general = &recommendations["general"];
        assert_eq!(general.algorithm, CompressionAlgorithm::Lz4);

        let high_compression = &recommendations["high_compression"];
        assert_eq!(high_compression.algorithm, CompressionAlgorithm::Zstd);
    }

    #[test]
    fn test_compression_manager_creation() {
        let config = CompressionConfig::default();
        let manager = F4KvsCompressionManager::new(config);

        assert!(manager
            .available_algorithms()
            .contains(&CompressionAlgorithm::None));
    }
}
