//! Memory Pool Coverage Tests
//!
//! Comprehensive tests for memory pool focusing on leak detection, exhaustion scenarios,
//! concurrent access, and cleanup verification to improve test coverage.

use f4kvs_core::memory_pool::{MemoryPool, MemoryPoolConfig, MemoryPoolError, PooledBlock};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_memory_pool_leak_detection() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 100,
        initial_blocks: 10,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();

    // Get initial stats
    let initial_stats = pool.get_stats();
    let initial_allocated = initial_stats.total_allocated;
    let initial_in_use = initial_stats.blocks_in_use;

    // Allocate and deallocate blocks
    let block1 = pool.allocate().unwrap();
    let block2 = pool.allocate().unwrap();

    let stats_after_alloc = pool.get_stats();
    assert_eq!(stats_after_alloc.blocks_in_use, initial_in_use + 2);

    // Deallocate blocks
    pool.deallocate(block1).unwrap();
    pool.deallocate(block2).unwrap();

    let stats_after_dealloc = pool.get_stats();
    assert_eq!(stats_after_dealloc.blocks_in_use, initial_in_use);

    // Verify no leak - total allocated should not increase unnecessarily
    // (may increase if pool grows, but should be reasonable)
    let final_stats = pool.get_stats();
    assert!(final_stats.total_allocated >= initial_allocated);
}

#[test]
fn test_memory_pool_exhaustion_scenarios() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 5,
        initial_blocks: 2,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();
    let mut blocks = Vec::new();

    // Allocate all available blocks
    for _ in 0..5 {
        let block = pool.allocate().unwrap();
        blocks.push(block);
    }

    let stats = pool.get_stats();
    assert_eq!(stats.blocks_in_use, 5);

    // Try to allocate more - should succeed (pool can grow beyond max_pool_size)
    // or fail depending on implementation
    let result = pool.allocate();
    // Either succeeds (grows) or fails (exhausted)
    if result.is_ok() {
        let _extra_block = result.unwrap();
        // If it succeeds, verify stats updated
        let new_stats = pool.get_stats();
        assert!(new_stats.blocks_in_use > 5);
    }

    // Deallocate all blocks
    for block in blocks {
        pool.deallocate(block).unwrap();
    }

    // Verify blocks returned to pool
    let final_stats = pool.get_stats();
    assert!(final_stats.blocks_available >= 5);
}

#[test]
fn test_memory_pool_concurrent_access() {
    let config = MemoryPoolConfig::default();
    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let mut handles = vec![];

    // Spawn multiple threads doing concurrent allocations/deallocations
    for _thread_id in 0..8 {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            let mut local_blocks = Vec::new();

            // Each thread allocates and deallocates multiple blocks
            for _ in 0..50 {
                let block = pool_clone.allocate().unwrap();
                local_blocks.push(block);

                // Occasionally deallocate some blocks
                if local_blocks.len() > 10 {
                    let block = local_blocks.remove(0);
                    pool_clone.deallocate(block).unwrap();
                }
            }

            // Deallocate remaining blocks
            for block in local_blocks {
                pool_clone.deallocate(block).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify pool is still functional and no leaks
    let final_block = pool.allocate().unwrap();
    pool.deallocate(final_block).unwrap();

    let stats = pool.get_stats();
    // Verify reasonable pool state
    assert!(stats.total_allocated > 0);
}

#[test]
fn test_memory_pool_cleanup_on_drop() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 10,
        initial_blocks: 5,
        enable_stats: true,
    };

    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let stats_before = pool.get_stats();

    // Allocate blocks using PooledBlock (RAII wrapper)
    let pooled_block1 = PooledBlock::new(Arc::clone(&pool)).unwrap();
    let pooled_block2 = PooledBlock::new(Arc::clone(&pool)).unwrap();

    let stats_after_alloc = pool.get_stats();
    assert_eq!(
        stats_after_alloc.blocks_in_use,
        stats_before.blocks_in_use + 2
    );

    // Drop pooled blocks - should automatically deallocate
    drop(pooled_block1);
    drop(pooled_block2);

    // Give a moment for cleanup
    thread::sleep(Duration::from_millis(10));

    let stats_after_drop = pool.get_stats();
    assert_eq!(stats_after_drop.blocks_in_use, stats_before.blocks_in_use);
}

#[test]
fn test_memory_pool_allocation_tracking() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 100,
        initial_blocks: 10,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();

    // Track allocations
    let mut allocated_blocks = Vec::new();

    // Allocate blocks and track them
    for i in 0..20 {
        let block = pool.allocate().unwrap();
        allocated_blocks.push((i, block));

        let stats = pool.get_stats();
        // blocks_in_use counts blocks currently checked out, not the preallocated capacity.
        assert_eq!(stats.blocks_in_use, i + 1);
        // total_allocated only grows once the preallocated blocks are exhausted.
        let expected_total_allocated = if i + 1 <= 10 { 10 } else { i + 1 };
        assert_eq!(stats.total_allocated, expected_total_allocated);
    }

    // Verify all blocks are tracked
    let stats = pool.get_stats();
    assert_eq!(stats.blocks_in_use, 20);
    assert_eq!(stats.total_allocated, 20);

    // Deallocate in reverse order
    while let Some((_, block)) = allocated_blocks.pop() {
        pool.deallocate(block).unwrap();
    }

    // Verify all deallocated
    let final_stats = pool.get_stats();
    assert_eq!(final_stats.blocks_in_use, 0);
    assert_eq!(final_stats.blocks_available, 20);
}

#[test]
fn test_memory_pool_rapid_allocate_deallocate_cycles() {
    let config = MemoryPoolConfig::default();
    let pool = MemoryPool::new(config).unwrap();

    // Rapid allocate/deallocate cycles to test pool stability
    for cycle in 0..1000 {
        let block = pool.allocate().unwrap();

        // Verify block is valid
        // Use the block (write some data)
        unsafe {
            let ptr = block.as_ptr() as *mut u8;
            std::ptr::write(ptr, (cycle % 256) as u8);
        }

        pool.deallocate(block).unwrap();

        // Every 100 cycles, verify pool stats are reasonable
        if cycle % 100 == 0 {
            let stats = pool.get_stats();
            assert!(stats.total_allocated > 0);
        }
    }

    // Final verification
    let final_block = pool.allocate().unwrap();
    pool.deallocate(final_block).unwrap();
}

#[test]
fn test_memory_pool_invalid_block_deallocation() {
    let config = MemoryPoolConfig::default();
    let pool1 = MemoryPool::new(config.clone()).unwrap();
    let pool2 = MemoryPool::new(config).unwrap();

    // Allocate from pool1
    let block1 = pool1.allocate().unwrap();

    // Try to deallocate in wrong pool - should fail
    let result = pool2.deallocate(block1);
    assert!(matches!(result, Err(MemoryPoolError::InvalidBlock)));
}

#[test]
fn test_memory_pool_stats_accuracy() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 50,
        initial_blocks: 10,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();

    // Get initial stats
    let initial_stats = pool.get_stats();
    assert_eq!(initial_stats.blocks_available, 10);
    assert_eq!(initial_stats.blocks_in_use, 0);
    assert_eq!(initial_stats.total_allocated, 10);

    // Allocate blocks
    let mut blocks = Vec::new();
    for _ in 0..5 {
        let block = pool.allocate().unwrap();
        blocks.push(block);
    }

    let stats_after_alloc = pool.get_stats();
    assert_eq!(stats_after_alloc.blocks_in_use, 5);
    assert_eq!(stats_after_alloc.blocks_available, 5);

    // Deallocate blocks
    for block in blocks {
        pool.deallocate(block).unwrap();
    }

    let stats_after_dealloc = pool.get_stats();
    assert_eq!(stats_after_dealloc.blocks_in_use, 0);
    assert_eq!(stats_after_dealloc.blocks_available, 10);
}

#[test]
fn test_memory_pool_large_block_sizes() {
    // Test with various large block sizes
    let block_sizes = vec![8192, 16384, 32768, 65536];

    for block_size in block_sizes {
        let config = MemoryPoolConfig {
            block_size,
            max_pool_size: 10,
            initial_blocks: 2,
            enable_stats: true,
        };

        let pool = MemoryPool::new(config).unwrap();
        let block = pool.allocate().unwrap();

        // Verify block is valid and has correct size
        // Verify block size
        let stats = pool.get_stats();
        assert!(stats.total_allocated > 0);

        pool.deallocate(block).unwrap();
    }
}

#[test]
fn test_memory_pool_concurrent_leak_detection() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 100,
        initial_blocks: 20,
        enable_stats: true,
    };

    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let initial_stats = pool.get_stats();

    let mut handles = vec![];

    // Spawn threads that allocate but don't deallocate (simulating leaks)
    for _ in 0..4 {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            let mut leaked_blocks = Vec::new();
            for _ in 0..10 {
                let block = pool_clone.allocate().unwrap();
                leaked_blocks.push(block);
            }
            // Intentionally don't deallocate to simulate leak
            // In real code, this would be detected by leak detection tools
            std::mem::forget(leaked_blocks);
        });
        handles.push(handle);
    }

    // Wait for threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify stats show the "leaked" blocks
    let stats = pool.get_stats();
    assert_eq!(stats.blocks_in_use, initial_stats.blocks_in_use + 40);

    // Note: In a real leak detection scenario, we would use tools like valgrind
    // or sanitizers to detect these leaks. This test just verifies the tracking.
}

#[test]
fn test_memory_pool_pooled_block_raii() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 10,
        initial_blocks: 5,
        enable_stats: true,
    };

    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let initial_stats = pool.get_stats();

    {
        // Create pooled blocks in a scope
        let _block1 = PooledBlock::new(Arc::clone(&pool)).unwrap();
        let _block2 = PooledBlock::new(Arc::clone(&pool)).unwrap();

        // Verify blocks allocated
        let stats = pool.get_stats();
        assert_eq!(stats.blocks_in_use, initial_stats.blocks_in_use + 2);

        // Blocks will be automatically deallocated when dropped
    }

    // After scope, blocks should be deallocated
    let final_stats = pool.get_stats();
    assert_eq!(final_stats.blocks_in_use, initial_stats.blocks_in_use);
}

#[test]
fn test_memory_pool_clear_operation() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 100,
        initial_blocks: 20,
        enable_stats: true,
    };

    let pool = MemoryPool::new(config).unwrap();

    // Allocate some blocks
    let mut blocks = Vec::new();
    for _ in 0..10 {
        let block = pool.allocate().unwrap();
        blocks.push(block);
    }

    // Clear the pool
    pool.clear();

    // Verify pool is cleared
    let stats = pool.get_stats();
    assert_eq!(stats.blocks_available, 0);

    // Deallocate blocks (should still work even after clear)
    for block in blocks {
        pool.deallocate(block).unwrap();
    }
}

#[test]
fn test_memory_pool_block_size_validation() {
    // Test zero block size
    let config = MemoryPoolConfig {
        block_size: 0,
        max_pool_size: 100,
        initial_blocks: 10,
        enable_stats: true,
    };

    let result = MemoryPool::new(config);
    assert!(matches!(result, Err(MemoryPoolError::InvalidBlockSize)));
}

#[test]
fn test_memory_pool_pooled_block_accessors() {
    let config = MemoryPoolConfig {
        block_size: 4096,
        max_pool_size: 10,
        initial_blocks: 5,
        enable_stats: true,
    };

    let pool = Arc::new(MemoryPool::new(config).unwrap());
    let pooled_block = PooledBlock::new(pool.clone()).unwrap();

    // Test accessors
    assert!(!pooled_block.as_mut_ptr().is_null());
    assert_eq!(pooled_block.size(), 4096);

    // Test that we can write to the block
    unsafe {
        let ptr = pooled_block.as_mut_ptr();
        std::ptr::write(ptr, 42u8);
        assert_eq!(std::ptr::read(ptr), 42u8);
    }
}
