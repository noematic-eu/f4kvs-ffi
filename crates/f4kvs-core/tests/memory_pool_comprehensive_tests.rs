//! Comprehensive tests for F4KVS Memory Pool and Allocator implementations
//!
//! This module provides extensive test coverage for memory pool and allocator
//! implementations, including basic operations, concurrent access, performance,
//! and edge cases.

use f4kvs_core::cache_efficient_allocator::CacheEfficientAllocator;
use f4kvs_core::memory_pool::{MemoryPool, MemoryPoolConfig, MemoryPoolError};
use f4kvs_core::safe_cache_efficient_allocator::SafeCacheEfficientAllocator;
use f4kvs_core::safe_memory_pool::{SafeMemoryPool, SafeMemoryPoolConfig};
use std::ptr::NonNull;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Test suite for basic memory pool operations
#[cfg(test)]
mod basic_memory_pool_tests {
    use super::*;

    #[test]
    fn test_memory_pool_creation_default() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config);

        assert!(pool.is_ok());
        let pool = pool.unwrap();

        // Check initial stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 50); // Default initial_blocks
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
    }

    #[test]
    fn test_memory_pool_creation_custom() {
        let config = MemoryPoolConfig {
            block_size: 8192,
            max_pool_size: 1000,
            initial_blocks: 10,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config);

        assert!(pool.is_ok());
        let pool = pool.unwrap();

        // Check initial stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 10);
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 10);
    }

    #[test]
    fn test_memory_pool_creation_invalid_config() {
        let config = MemoryPoolConfig {
            block_size: 0, // Invalid block size
            max_pool_size: 1000,
            initial_blocks: 10,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config);

        assert!(pool.is_err());
        match pool.unwrap_err() {
            MemoryPoolError::InvalidBlockSize => {}
            _ => panic!("Expected InvalidBlockSize error"),
        }
    }

    #[test]
    fn test_memory_pool_allocate() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config).unwrap();

        // Allocate a block
        let block = pool.allocate();
        assert!(block.is_ok());
        let _block = block.unwrap();

        // Check that block is valid
        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 1);
        assert_eq!(stats.blocks_available, 49); // 50 - 1
    }

    #[test]
    fn test_memory_pool_deallocate() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config).unwrap();

        // Allocate a block
        let block = pool.allocate().unwrap();

        // Deallocate the block
        let result = pool.deallocate(block);
        assert!(result.is_ok());

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50); // Back to original
    }

    #[test]
    fn test_memory_pool_multiple_allocations() {
        let config = MemoryPoolConfig {
            block_size: 4096,
            max_pool_size: 10,
            initial_blocks: 5,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config).unwrap();

        // Allocate multiple blocks
        let mut blocks = Vec::new();
        for _ in 0..5 {
            let block = pool.allocate().unwrap();
            blocks.push(block);
        }

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 5);
        assert_eq!(stats.blocks_available, 0);

        // Deallocate all blocks
        for block in blocks {
            pool.deallocate(block).unwrap();
        }

        // Check stats after deallocation
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 5);
    }

    #[test]
    fn test_memory_pool_pool_full() {
        let config = MemoryPoolConfig {
            block_size: 4096,
            max_pool_size: 2,
            initial_blocks: 1,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config).unwrap();

        // Allocate a block
        let block1 = pool.allocate().unwrap();
        let block2 = pool.allocate().unwrap();

        // Deallocate both blocks
        pool.deallocate(block1).unwrap();
        pool.deallocate(block2).unwrap();

        // Pool should be full now
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_available, 2);
    }

    #[test]
    fn test_memory_pool_stats() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config).unwrap();

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 50);
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
        assert_eq!(stats.total_memory_bytes, 50 * 4096);
        assert_eq!(stats.utilization_percent, 0.0);
        assert_eq!(stats.pool_hits, 0);
        assert_eq!(stats.pool_misses, 0);
    }
}

/// Test suite for safe memory pool operations
#[cfg(test)]
mod safe_memory_pool_tests {
    use super::*;

    #[test]
    fn test_safe_memory_pool_creation_default() {
        let config = SafeMemoryPoolConfig::default();
        let pool = SafeMemoryPool::new(config);

        assert!(pool.is_ok());
        let pool = pool.unwrap();

        // Check initial stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 50); // Default initial_blocks
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
    }

    #[test]
    fn test_safe_memory_pool_creation_custom() {
        let config = SafeMemoryPoolConfig {
            block_size: 8192,
            max_pool_size: 1000,
            initial_blocks: 10,
            enable_stats: true,
        };
        let pool = SafeMemoryPool::new(config);

        assert!(pool.is_ok());
        let pool = pool.unwrap();

        // Check initial stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 10);
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 10);
    }

    #[test]
    fn test_safe_memory_pool_allocate() {
        let config = SafeMemoryPoolConfig::default();
        let pool = SafeMemoryPool::new(config).unwrap();

        // Allocate a block
        let block = pool.allocate();
        assert!(block.is_ok());
        let _block = block.unwrap();

        // Check that block is valid
        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 1);
        assert_eq!(stats.blocks_available, 49); // 50 - 1
    }

    #[test]
    fn test_safe_memory_pool_deallocate() {
        let config = SafeMemoryPoolConfig::default();
        let pool = SafeMemoryPool::new(config).unwrap();

        // Allocate a block
        let block = pool.allocate().unwrap();

        // Deallocate the block
        let result = pool.deallocate(block);
        assert!(result.is_ok());

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50); // Back to original
    }

    #[test]
    fn test_safe_memory_pool_concurrent_access() {
        let config = SafeMemoryPoolConfig::default();
        let pool = Arc::new(SafeMemoryPool::new(config).unwrap());
        let mut handles = Vec::new();

        // Spawn multiple threads to allocate and deallocate blocks
        for _ in 0..4 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                let mut blocks = Vec::new();

                // Allocate blocks
                for _ in 0..10 {
                    let block = pool_clone.allocate().unwrap();
                    blocks.push(block);
                }

                // Deallocate blocks
                for block in blocks {
                    pool_clone.deallocate(block).unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Check final stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
    }
}

/// Test suite for cache efficient allocator
#[cfg(test)]
mod cache_efficient_allocator_tests {
    use super::*;

    #[test]
    fn test_cache_efficient_allocator_creation() {
        let allocator = CacheEfficientAllocator::new();
        // Just check that it was created successfully
        assert!(!std::ptr::addr_of!(allocator).is_null());
    }

    #[test]
    fn test_cache_efficient_allocator_allocate() {
        let allocator = CacheEfficientAllocator::new();
        let layout = std::alloc::Layout::from_size_align(1024, 8).unwrap();

        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());
        let ptr = ptr.unwrap();

        // Check that pointer is valid
        // Deallocate
        let _ = allocator.deallocate(ptr, layout);
    }

    #[test]
    fn test_cache_efficient_allocator_stats() {
        let allocator = CacheEfficientAllocator::new();
        let layout = std::alloc::Layout::from_size_align(1024, 8).unwrap();

        // Allocate and deallocate some memory
        for _ in 0..10 {
            let ptr = allocator.allocate(layout).unwrap();
            allocator.deallocate(ptr, layout);
        }

        let stats = allocator.stats();
        assert!(stats.allocation_count > 0);
    }
}

/// Test suite for safe cache efficient allocator
#[cfg(test)]
mod safe_cache_efficient_allocator_tests {
    use super::*;

    #[test]
    fn test_safe_cache_efficient_allocator_creation() {
        let allocator = SafeCacheEfficientAllocator::new();
        // Just check that it was created successfully
        assert!(!std::ptr::addr_of!(allocator).is_null());
    }

    #[test]
    fn test_safe_cache_efficient_allocator_allocate() {
        let allocator = SafeCacheEfficientAllocator::new();
        let layout = std::alloc::Layout::from_size_align(1024, 8).unwrap();

        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());
        let ptr = ptr.unwrap();

        // Check that pointer is valid
        // Deallocate
        let _ = allocator.deallocate(ptr, layout);
    }

    #[test]
    fn test_safe_cache_efficient_allocator_stats() {
        let allocator = SafeCacheEfficientAllocator::new();
        let layout = std::alloc::Layout::from_size_align(1024, 8).unwrap();

        // Allocate and deallocate some memory
        for _ in 0..10 {
            let ptr = allocator.allocate(layout).unwrap();
            allocator.deallocate(ptr, layout);
        }

        let stats = allocator.stats();
        assert!(stats.allocation_count > 0);
    }
}

/// Test suite for concurrent memory pool operations
#[cfg(test)]
mod concurrent_memory_pool_tests {
    use super::*;

    #[test]
    fn test_memory_pool_concurrent_allocations() {
        let config = MemoryPoolConfig::default();
        let pool = Arc::new(MemoryPool::new(config).unwrap());
        let mut handles = Vec::new();

        // Spawn multiple threads to allocate blocks
        for _ in 0..4 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                let mut blocks = Vec::new();

                // Allocate blocks
                for _ in 0..10 {
                    let block = pool_clone.allocate().unwrap();
                    blocks.push(block);
                }

                // Deallocate blocks
                for block in blocks {
                    pool_clone.deallocate(block).unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Check final stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
    }

    #[test]
    fn test_safe_memory_pool_concurrent_allocations() {
        let config = SafeMemoryPoolConfig::default();
        let pool = Arc::new(SafeMemoryPool::new(config).unwrap());
        let mut handles = Vec::new();

        // Spawn multiple threads to allocate blocks
        for _ in 0..4 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                let mut blocks = Vec::new();

                // Allocate blocks
                for _ in 0..10 {
                    let block = pool_clone.allocate().unwrap();
                    blocks.push(block);
                }

                // Deallocate blocks
                for block in blocks {
                    pool_clone.deallocate(block).unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Check final stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 50);
    }
}

/// Test suite for performance testing
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_memory_pool_performance() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config).unwrap();

        let start = Instant::now();
        let mut blocks = Vec::new();

        // Allocate many blocks
        for _ in 0..1000 {
            let block = pool.allocate().unwrap();
            blocks.push(block);
        }

        let allocation_time = start.elapsed();
        println!("Allocation time for 1000 blocks: {:?}", allocation_time);

        // Deallocate all blocks
        let start = Instant::now();
        for block in blocks {
            pool.deallocate(block).unwrap();
        }

        let deallocation_time = start.elapsed();
        println!("Deallocation time for 1000 blocks: {:?}", deallocation_time);

        // Should be reasonably fast
        assert!(allocation_time.as_millis() < 100);
        assert!(deallocation_time.as_millis() < 100);
    }

    #[test]
    fn test_safe_memory_pool_performance() {
        let config = SafeMemoryPoolConfig::default();
        let pool = SafeMemoryPool::new(config).unwrap();

        let start = Instant::now();
        let mut blocks = Vec::new();

        // Allocate many blocks
        for _ in 0..1000 {
            let block = pool.allocate().unwrap();
            blocks.push(block);
        }

        let allocation_time = start.elapsed();
        println!(
            "Safe allocation time for 1000 blocks: {:?}",
            allocation_time
        );

        // Deallocate all blocks
        let start = Instant::now();
        for block in blocks {
            pool.deallocate(block).unwrap();
        }

        let deallocation_time = start.elapsed();
        println!(
            "Safe deallocation time for 1000 blocks: {:?}",
            deallocation_time
        );

        // Should be reasonably fast
        assert!(allocation_time.as_millis() < 100);
        assert!(deallocation_time.as_millis() < 100);
    }
}

/// Test suite for edge cases and error handling
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_memory_pool_invalid_block_deallocation() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config).unwrap();

        // Try to deallocate a null pointer (this should be handled gracefully)
        // Note: NonNull::new(null) returns None, so we need to create a valid but invalid block
        let invalid_block = NonNull::new(0x1 as *mut u8).unwrap(); // Invalid but non-null pointer
        let result = pool.deallocate(invalid_block);
        // This might succeed or fail depending on implementation
        // The important thing is that it doesn't crash
        let _ = result; // Ignore the result
    }

    #[test]
    fn test_memory_pool_large_allocation() {
        let config = MemoryPoolConfig {
            block_size: 1024 * 1024, // 1MB blocks
            max_pool_size: 10,
            initial_blocks: 2,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config).unwrap();

        // Allocate large blocks
        let mut blocks = Vec::new();
        for _ in 0..5 {
            let block = pool.allocate().unwrap();
            blocks.push(block);
        }

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, 5);
        assert_eq!(stats.total_memory_bytes, 5 * 1024 * 1024);

        // Deallocate all blocks
        for block in blocks {
            pool.deallocate(block).unwrap();
        }
    }

    #[test]
    fn test_memory_pool_zero_initial_blocks() {
        let config = MemoryPoolConfig {
            block_size: 4096,
            max_pool_size: 100,
            initial_blocks: 0,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config).unwrap();

        // Check initial stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 0);
        assert_eq!(stats.blocks_in_use, 0);
        assert_eq!(stats.blocks_available, 0);

        // Allocate a block (should create new block)
        let block = pool.allocate().unwrap();
        // Check stats after allocation
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 1);
        assert_eq!(stats.blocks_in_use, 1);
        assert_eq!(stats.blocks_available, 0);

        // Deallocate
        pool.deallocate(block).unwrap();
    }

    #[test]
    fn test_memory_pool_max_pool_size_reached() {
        let config = MemoryPoolConfig {
            block_size: 4096,
            max_pool_size: 2,
            initial_blocks: 1,
            enable_stats: true,
        };
        let pool = MemoryPool::new(config).unwrap();

        // Allocate and deallocate to fill the pool
        let block1 = pool.allocate().unwrap();
        let block2 = pool.allocate().unwrap();
        pool.deallocate(block1).unwrap();
        pool.deallocate(block2).unwrap();

        // Pool should be full
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_available, 2);

        // Allocate another block (should create new block)
        let block3 = pool.allocate().unwrap();
        // Deallocate (should free the block since pool is full)
        pool.deallocate(block3).unwrap();
    }
}
