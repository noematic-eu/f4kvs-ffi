//! Concrete compression algorithm implementations for F4KVS Core
//!
//! This module provides concrete implementations of compression algorithms
//! using various compression libraries.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::compression::{CompressionAlgorithm, CompressionError};
use crate::compression_traits::CompressionAlgorithmImpl;

/// LZ4 compression implementation
pub struct Lz4Compression;

impl CompressionAlgorithmImpl for Lz4Compression {
    fn compress(&self, _data: &[u8], _level: u8) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "lz4")]
        {
            use lz4::block::compress;

            let compressed = compress(_data, None, false)
                .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
            Ok(compressed)
        }
        #[cfg(not(feature = "lz4"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn decompress(
        &self,
        _data: &[u8],
        _original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "lz4")]
        {
            use lz4::block::decompress;

            let size = _original_size.unwrap_or(_data.len() * 2);

            let decompressed = decompress(_data, Some(size as i32))
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "lz4"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn name(&self) -> &'static str {
        "LZ4"
    }

    fn algorithm_type(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Lz4
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "lz4")]
        {
            true
        }
        #[cfg(not(feature = "lz4"))]
        {
            false
        }
    }

    fn recommended_level(&self, _data: &[u8]) -> u8 {
        3 // LZ4 level 3 is a good balance
    }

    fn estimate_ratio(&self, _data: &[u8]) -> f64 {
        0.6 // Typical LZ4 compression ratio
    }
}

/// Zstd compression implementation
pub struct ZstdCompression;

impl CompressionAlgorithmImpl for ZstdCompression {
    fn compress(&self, _data: &[u8], _level: u8) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "zstd")]
        {
            use zstd::encode_all;

            let compressed = encode_all(_data, _level as i32)
                .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
            Ok(compressed)
        }
        #[cfg(not(feature = "zstd"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn decompress(
        &self,
        _data: &[u8],
        _original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "zstd")]
        {
            use zstd::decode_all;

            let decompressed = decode_all(_data)
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "zstd"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn name(&self) -> &'static str {
        "Zstd"
    }

    fn algorithm_type(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Zstd
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "zstd")]
        {
            true
        }
        #[cfg(not(feature = "zstd"))]
        {
            false
        }
    }

    fn recommended_level(&self, data: &[u8]) -> u8 {
        // Adjust level based on data size
        if data.len() < 1024 {
            1
        } else if data.len() < 10240 {
            3
        } else {
            6
        }
    }

    fn estimate_ratio(&self, _data: &[u8]) -> f64 {
        0.5 // Typical Zstd compression ratio
    }
}

/// Gzip compression implementation
pub struct GzipCompression;

impl CompressionAlgorithmImpl for GzipCompression {
    fn compress(&self, _data: &[u8], _level: u8) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "gzip")]
        {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            use std::io::Write;

            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(_level as u32));
            encoder
                .write_all(_data)
                .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
            encoder
                .finish()
                .map_err(|e| CompressionError::CompressionFailed(e.to_string()))
        }
        #[cfg(not(feature = "gzip"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn decompress(
        &self,
        _data: &[u8],
        _original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "gzip")]
        {
            use flate2::read::GzDecoder;
            use std::io::Read;

            let mut decoder = GzDecoder::new(_data);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "gzip"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn name(&self) -> &'static str {
        "Gzip"
    }

    fn algorithm_type(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Gzip
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "gzip")]
        {
            true
        }
        #[cfg(not(feature = "gzip"))]
        {
            false
        }
    }

    fn recommended_level(&self, data: &[u8]) -> u8 {
        // Adjust level based on data size
        if data.len() < 1024 {
            1
        } else if data.len() < 10240 {
            4
        } else {
            6
        }
    }

    fn estimate_ratio(&self, _data: &[u8]) -> f64 {
        0.4 // Typical Gzip compression ratio
    }
}

/// Snappy compression implementation
pub struct SnappyCompression;

impl CompressionAlgorithmImpl for SnappyCompression {
    fn compress(&self, _data: &[u8], _level: u8) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "snappy")]
        {
            use snap::raw::Encoder;

            let mut encoder = Encoder::new();
            let compressed = encoder
                .compress_vec(_data)
                .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
            Ok(compressed)
        }
        #[cfg(not(feature = "snappy"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn decompress(
        &self,
        _data: &[u8],
        _original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        #[cfg(feature = "snappy")]
        {
            use snap::raw::Decoder;

            let mut decoder = Decoder::new();
            let decompressed = decoder
                .decompress_vec(_data)
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "snappy"))]
        {
            Err(CompressionError::UnsupportedAlgorithm)
        }
    }

    fn name(&self) -> &'static str {
        "Snappy"
    }

    fn algorithm_type(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::Snappy
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "snappy")]
        {
            true
        }
        #[cfg(not(feature = "snappy"))]
        {
            false
        }
    }

    fn recommended_level(&self, _data: &[u8]) -> u8 {
        1 // Snappy only has one level
    }

    fn estimate_ratio(&self, _data: &[u8]) -> f64 {
        0.7 // Typical Snappy compression ratio
    }
}

/// No compression implementation
pub struct NoCompression;

impl CompressionAlgorithmImpl for NoCompression {
    fn compress(&self, _data: &[u8], _level: u8) -> Result<Vec<u8>, CompressionError> {
        Ok(_data.to_vec())
    }

    fn decompress(
        &self,
        _data: &[u8],
        _original_size: Option<usize>,
    ) -> Result<Vec<u8>, CompressionError> {
        Ok(_data.to_vec())
    }

    fn name(&self) -> &'static str {
        "None"
    }

    fn algorithm_type(&self) -> CompressionAlgorithm {
        CompressionAlgorithm::None
    }

    fn is_available(&self) -> bool {
        true
    }

    fn recommended_level(&self, _data: &[u8]) -> u8 {
        0
    }

    fn estimate_ratio(&self, _data: &[u8]) -> f64 {
        1.0
    }
}

/// Factory for creating compression algorithm instances
pub struct CompressionAlgorithmFactory;

impl CompressionAlgorithmFactory {
    /// Create all available compression algorithms
    pub fn create_all(
    ) -> std::collections::HashMap<CompressionAlgorithm, Box<dyn CompressionAlgorithmImpl>> {
        let mut algorithms: std::collections::HashMap<
            CompressionAlgorithm,
            Box<dyn CompressionAlgorithmImpl>,
        > = std::collections::HashMap::new();

        algorithms.insert(
            CompressionAlgorithm::None,
            Box::new(NoCompression) as Box<dyn CompressionAlgorithmImpl>,
        );
        algorithms.insert(
            CompressionAlgorithm::Lz4,
            Box::new(Lz4Compression) as Box<dyn CompressionAlgorithmImpl>,
        );
        algorithms.insert(
            CompressionAlgorithm::Zstd,
            Box::new(ZstdCompression) as Box<dyn CompressionAlgorithmImpl>,
        );
        algorithms.insert(
            CompressionAlgorithm::Gzip,
            Box::new(GzipCompression) as Box<dyn CompressionAlgorithmImpl>,
        );
        algorithms.insert(
            CompressionAlgorithm::Snappy,
            Box::new(SnappyCompression) as Box<dyn CompressionAlgorithmImpl>,
        );

        algorithms
    }

    /// Create a specific compression algorithm
    pub fn create(algorithm: CompressionAlgorithm) -> Box<dyn CompressionAlgorithmImpl> {
        match algorithm {
            CompressionAlgorithm::None => {
                Box::new(NoCompression) as Box<dyn CompressionAlgorithmImpl>
            }
            CompressionAlgorithm::Lz4 => {
                Box::new(Lz4Compression) as Box<dyn CompressionAlgorithmImpl>
            }
            CompressionAlgorithm::Zstd => {
                Box::new(ZstdCompression) as Box<dyn CompressionAlgorithmImpl>
            }
            CompressionAlgorithm::Gzip => {
                Box::new(GzipCompression) as Box<dyn CompressionAlgorithmImpl>
            }
            CompressionAlgorithm::Snappy => {
                Box::new(SnappyCompression) as Box<dyn CompressionAlgorithmImpl>
            }
        }
    }

    /// Get list of available algorithms
    pub fn available_algorithms() -> Vec<CompressionAlgorithm> {
        #[allow(unused_mut)]
        let mut available = vec![CompressionAlgorithm::None];

        #[cfg(feature = "lz4")]
        available.push(CompressionAlgorithm::Lz4);

        #[cfg(feature = "zstd")]
        available.push(CompressionAlgorithm::Zstd);

        #[cfg(feature = "gzip")]
        available.push(CompressionAlgorithm::Gzip);

        #[cfg(feature = "snappy")]
        available.push(CompressionAlgorithm::Snappy);

        available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_algorithm_factory() {
        let algorithms = CompressionAlgorithmFactory::create_all();

        // No compression should always be available
        assert!(algorithms.contains_key(&CompressionAlgorithm::None));

        // Check if other algorithms are available based on features
        #[cfg(feature = "lz4")]
        assert!(algorithms.contains_key(&CompressionAlgorithm::Lz4));

        #[cfg(feature = "zstd")]
        assert!(algorithms.contains_key(&CompressionAlgorithm::Zstd));

        #[cfg(feature = "gzip")]
        assert!(algorithms.contains_key(&CompressionAlgorithm::Gzip));

        #[cfg(feature = "snappy")]
        assert!(algorithms.contains_key(&CompressionAlgorithm::Snappy));
    }

    #[test]
    fn test_no_compression() {
        let no_comp = NoCompression;
        let data = b"test data";

        assert!(no_comp.is_available());
        assert_eq!(no_comp.name(), "None");
        assert_eq!(no_comp.algorithm_type(), CompressionAlgorithm::None);

        let compressed = no_comp.compress(data, 0).unwrap();
        assert_eq!(compressed, data);

        let decompressed = no_comp.decompress(data, None).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_available_algorithms() {
        let available = CompressionAlgorithmFactory::available_algorithms();

        // No compression should always be available
        assert!(available.contains(&CompressionAlgorithm::None));

        // Check other algorithms based on features
        #[cfg(feature = "lz4")]
        assert!(available.contains(&CompressionAlgorithm::Lz4));

        #[cfg(feature = "zstd")]
        assert!(available.contains(&CompressionAlgorithm::Zstd));

        #[cfg(feature = "gzip")]
        assert!(available.contains(&CompressionAlgorithm::Gzip));

        #[cfg(feature = "snappy")]
        assert!(available.contains(&CompressionAlgorithm::Snappy));
    }
}
