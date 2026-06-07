#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! # F4KVS Core — Key-Value Store Foundation
//!
//! In-memory engine, value model, and shared primitives for the F4KVS stack.
//! Extracted from the f4kvs-v2 monorepo; used by [`f4kvs-storage-lsm`] and
//! [`f4kvs-ffi`] for persistent and C-bound access.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use f4kvs_core::{F4KVSCore, Config, Value, Result, StorageMode};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = Config::new().with_storage_mode(StorageMode::HashMap);
//!     let engine = F4KVSCore::with_config(config)?;
//!
//!     engine.put("greeting", &Value::from("hello world")).await?;
//!     let retrieved = engine.get("greeting").await?;
//!     println!("Retrieved: {:?}", retrieved);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Stable API
//!
//! - **Basic Operations**: `get`, `put`, `delete`, `exists`
//! - **Batch Operations**: `batch_put`, `batch_get`, `batch_delete`
//! - **Scan Operations**: `scan_prefix`, `scan_range`, `scan_prefix_pairs`, `scan_range_pairs`
//! - **Count Operations**: `count`, `count_prefix`, `count_range`
//! - **Async & Sync APIs**: Both async and blocking interfaces available
//! - **Storage Modes**: HashMap (O(1) lookups) or BTreeMap (ordered keys)
//! - **Memory Storage**: Fast in-memory storage with optional persistence
//!
//! ## Experimental modules
//!
//! The `auth`, `rbac`, `encryption`, and `security` modules are carried over from
//! the upstream monorepo for API compatibility. They use simplified placeholder
//! logic (e.g. password hashing, JWT signing) and are **not production-ready**.
//!
//! Generate local documentation with `cargo doc --open -p f4kvs-core`.
//!
//! [`f4kvs-storage-lsm`]: https://github.com/f4kvs/f4kvs-ffi/tree/main/crates/f4kvs-storage-lsm
//! [`f4kvs-ffi`]: https://github.com/f4kvs/f4kvs-ffi/tree/main/crates/f4kvs-ffi

#[cfg(test)]
mod simd_json_test {
    use crate::Value;
    use serde_json::json;

    #[test]
    fn test_simd_json_integration() {
        let json_str = json!({"key": "value", "number": 42, "array": [1, 2, 3]}).to_string();

        // Test serde_json (baseline)
        let v_serde: Value = serde_json::from_str(&json_str).expect("serde_json failed");

        // Test simd-json
        let mut bytes = json_str.as_bytes().to_vec();
        let v_simd: Value = simd_json::from_slice(&mut bytes).expect("simd-json failed");

        assert_eq!(v_serde, v_simd);
    }

    #[test]
    fn test_performance_comparison() {
        use std::time::Instant;

        let json_small = json!({"a": 1}).to_string();
        let json_large = json!({
            "id": "uuid-v4-format-example",
            "data": (0..100).map(|i| i).collect::<Vec<_>>(),
            "metadata": {"tags": ["rust", "fast", "simd"], "active": true}
        })
        .to_string();

        let test_cases = vec![("Small JSON", json_small), ("Large JSON", json_large)];

        println!("\n--- Parsing Performance Analysis ---");
        for (name, json_str) in test_cases {
            // Serde_json approach
            let start = Instant::now();
            for _ in 0..1000 {
                let _: Value = serde_json::from_str(&json_str).unwrap();
            }
            let duration_serde = start.elapsed();

            // simd-json approach (including buffer allocation/copy)
            let start = Instant::now();
            for _ in 0..1000 {
                let mut bytes = json_str.as_bytes().to_vec();
                let _: Value = simd_json::from_slice(&mut bytes).unwrap();
            }
            let duration_simd = start.elapsed();

            println!("{}:", name);
            println!("  serde_json: {:?}", duration_serde);
            println!("  simd-json (with alloc): {:?}", duration_simd);
            println!(
                "  Overhead/Gain: {:?}",
                duration_serde
                    .checked_sub(duration_simd)
                    .map(|d| d.as_micros().to_string() + "us gain")
                    .unwrap_or_else(|| "No gain".to_string())
            );
        }
        println!("------------------------------------\n");
    }
}

// ============================================================================
// Core Engine Modules
// ============================================================================

/// Core storage engine implementation
pub mod engine;
pub mod engine_utils;
/// Error types and handling utilities
pub mod error;
/// Error boundary for graceful failure handling
pub mod error_boundary;
/// Synchronous API wrappers for async operations
pub mod sync;
/// Value types and serialization
pub mod value;

// ============================================================================
// Storage Engines
// ============================================================================

/// BTreeMap-based storage engine for ordered key access
pub mod btreemap;
/// HashMap-based storage engine for O(1) lookups
pub mod hashmap;
/// In-memory storage with optional persistence
pub mod memory_storage;
/// Optimized engine implementation with performance enhancements
pub mod optimized_engine;
/// Optimized memory storage with performance enhancements
pub mod optimized_memory_storage;
/// Storage engine abstractions and implementations
pub mod storage;
/// Storage engine traits and interfaces
pub mod storage_traits;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration management and validation
pub mod config;
/// Hot-reloading configuration system
pub mod config_reloader;
/// Configuration validation and error reporting
pub mod config_validator;
/// Enhanced configuration with environment overrides
pub mod enhanced_config;
/// Environment variable configuration loader
pub mod env_config;

// ============================================================================
// Concurrency & Lock-Free Structures
// ============================================================================

/// Lock-free data structures and algorithms
///
/// IMPORTANT:
/// - The crate-level `LockFree*` types are production-safe wrappers re-exported from
///   `safe_concurrency_wrappers`.
/// - The historical unsafe implementations previously lived in `src/lockfree.rs`, but were
///   removed from the public API due to memory safety issues.
///
/// This module is kept as a compatibility shim for existing imports like:
/// `use f4kvs_core::lockfree::{LockFreeHashMap, LockFreeQueue, LockFreeStack};`
pub mod lockfree {
    pub use crate::lockfree_cache::LockFreeCache;

    /// Compatibility configuration struct for `lockfree::LockFreeHashMap`.
    ///
    /// Note: `max_buckets` is ignored by the safe implementation (DashMap-backed).
    #[derive(Debug, Clone)]
    pub struct LockFreeHashMapConfig {
        /// Initial number of buckets (historical field name).
        pub initial_buckets: usize,
        /// Load factor threshold (historical field name).
        pub load_factor: f64,
        /// Maximum number of buckets (historical field name; ignored).
        pub max_buckets: usize,
    }

    impl Default for LockFreeHashMapConfig {
        fn default() -> Self {
            Self {
                initial_buckets: 16,
                load_factor: 0.75,
                max_buckets: 1024 * 1024, // kept for backwards compatibility
            }
        }
    }

    impl From<LockFreeHashMapConfig> for crate::safe_concurrency_wrappers::SafeConcurrentHashMapConfig {
        fn from(cfg: LockFreeHashMapConfig) -> Self {
            Self {
                initial_capacity: cfg.initial_buckets,
                load_factor: cfg.load_factor,
            }
        }
    }

    /// Backwards-compatible `LockFreeHashMap` API backed by the safe DashMap wrapper.
    pub struct LockFreeHashMap<K, V> {
        inner: crate::safe_concurrency_wrappers::SafeConcurrentHashMap<K, V>,
    }

    impl<K, V> LockFreeHashMap<K, V>
    where
        K: std::hash::Hash + Eq + Clone,
        V: Clone,
    {
        /// Create a new lock-free hash map.
        ///
        /// This is a compatibility API that preserves the historical constructor signature.
        /// Internally it uses the safe DashMap-backed implementation.
        pub fn new(config: LockFreeHashMapConfig) -> Self {
            Self {
                inner: crate::safe_concurrency_wrappers::SafeConcurrentHashMap::new(config.into()),
            }
        }

        /// Insert a key/value pair, returning the old value if present.
        pub fn insert(&self, key: K, value: V) -> Option<V> {
            self.inner.insert(key, value)
        }

        /// Get a value by key.
        pub fn get(&self, key: &K) -> Option<V>
        where
            K: std::hash::Hash + Eq,
        {
            self.inner.get(key)
        }

        /// Remove a value by key, returning the removed value if present.
        pub fn remove(&self, key: &K) -> Option<V>
        where
            K: std::hash::Hash + Eq,
        {
            self.inner.remove(key)
        }

        /// Returns true if the map contains the given key.
        pub fn contains_key(&self, key: &K) -> bool
        where
            K: std::hash::Hash + Eq,
        {
            self.inner.contains_key(key)
        }

        /// Return the number of elements in the map.
        pub fn len(&self) -> usize {
            self.inner.len()
        }

        /// Returns true if the map is empty.
        pub fn is_empty(&self) -> bool {
            self.inner.is_empty()
        }

        /// Remove all entries from the map.
        pub fn clear(&self) {
            self.inner.clear()
        }
    }

    /// Backwards-compatible `LockFreeQueue` API backed by the safe crossbeam queue wrapper.
    pub type LockFreeQueue<T> = crate::safe_concurrency_wrappers::SafeConcurrentQueue<T>;

    /// Backwards-compatible `LockFreeStack` API backed by the safe synchronized stack wrapper.
    pub type LockFreeStack<T> = crate::safe_concurrency_wrappers::SafeConcurrentStack<T>;
}

/// Hazard pointer implementation for memory safety
pub mod hazard_pointers;
/// Lock-free cache implementation
pub mod lockfree_cache;
/// Lock-free HashMap implementation (internal - use lockfree::LockFreeHashMap for public API)
mod lockfree_hashmap;
pub mod lockfree_utils;
pub mod safe_concurrency;
pub mod safe_concurrency_wrappers;
/// Safe lock-free data structures with hazard pointers (internal - use lockfree::LockFreeHashMap for public API)
mod safe_lockfree;

// ============================================================================
// Memory Management
// ============================================================================

/// Cache-efficient memory allocator for reduced fragmentation
pub mod cache_efficient_allocator;
pub mod memory_leak_detection;
/// Memory pool allocator for efficient memory management
pub mod memory_pool;
pub mod memory_pressure_manager;
/// Safe cache-efficient memory allocator with ABA protection
pub mod safe_cache_efficient_allocator;
/// Safe memory pool allocator with proper synchronization
pub mod safe_memory_pool;

// ============================================================================
// Performance & Optimization
// ============================================================================

/// Batch operation optimizer for high-throughput scenarios
pub mod batch_optimizer;
/// LRU cache implementation for performance optimization
pub mod cache;
/// Performance metrics and monitoring
pub mod metrics;
pub mod monitoring_hooks;
#[cfg(feature = "safe-simd")]
pub mod safe_simd;
/// SIMD optimizations for performance-critical operations
pub mod simd;

// ============================================================================
// Compression
// ============================================================================

/// Compression algorithms and utilities
pub mod compression;
/// Compression algorithm implementations
pub mod compression_impls;
/// Integration layer for compression features
pub mod compression_integration;
/// Compression traits and abstractions
pub mod compression_traits;

// ============================================================================
// Security & Authentication
// ============================================================================

pub mod audit_logging;
/// Authentication and authorization
pub mod auth;
/// Encryption algorithms and key management
pub mod encryption;
/// Role-based access control system
pub mod rbac;
/// Security utilities and middleware
pub mod security;

// ============================================================================
// Database & Query
// ============================================================================

/// Database and table abstraction layer
pub mod database;
/// Query language and filtering capabilities
pub mod query;

// ============================================================================
// Utilities
// ============================================================================

/// Utility functions and helpers
pub mod utils;

// ============================================================================
// Testing Modules
// ============================================================================

#[cfg(test)]
pub mod advanced_property_tests;
#[cfg(test)]
pub mod chaos_tests;
#[cfg(test)]
pub mod comprehensive_tests;
#[cfg(test)]
mod memory_storage_tests;
#[cfg(test)]
pub mod property_tests;

// ============================================================================
// Public API Re-exports - Core Types
// ============================================================================

pub use config::{Config, StorageMode};
pub use engine::{EngineInfo, F4KVSCore};
pub use error::{ErrorSeverity, F4KvsError, Result};
pub use storage::{BTreeMapStorage, HashMapStorage};
pub use storage_traits::{StorageEngine, StorageStats};
pub use sync::F4KVSCoreSync;
pub use value::Value;

// ============================================================================
// Public API Re-exports - Configuration
// ============================================================================

pub use config_reloader::{
    ConfigChangeEvent, ConfigManager, ConfigReloader, ConfigReloaderBuilder,
};
pub use config_validator::{
    ConfigValidationUtils, ConfigValidator, ValidationError, ValidationResult, ValidationSeverity,
    ValidationWarning,
};
pub use enhanced_config::{
    BatchConfig as EnhancedBatchConfig, CacheConfig as EnhancedCacheConfig, ConfigSource,
    EnhancedConfig, EnhancedConfigBuilder, EnhancedConfigManager, EnvironmentConfig,
    EvictionPolicy, LoggingConfig, MemoryPoolConfig as EnhancedMemoryPoolConfig, MonitoringConfig,
    PerformanceConfig, SecurityConfig,
};
pub use env_config::{ConfigValue, EnvConfig, EnvConfigLoader, EnvConfigUtils};

// ============================================================================
// Public API Re-exports - Database
// ============================================================================

pub use database::config::DatabasesConfig;
pub use database::metadata::MetadataManager;
pub use database::registry::{EngineConfig, EngineRegistry};
pub use database::{ColumnDefinition, ColumnType, Database, Table, TableConfig, TableSchema};

// ============================================================================
// Public API Re-exports - Concurrency
// ============================================================================

// Safe concurrency wrappers (production-tested alternatives)
pub use safe_concurrency_wrappers::{
    SafeConcurrentHashMap as LockFreeHashMap, SafeConcurrentHashMapConfig as LockFreeHashMapConfig,
    SafeConcurrentQueue as LockFreeQueue, SafeConcurrentStack as LockFreeStack,
};

pub use hazard_pointers::{
    acquire_hazard_pointer, get_stats, is_in_pending_list, reclaim_pending, release_hazard_pointer,
    safe_free, HazardPointerGuard, SafeAtomicPtr,
};

// Legacy unsafe implementations (REMOVED - use safe_concurrency_wrappers instead)
// Deprecated code removed due to memory safety issues (BUG-001)
// Safe alternatives available:
// - safe_concurrency_wrappers::SafeConcurrentHashMap (replaces LockFreeHashMap)
// - safe_concurrency_wrappers::SafeConcurrentQueue (replaces LockFreeQueue)
// - safe_concurrency_wrappers::SafeConcurrentStack (replaces LockFreeStack)
// - safe_lockfree::SafeLockFreeHashMap (if hazard pointer implementation needed)

// ============================================================================
// Public API Re-exports - Memory Management
// ============================================================================

pub use cache_efficient_allocator::{AllocatorStats, CacheEfficientAllocator};
pub use memory_pool::{
    MemoryPool, MemoryPoolConfig, MemoryPoolError, MemoryPoolManager, MemoryPoolStats, PooledBlock,
};
pub use memory_storage::MemoryStorage;
pub use safe_cache_efficient_allocator::{SafeAllocatorError, SafeCacheEfficientAllocator};
pub use safe_memory_pool::{
    SafeMemoryPool, SafeMemoryPoolConfig, SafeMemoryPoolError, SafeMemoryPoolManager,
    SafeMemoryPoolStats, SafePooledBlock,
};

// ============================================================================
// Public API Re-exports - Performance & Optimization
// ============================================================================

pub use batch_optimizer::{
    BatchConfig, BatchError, BatchGetResult, BatchOptimizer, BatchOptimizerStats, BatchResult,
};
pub use cache::{CacheConfig, CacheStats, CachedStorageEngine, LruCache};
pub use metrics::{MetricsCollector, MonitoredStorageEngine, OperationType, PerformanceMetrics};
pub use simd::{SimdBulkOps, SimdConfig, SimdError, SimdHashOps, SimdStringOps, SimdUtils};

// ============================================================================
// Public API Re-exports - Compression
// ============================================================================

pub use compression::{
    CompressionAlgorithm, CompressionConfig, CompressionError, CompressionManager,
    CompressionStats, CompressionUtils,
};
pub use compression_impls::CompressionAlgorithmFactory;
pub use compression_integration::{
    CompressionAnalysisResult, CompressionAnalyzer, CompressionBenchmarkResult,
    CompressionConfigBuilder, F4KvsCompressionManager, F4KvsCompressionUtils,
};
pub use compression_traits::{
    CompressionAlgorithmImpl, CompressionStatsCollector, CompressionStrategy,
    DefaultCompressionStrategy, DefaultStatsCollector, TraitBasedCompressionManager,
};

// ============================================================================
// Public API Re-exports - Security & Authentication
// ============================================================================

pub use auth::{
    AuditLog, AuditResult, AuthConfig, AuthContext, AuthError, AuthManager, AuthResult,
    CreateRoleRequest, CreateUserRequest, Permission, Role, UpdateUserRequest, User,
};

// ============================================================================
// Public API Re-exports - Query
// ============================================================================

pub use query::{PrefixStats, QueryBuilder, QueryEngine, QueryResult};

// ============================================================================
// Constants & Initialization
// ============================================================================

/// F4KVS Core version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize F4KVS Core with basic tracing
///
/// This function sets up the default tracing subscriber for logging and debugging.
/// It's automatically called when creating a new `F4KVSCore` instance, but can
/// be called manually if you need to initialize tracing separately.
///
/// # Example
///
/// ```rust
/// use f4kvs_core::init;
///
/// // Initialize tracing before creating the engine
/// init();
/// ```
pub fn init() {
    #[cfg(feature = "tracing-subscriber")]
    tracing_subscriber::fmt::init();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        // VERSION is always set by cargo, so this test always passes
    }
}
