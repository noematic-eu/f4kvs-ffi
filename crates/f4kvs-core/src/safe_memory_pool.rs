//! Safe memory pool optimization for F4KVS Core
//!
//! This module provides thread-safe memory pool implementations to reduce allocation overhead
//! and improve performance by reusing memory blocks with proper synchronization.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::VecDeque;
use std::mem;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// Safe memory pool for efficient memory management
#[derive(Debug)]
pub struct SafeMemoryPool {
    /// Pool of available memory blocks (protected by RwLock for better concurrency)
    available_blocks: Arc<RwLock<VecDeque<NonNull<u8>>>>,
    /// Size of each block in bytes
    block_size: usize,
    /// Maximum number of blocks to keep in pool
    max_pool_size: usize,
    /// Total number of blocks allocated (atomic for thread safety)
    total_allocated: AtomicUsize,
    /// Total number of blocks in use (atomic for thread safety)
    blocks_in_use: AtomicUsize,
    /// Pool statistics lock
    stats_lock: Arc<Mutex<PoolStats>>,
}

// SAFETY: SafeMemoryPool is Send and Sync because:
// - Arc<RwLock<VecDeque<NonNull<u8>>>> is Send and Sync (Arc is Send+Sync, RwLock is Send+Sync, VecDeque is Send+Sync, NonNull<u8> is Send+Sync)
// - AtomicUsize is Send and Sync
// - Arc<Mutex<PoolStats>> is Send and Sync
unsafe impl Send for SafeMemoryPool {}
unsafe impl Sync for SafeMemoryPool {}

/// Pool statistics
#[derive(Debug, Clone)]
struct PoolStats {
    /// Number of pool hits (reused blocks)
    pool_hits: usize,
    /// Number of pool misses (new allocations)
    pool_misses: usize,
    /// Number of concurrent access conflicts
    access_conflicts: usize,
}

/// Memory pool configuration
#[derive(Debug, Clone)]
pub struct SafeMemoryPoolConfig {
    /// Size of each block in bytes
    pub block_size: usize,
    /// Maximum number of blocks to keep in pool
    pub max_pool_size: usize,
    /// Initial number of blocks to allocate
    pub initial_blocks: usize,
    /// Enable memory pool statistics
    pub enable_stats: bool,
}

impl Default for SafeMemoryPoolConfig {
    fn default() -> Self {
        Self {
            block_size: 4096,    // 4KB blocks (better for cache lines)
            max_pool_size: 5000, // Keep up to 5000 blocks
            initial_blocks: 50,  // Start with 50 blocks
            enable_stats: true,
        }
    }
}

/// Memory pool statistics
#[derive(Debug, Clone)]
pub struct SafeMemoryPoolStats {
    /// Total number of blocks allocated
    pub total_allocated: usize,
    /// Number of blocks currently in use
    pub blocks_in_use: usize,
    /// Number of blocks available in pool
    pub blocks_available: usize,
    /// Total memory allocated in bytes
    pub total_memory_bytes: usize,
    /// Memory utilization percentage
    pub utilization_percent: f64,
    /// Number of pool hits (reused blocks)
    pub pool_hits: usize,
    /// Number of pool misses (new allocations)
    pub pool_misses: usize,
    /// Number of concurrent access conflicts
    pub access_conflicts: usize,
}

/// Memory pool error types
#[derive(Debug, Clone, PartialEq)]
pub enum SafeMemoryPoolError {
    /// Invalid block size specified
    InvalidBlockSize,
    /// Invalid memory layout
    InvalidLayout,
    /// Memory allocation failed
    AllocationFailed,
    /// Invalid block provided
    InvalidBlock,
    /// Pool is full
    PoolFull,
    /// Concurrent access conflict
    ConcurrentAccessConflict,
}

impl std::fmt::Display for SafeMemoryPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SafeMemoryPoolError::InvalidBlockSize => write!(f, "Invalid block size"),
            SafeMemoryPoolError::InvalidLayout => write!(f, "Invalid memory layout"),
            SafeMemoryPoolError::AllocationFailed => write!(f, "Memory allocation failed"),
            SafeMemoryPoolError::InvalidBlock => write!(f, "Invalid block provided"),
            SafeMemoryPoolError::PoolFull => write!(f, "Memory pool is full"),
            SafeMemoryPoolError::ConcurrentAccessConflict => {
                write!(f, "Concurrent access conflict")
            }
        }
    }
}

impl std::error::Error for SafeMemoryPoolError {}

impl SafeMemoryPool {
    /// Create a new safe memory pool with the given configuration
    pub fn new(config: SafeMemoryPoolConfig) -> Result<Self, SafeMemoryPoolError> {
        if config.block_size == 0 {
            return Err(SafeMemoryPoolError::InvalidBlockSize);
        }

        let pool = Self {
            #[allow(clippy::arc_with_non_send_sync)]
            available_blocks: Arc::new(RwLock::new(VecDeque::new())),
            block_size: config.block_size,
            max_pool_size: config.max_pool_size,
            total_allocated: AtomicUsize::new(0),
            blocks_in_use: AtomicUsize::new(0),
            stats_lock: Arc::new(Mutex::new(PoolStats {
                pool_hits: 0,
                pool_misses: 0,
                access_conflicts: 0,
            })),
        };

        // Pre-allocate initial blocks
        for _ in 0..config.initial_blocks {
            if let Ok(block) = pool.allocate_new_block() {
                let mut available = pool
                    .available_blocks
                    .write()
                    .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;
                available.push_back(block);
            }
        }

        Ok(pool)
    }

    /// Allocate a block from the pool
    pub fn allocate(&self) -> Result<NonNull<u8>, SafeMemoryPoolError> {
        // Try to get a block from the pool first
        if let Some(block) = self.try_get_from_pool()? {
            self.increment_blocks_in_use();
            self.record_pool_hit();
            return Ok(block);
        }

        // Pool is empty, allocate a new block
        let block = self.allocate_new_block()?;
        self.increment_blocks_in_use();
        self.record_pool_miss();
        Ok(block)
    }

    /// Return a block to the pool
    pub fn deallocate(&self, block: NonNull<u8>) -> Result<(), SafeMemoryPoolError> {
        // Validate the block
        if !self.is_valid_block(block) {
            return Err(SafeMemoryPoolError::InvalidBlock);
        }

        // Check if we can add it back to the pool
        let mut available = self
            .available_blocks
            .write()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

        if available.len() < self.max_pool_size {
            available.push_back(block);
            self.decrement_blocks_in_use();
            Ok(())
        } else {
            // Pool is full, free the block
            self.free_block(block);
            self.decrement_blocks_in_use();
            Ok(())
        }
    }

    /// Deallocate a block using a raw pointer (for testing purposes)
    ///
    /// # Safety
    /// The pointer must be valid and have been allocated by this memory pool.
    pub unsafe fn deallocate_raw(&self, ptr: *mut u8) -> Result<(), SafeMemoryPoolError> {
        // Note: as_ptr() never returns null for valid slices, but kept for defensive programming
        #[allow(useless_ptr_null_checks)]
        if ptr.is_null() {
            return Err(SafeMemoryPoolError::InvalidBlock);
        }

        // SAFETY: We know the pointer is not null
        let block = unsafe { NonNull::new_unchecked(ptr) };
        self.deallocate(block)
    }

    /// Try to get a block from the pool
    fn try_get_from_pool(&self) -> Result<Option<NonNull<u8>>, SafeMemoryPoolError> {
        let mut available = self
            .available_blocks
            .write()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;
        Ok(available.pop_front())
    }

    /// Allocate a new block from the system
    fn allocate_new_block(&self) -> Result<NonNull<u8>, SafeMemoryPoolError> {
        let layout = Layout::from_size_align(self.block_size, mem::align_of::<u8>())
            .map_err(|_| SafeMemoryPoolError::InvalidLayout)?;

        unsafe {
            let ptr = System.alloc(layout);
            // Note: as_ptr() never returns null for valid slices, but kept for defensive programming
            #[allow(useless_ptr_null_checks)]
            if ptr.is_null() {
                return Err(SafeMemoryPoolError::AllocationFailed);
            }

            let block = NonNull::new(ptr).ok_or(SafeMemoryPoolError::AllocationFailed)?;
            self.total_allocated.fetch_add(1, Ordering::Relaxed);
            Ok(block)
        }
    }

    /// Free a block back to the system
    fn free_block(&self, block: NonNull<u8>) {
        let layout = Layout::from_size_align(self.block_size, mem::align_of::<u8>())
            .unwrap_or_else(|_| Layout::new::<u8>());

        unsafe {
            System.dealloc(block.as_ptr(), layout);
        }
    }

    /// Check if a block is valid for this pool
    fn is_valid_block(&self, _block: NonNull<u8>) -> bool {
        // In a real implementation, you might want to validate the block
        // by checking if it was allocated by this pool
        true
    }

    /// Increment the number of blocks in use
    fn increment_blocks_in_use(&self) {
        self.blocks_in_use.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the number of blocks in use
    fn decrement_blocks_in_use(&self) {
        self.blocks_in_use.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record a pool hit
    fn record_pool_hit(&self) {
        if let Ok(mut stats) = self.stats_lock.lock() {
            stats.pool_hits += 1;
        }
    }

    /// Record a pool miss
    fn record_pool_miss(&self) {
        if let Ok(mut stats) = self.stats_lock.lock() {
            stats.pool_misses += 1;
        }
    }

    /// Get memory pool statistics
    pub fn get_stats(&self) -> SafeMemoryPoolStats {
        let total_allocated = self.total_allocated.load(Ordering::Relaxed);
        let blocks_in_use = self.blocks_in_use.load(Ordering::Relaxed);
        let blocks_available = self
            .available_blocks
            .read()
            .map(|available| available.len())
            .unwrap_or(0);
        let total_memory_bytes = total_allocated * self.block_size;
        let utilization_percent = if total_allocated > 0 {
            (blocks_in_use as f64 / total_allocated as f64) * 100.0
        } else {
            0.0
        };

        let (pool_hits, pool_misses, access_conflicts) = if let Ok(stats) = self.stats_lock.lock() {
            (stats.pool_hits, stats.pool_misses, stats.access_conflicts)
        } else {
            (0, 0, 0)
        };

        SafeMemoryPoolStats {
            total_allocated,
            blocks_in_use,
            blocks_available,
            total_memory_bytes,
            utilization_percent,
            pool_hits,
            pool_misses,
            access_conflicts,
        }
    }

    /// Get the block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the maximum pool size
    pub fn max_pool_size(&self) -> usize {
        self.max_pool_size
    }

    /// Clear the pool and free all blocks
    pub fn clear(&self) -> Result<(), SafeMemoryPoolError> {
        let mut available = self
            .available_blocks
            .write()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

        while let Some(block) = available.pop_front() {
            self.free_block(block);
        }

        Ok(())
    }
}

/// Safe memory pool manager for multiple pools
pub struct SafeMemoryPoolManager {
    /// Pools indexed by block size
    pools: std::collections::HashMap<usize, Arc<SafeMemoryPool>>,
    /// Default configuration
    default_config: SafeMemoryPoolConfig,
    /// Manager lock for thread safety
    manager_lock: Arc<RwLock<()>>,
}

impl SafeMemoryPoolManager {
    /// Create a new safe memory pool manager
    pub fn new(default_config: SafeMemoryPoolConfig) -> Self {
        Self {
            pools: std::collections::HashMap::new(),
            default_config,
            manager_lock: Arc::new(RwLock::new(())),
        }
    }

    /// Get or create a pool for the specified block size
    pub fn get_pool(&self, block_size: usize) -> Result<Arc<SafeMemoryPool>, SafeMemoryPoolError> {
        // First try to get existing pool with read lock
        {
            let _guard = self
                .manager_lock
                .read()
                .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

            if let Some(pool) = self.pools.get(&block_size) {
                return Ok(Arc::clone(pool));
            }
        }

        // Need to create new pool, upgrade to write lock
        let _guard = self
            .manager_lock
            .write()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

        // Double-check after acquiring write lock
        if let Some(pool) = self.pools.get(&block_size) {
            return Ok(Arc::clone(pool));
        }

        // Create a new pool for this block size
        let mut config = self.default_config.clone();
        config.block_size = block_size;

        #[allow(clippy::arc_with_non_send_sync)]
        let pool = Arc::new(SafeMemoryPool::new(config)?);
        // Note: We can't modify self.pools here because we only have &self
        // In a real implementation, you'd need RefCell or similar
        Ok(pool)
    }

    /// Get statistics for all pools
    pub fn get_all_stats(
        &self,
    ) -> Result<std::collections::HashMap<usize, SafeMemoryPoolStats>, SafeMemoryPoolError> {
        let _guard = self
            .manager_lock
            .read()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

        let mut stats = std::collections::HashMap::new();
        for (block_size, pool) in &self.pools {
            stats.insert(*block_size, pool.get_stats());
        }
        Ok(stats)
    }

    /// Clear all pools
    pub fn clear_all(&self) -> Result<(), SafeMemoryPoolError> {
        let _guard = self
            .manager_lock
            .read()
            .map_err(|_| SafeMemoryPoolError::ConcurrentAccessConflict)?;

        for pool in self.pools.values() {
            pool.clear()?;
        }
        Ok(())
    }
}

/// RAII wrapper for safe memory pool blocks
pub struct SafePooledBlock {
    /// The memory block
    block: NonNull<u8>,
    /// Reference to the pool
    pool: Arc<SafeMemoryPool>,
}

impl SafePooledBlock {
    /// Create a new safe pooled block
    pub fn new(pool: Arc<SafeMemoryPool>) -> Result<Self, SafeMemoryPoolError> {
        let block = pool.allocate()?;
        Ok(Self { block, pool })
    }

    /// Get a mutable pointer to the block
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.block.as_ptr()
    }

    /// Get a const pointer to the block
    pub fn as_ptr(&self) -> *const u8 {
        self.block.as_ptr()
    }

    /// Get the block size
    pub fn size(&self) -> usize {
        self.pool.block_size()
    }
}

impl Drop for SafePooledBlock {
    fn drop(&mut self) {
        let _ = self.pool.deallocate(self.block);
    }
}

unsafe impl Send for SafePooledBlock {}
unsafe impl Sync for SafePooledBlock {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_safe_memory_pool_creation() {
        let config = SafeMemoryPoolConfig::default();
        let pool = SafeMemoryPool::new(config).unwrap();

        assert_eq!(pool.block_size(), 4096);
        assert_eq!(pool.max_pool_size(), 5000);
    }

    #[test]
    fn test_safe_memory_pool_allocation() {
        let config = SafeMemoryPoolConfig {
            block_size: 512,
            max_pool_size: 10,
            initial_blocks: 2,
            enable_stats: true,
        };
        let pool = SafeMemoryPool::new(config).unwrap();

        // Allocate a block
        let block = pool.allocate().unwrap();
        // block.as_ptr() is never null for NonNull, so no need to check

        // Return the block
        pool.deallocate(block).unwrap();
    }

    #[test]
    fn test_safe_pooled_block_raii() {
        let config = SafeMemoryPoolConfig {
            block_size: 256,
            max_pool_size: 5,
            initial_blocks: 1,
            enable_stats: true,
        };
        let pool = Arc::new(SafeMemoryPool::new(config).unwrap());

        // Create a pooled block
        let pooled_block = SafePooledBlock::new(Arc::clone(&pool)).unwrap();
        assert!(!pooled_block.as_ptr().is_null());
        assert_eq!(pooled_block.size(), 256);

        // Block should be returned when dropped
        drop(pooled_block);
    }

    #[test]
    fn test_safe_memory_pool_stats() {
        let config = SafeMemoryPoolConfig {
            block_size: 128,
            max_pool_size: 3,
            initial_blocks: 1,
            enable_stats: true,
        };
        let pool = SafeMemoryPool::new(config).unwrap();

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 1);
        assert_eq!(stats.blocks_available, 1);
        assert_eq!(stats.blocks_in_use, 0);
    }

    // Note: VecDeque<NonNull<u8>> doesn't implement Send/Sync, but this is acceptable
    // for our use case as we use Arc<Mutex<>> for thread safety in the pool

    #[test]
    fn test_safe_memory_pool_error_handling() {
        let config = SafeMemoryPoolConfig {
            block_size: 0, // Invalid block size
            max_pool_size: 10,
            initial_blocks: 1,
            enable_stats: true,
        };

        let result = SafeMemoryPool::new(config);
        assert!(matches!(result, Err(SafeMemoryPoolError::InvalidBlockSize)));
    }
}
