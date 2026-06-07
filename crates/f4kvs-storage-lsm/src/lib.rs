//! # F4KVS LSM Tree Storage Engine
//!
//! On-disk LSM (Log-Structured Merge) tree storage for F4KVS with WAL-backed
//! durability, bloom filters, and background compaction.
//!
//! ## Implemented
//!
//! - **Multi-Level LSM Tree**: Configurable levels with size-based tiering
//! - **Automatic Compaction**: Background compaction with configurable strategies
//! - **Bloom Filters**: Fast negative lookups for SSTables
//! - **Write-Ahead Logging**: Crash recovery and durability guarantees
//! - **Block Cache**: Read caching for SSTable blocks
//! - **MemTable Management**: Configurable memtable sizes and flush policies
//!
//! ## Not implemented in this repository
//!
//! - SSTable compression (LZ4, Snappy, Zstd)
//! - Column families
//! - TTL expiry (optional `ttl` feature requires external `f4kvs_ttl` crate)
//!
//! WAL compression can be enabled via `LsmConfig`; on-disk block compression is not available.
//!
//! ## Architecture
//!
//! The LSM tree consists of multiple levels:
//! - **MemTable**: In-memory sorted structure for fast writes
//! - **Immutable MemTable**: Read-only memtable during flush operations
//! - **L0 SSTables**: May overlap, direct from memtable
//! - **L1+ SSTables**: Non-overlapping, sorted files for efficient reads
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use f4kvs_storage_lsm::{LsmTreeEngine, LsmConfig};
//! use f4kvs_storage_core::traits::StorageEngine;
//! use f4kvs_core::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = LsmConfig::default();
//!     let engine = LsmTreeEngine::new(config).await?;
//!
//!     // Basic operations
//!     engine.put("key1", &Value::String("hello world".to_string())).await?;
//!     let value = engine.get("key1").await?;
//!     println!("Retrieved: {:?}", value);
//!
//!     Ok(())
//! }
//! ```

// Re-export core types for convenience
pub use f4kvs_core::{F4KvsError, Result as CoreResult, Value};
pub use f4kvs_storage_core::{
    stats::StorageStats,
    traits::{KeyValueIterator, Storage, StorageEngine, Transaction},
    StorageConfig,
};

// Internal modules
pub mod compaction;
pub mod core;
pub mod error;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod storage;
pub mod utils;

// Re-export main types for convenience
pub use core::{LsmConfig, LsmStorage, LsmTreeEngine};
pub use error::{LsmError, Result as LsmResult};

/// Create an LSM storage engine from a storage configuration
///
/// This factory function creates a new LSM tree engine based on the provided
/// storage configuration.
///
/// # Arguments
///
/// * `config` - Storage configuration containing LSM-specific settings
///
/// # Returns
///
/// * `Ok(Box<dyn StorageEngine + Send + Sync>)` - A boxed storage engine
/// * `Err(F4KvsError)` - If engine creation fails
///
/// # Example
///
/// ```rust,no_run
/// use f4kvs_storage_lsm::create_lsm_engine;
/// use f4kvs_storage_core::StorageConfig;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = StorageConfig::default();
/// let engine = create_lsm_engine(&config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_lsm_engine(
    config: &StorageConfig,
) -> Result<Box<dyn StorageEngine + Send + Sync>, F4KvsError> {
    let lsm_config = LsmConfig::from_storage_config(config);
    let engine = LsmTreeEngine::new(lsm_config)
        .await
        .map_err(|e| F4KvsError::storage(format!("Failed to create LSM engine: {}", e)))?;
    Ok(Box::new(engine))
}

/// Version information for the LSM storage engine
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
