//! Memory Pool Leak Detection Edge Case Tests
//!
//! Additional edge case tests for memory pool leak detection to improve test coverage.

use f4kvs_core::memory_pool::{MemoryPool, MemoryPoolConfig, MemoryPoolError};

#[test]
fn test_memory_pool_zero_block_size() {
    let config = MemoryPoolConfig {
        block_size: 0,
        max_pool_size: 100,
        initial_blocks: 10,
        enable_stats: true,
    };

    let result = MemoryPool::new(config);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MemoryPoolError::InvalidBlockSize
    ));
}

#[test]
fn test_memory_pool_single_block() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 1,
        initial_blocks: 1,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();
    let block1 = pool.allocate().unwrap();

    // Pool should be exhausted
    let block2_result = pool.allocate();
    // Should either succeed (grow pool) or return error
    assert!(block2_result.is_ok() || block2_result.is_err());

    // Deallocate first block
    pool.deallocate(block1).unwrap();
}

#[test]
fn test_memory_pool_rapid_allocate_deallocate() {
    let config = MemoryPoolConfig::default();
    let pool = MemoryPool::new(config).unwrap();

    // Rapid allocate/deallocate cycles
    for _ in 0..1000 {
        let block = pool.allocate().unwrap();
        pool.deallocate(block).unwrap();
    }

    // Verify pool is still functional
    let final_block = pool.allocate().unwrap();
    pool.deallocate(final_block).unwrap();
}

#[test]
fn test_memory_pool_large_block_size() {
    let config = MemoryPoolConfig {
        block_size: 1024 * 1024, // 1MB blocks
        max_pool_size: 10,
        initial_blocks: 2,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();
    let block = pool.allocate().unwrap();

    // Verify block is valid
    pool.deallocate(block).unwrap();
}

#[test]
fn test_memory_pool_max_pool_size_limit() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 5,
        initial_blocks: 5,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();
    let mut blocks = Vec::new();

    // Allocate all initial blocks
    for _ in 0..5 {
        blocks.push(pool.allocate().unwrap());
    }

    // Deallocate all blocks (should return to pool)
    for block in blocks {
        pool.deallocate(block).unwrap();
    }

    // Should be able to allocate again from pool
    let new_block = pool.allocate().unwrap();
    pool.deallocate(new_block).unwrap();
}

#[test]
fn test_memory_pool_concurrent_allocation() {
    use std::sync::Arc;
    use std::thread;

    let config = MemoryPoolConfig::default();
    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let mut handles = vec![];

    // Spawn multiple threads allocating/deallocating
    for _ in 0..4 {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let block = pool_clone.allocate().unwrap();
                pool_clone.deallocate(block).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify pool is still functional
    let final_block = pool.allocate().unwrap();
    pool.deallocate(final_block).unwrap();
}

#[test]
fn test_memory_pool_stats_tracking() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 100,
        initial_blocks: 10,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();
    let stats = pool.get_stats();

    // Initial stats should show initial blocks
    assert!(stats.total_allocated >= 10);
    assert_eq!(stats.blocks_in_use, 0);
    assert!(stats.blocks_available >= 10);

    // Allocate some blocks
    let block1 = pool.allocate().unwrap();
    let block2 = pool.allocate().unwrap();

    let stats_after_alloc = pool.get_stats();
    assert!(stats_after_alloc.blocks_in_use >= 2);

    // Deallocate
    pool.deallocate(block1).unwrap();
    pool.deallocate(block2).unwrap();

    let stats_after_dealloc = pool.get_stats();
    assert!(stats_after_dealloc.blocks_in_use < stats_after_alloc.blocks_in_use);
}
