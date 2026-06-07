//! Compression trait definitions for F4KVS Core
//!
//! This module provides trait abstractions for compression algorithms,
//! allowing for easy extension and testing of different compression backends.

use crate::compression::{CompressionAlgorithm, CompressionError, CompressionStats};
use std::time::Instant;

/// Trait for compression algorithm implementations
pub trait CompressionAlgorithmImpl: Send + Sync {
    /// Compress data
    fn compress(&self, data: &[u8], level: u8) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data
    fn decompress(
        &self,
        data: &[u8],
        original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError>;

    /// Get algorithm name
    fn name(&self) -> &'static str;

    /// Get algorithm type
    fn algorithm_type(&self) -> CompressionAlgorithm;

    /// Check if algorithm is available
    fn is_available(&self) -> bool;

    /// Get recommended compression level for data
    fn recommended_level(&self, data: &[u8]) -> u8;

    /// Estimate compression ratio for data
    fn estimate_ratio(&self, data: &[u8]) -> f64;
}

/// Trait for compression statistics collection
pub trait CompressionStatsCollector: Send + Sync {
    /// Record compression operation
    fn record_compression(
        &mut self,
        original_size: usize,
        compressed_size: usize,
        duration: std::time::Duration,
    );

    /// Record decompression operation
    fn record_decompression(
        &mut self,
        compressed_size: usize,
        decompressed_size: usize,
        duration: std::time::Duration,
    );

    /// Get current statistics
    fn get_stats(&self) -> CompressionStats;

    /// Reset statistics
    fn reset_stats(&mut self);
}

/// Trait for compression strategy selection
pub trait CompressionStrategy: Send + Sync {
    /// Select the best compression algorithm for given data
    fn select_algorithm(
        &self,
        data: &[u8],
        available_algorithms: &[CompressionAlgorithm],
    ) -> CompressionAlgorithm;

    /// Determine if data should be compressed
    fn should_compress(&self, data: &[u8], algorithm: CompressionAlgorithm) -> bool;

    /// Get compression level for algorithm and data
    fn get_compression_level(&self, algorithm: CompressionAlgorithm, data: &[u8]) -> u8;
}

/// Default compression strategy implementation
pub struct DefaultCompressionStrategy {
    min_size: usize,
    #[allow(dead_code)]
    max_ratio: f64,
    enable_entropy_check: bool,
}

impl DefaultCompressionStrategy {
    /// Create a new default compression strategy
    pub fn new(min_size: usize, max_ratio: f64, enable_entropy_check: bool) -> Self {
        Self {
            min_size,
            max_ratio,
            enable_entropy_check,
        }
    }
}

impl CompressionStrategy for DefaultCompressionStrategy {
    fn select_algorithm(
        &self,
        data: &[u8],
        available_algorithms: &[CompressionAlgorithm],
    ) -> CompressionAlgorithm {
        if data.len() < self.min_size {
            return CompressionAlgorithm::None;
        }

        // Prefer algorithms in order of efficiency
        let preferred_order = [
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::Gzip,
        ];

        for &algorithm in &preferred_order {
            if available_algorithms.contains(&algorithm) {
                return algorithm;
            }
        }

        CompressionAlgorithm::None
    }

    fn should_compress(&self, data: &[u8], algorithm: CompressionAlgorithm) -> bool {
        if data.len() < self.min_size {
            return false;
        }

        if algorithm == CompressionAlgorithm::None {
            return false;
        }

        if self.enable_entropy_check {
            // Check if data is compressible based on entropy
            let entropy = calculate_entropy(data);
            if entropy > 7.0 {
                return false; // High entropy, likely not compressible
            }
        }

        true
    }

    fn get_compression_level(&self, algorithm: CompressionAlgorithm, _data: &[u8]) -> u8 {
        match algorithm {
            CompressionAlgorithm::None => 0,
            CompressionAlgorithm::Lz4 => 3, // LZ4 level 3 is a good balance
            CompressionAlgorithm::Zstd => 3, // Zstd level 3 is fast and efficient
            CompressionAlgorithm::Gzip => 6, // Gzip level 6 is a good balance
            CompressionAlgorithm::Snappy => 1, // Snappy only has one level
        }
    }
}

/// Calculate entropy of data
fn calculate_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut byte_counts = [0u32; 256];
    for &byte in data {
        byte_counts[byte as usize] += 1;
    }

    let entropy = byte_counts
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / data.len() as f64;
            -p * p.log2()
        })
        .sum::<f64>();

    entropy
}

/// Compression manager with trait-based architecture
pub struct TraitBasedCompressionManager {
    algorithms: std::collections::HashMap<CompressionAlgorithm, Box<dyn CompressionAlgorithmImpl>>,
    strategy: Box<dyn CompressionStrategy>,
    stats_collector: Box<dyn CompressionStatsCollector>,
}

impl TraitBasedCompressionManager {
    /// Create a new trait-based compression manager
    pub fn new(
        algorithms: std::collections::HashMap<
            CompressionAlgorithm,
            Box<dyn CompressionAlgorithmImpl>,
        >,
        strategy: Box<dyn CompressionStrategy>,
        stats_collector: Box<dyn CompressionStatsCollector>,
    ) -> Self {
        Self {
            algorithms,
            strategy,
            stats_collector,
        }
    }

    /// Compress data using the best available algorithm
    pub fn compress(
        &mut self,
        data: &[u8],
    ) -> Result<(Vec<u8>, CompressionAlgorithm), CompressionError> {
        let available_algorithms: Vec<CompressionAlgorithm> = self
            .algorithms
            .keys()
            .filter(|&&alg| self.algorithms[&alg].is_available())
            .copied()
            .collect();

        let algorithm = self.strategy.select_algorithm(data, &available_algorithms);

        if !self.strategy.should_compress(data, algorithm) {
            return Ok((data.to_vec(), CompressionAlgorithm::None));
        }

        let algorithm_impl = self
            .algorithms
            .get(&algorithm)
            .ok_or(CompressionError::UnsupportedAlgorithm)?;

        let level = self.strategy.get_compression_level(algorithm, data);
        let start_time = Instant::now();

        let compressed = algorithm_impl.compress(data, level)?;
        let duration = start_time.elapsed();

        // Record statistics
        self.stats_collector
            .record_compression(data.len(), compressed.len(), duration);

        // Check compression ratio
        let ratio = compressed.len() as f64 / data.len() as f64;
        if ratio > 0.9 {
            // Compression didn't help much, return original data
            return Ok((data.to_vec(), CompressionAlgorithm::None));
        }

        Ok((compressed, algorithm))
    }

    /// Decompress data
    pub fn decompress(
        &mut self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
        original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        if algorithm == CompressionAlgorithm::None {
            return Ok(data.to_vec());
        }

        let algorithm_impl = self
            .algorithms
            .get(&algorithm)
            .ok_or(CompressionError::UnsupportedAlgorithm)?;

        let start_time = Instant::now();
        let decompressed = algorithm_impl.decompress(data, original_size)?;
        let duration = start_time.elapsed();

        // Record statistics
        self.stats_collector
            .record_decompression(data.len(), decompressed.len(), duration);

        Ok(decompressed)
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStats {
        self.stats_collector.get_stats()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats_collector.reset_stats();
    }
}

/// Default statistics collector implementation
pub struct DefaultStatsCollector {
    stats: CompressionStats,
}

impl Default for DefaultStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultStatsCollector {
    /// Create a new default statistics collector
    pub fn new() -> Self {
        Self {
            stats: CompressionStats::default(),
        }
    }
}

impl CompressionStatsCollector for DefaultStatsCollector {
    fn record_compression(
        &mut self,
        original_size: usize,
        compressed_size: usize,
        duration: std::time::Duration,
    ) {
        self.stats.bytes_compressed += original_size as u64;
        self.stats.compression_count += 1;
        self.stats.compression_time_us += duration.as_micros() as u64;

        // Update average ratio
        let ratio = compressed_size as f64 / original_size as f64;
        if self.stats.compression_count > 0 {
            let total_ratio =
                self.stats.avg_ratio * (self.stats.compression_count - 1) as f64 + ratio;
            self.stats.avg_ratio = total_ratio / self.stats.compression_count as f64;
        } else {
            self.stats.avg_ratio = ratio;
        }
    }

    fn record_decompression(
        &mut self,
        _compressed_size: usize,
        decompressed_size: usize,
        duration: std::time::Duration,
    ) {
        self.stats.bytes_decompressed += decompressed_size as u64;
        self.stats.decompression_count += 1;
        self.stats.decompression_time_us += duration.as_micros() as u64;
    }

    fn get_stats(&self) -> CompressionStats {
        self.stats.clone()
    }

    fn reset_stats(&mut self) {
        self.stats = CompressionStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_compression_strategy() {
        let strategy = DefaultCompressionStrategy::new(64, 0.8, true);

        let small_data = b"hello";
        let large_data = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        assert_eq!(
            strategy.select_algorithm(small_data, &[CompressionAlgorithm::Lz4]),
            CompressionAlgorithm::None
        );
        assert_eq!(
            strategy.select_algorithm(large_data, &[CompressionAlgorithm::Lz4]),
            CompressionAlgorithm::Lz4
        );
    }

    #[test]
    fn test_entropy_calculation() {
        let low_entropy = b"aaaaaaaaaaaaaaaa";
        let high_entropy = b"abcdefghijklmnop";

        assert!(calculate_entropy(low_entropy) < calculate_entropy(high_entropy));
    }

    #[test]
    fn test_default_stats_collector() {
        let mut collector = DefaultStatsCollector::new();

        collector.record_compression(100, 50, std::time::Duration::from_millis(1));
        collector.record_decompression(50, 100, std::time::Duration::from_millis(1));

        let stats = collector.get_stats();
        assert_eq!(stats.compression_count, 1);
        assert_eq!(stats.decompression_count, 1);
        assert_eq!(stats.bytes_compressed, 100);
        assert_eq!(stats.bytes_decompressed, 100);
    }
}
