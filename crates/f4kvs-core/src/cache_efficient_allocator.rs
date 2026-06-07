//! Cache-efficient memory allocator for F4KVS Core
//!
//! This module provides a memory allocator optimized for cache efficiency
//! and reduced memory fragmentation.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Allocation error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocatorError {
    /// Allocation failed due to insufficient memory
    OutOfMemory,
    /// Invalid layout provided
    InvalidLayout,
    /// Pool is full
    PoolFull,
}

impl std::fmt::Display for AllocatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AllocatorError::OutOfMemory => write!(f, "Out of memory"),
            AllocatorError::InvalidLayout => write!(f, "Invalid layout"),
            AllocatorError::PoolFull => write!(f, "Pool is full"),
        }
    }
}

impl std::error::Error for AllocatorError {}

/// Cache-efficient memory allocator
pub struct CacheEfficientAllocator {
    /// Small object pools (aligned to cache lines)
    small_pools: [AtomicPtr<Pool>; 8],
    /// Medium object pools
    medium_pools: [AtomicPtr<Pool>; 4],
    /// Large object pools
    large_pools: [AtomicPtr<Pool>; 2],
    /// Statistics
    stats: Mutex<AllocatorStats>,
}

/// Memory pool for objects of a specific size
struct Pool {
    /// Size of objects in this pool
    object_size: usize,
    /// Number of objects per block
    objects_per_block: usize,
    /// Free list head
    free_list: AtomicPtr<FreeBlock>,
    /// Total allocated blocks
    total_blocks: AtomicUsize,
    /// Total allocated objects
    total_objects: AtomicUsize,
}

/// Free block in the pool
struct FreeBlock {
    next: AtomicPtr<FreeBlock>,
}

/// Statistics about memory allocator performance
#[derive(Debug, Clone)]
pub struct AllocatorStats {
    /// Total bytes allocated
    pub total_allocated: usize,
    /// Total bytes freed
    pub total_freed: usize,
    /// Current memory usage in bytes
    pub current_usage: usize,
    /// Peak memory usage in bytes
    pub peak_usage: usize,
    /// Number of allocations performed
    pub allocation_count: usize,
    /// Number of deallocations performed
    pub deallocation_count: usize,
    /// Memory fragmentation ratio (0.0 = no fragmentation, 1.0 = maximum fragmentation)
    pub fragmentation_ratio: f64,
}

impl Default for CacheEfficientAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheEfficientAllocator {
    /// Create a new cache-efficient allocator
    pub fn new() -> Self {
        let allocator = Self {
            small_pools: [
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
            ],
            medium_pools: [
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
            ],
            large_pools: [
                AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()),
            ],
            stats: Mutex::new(AllocatorStats {
                total_allocated: 0,
                total_freed: 0,
                current_usage: 0,
                peak_usage: 0,
                allocation_count: 0,
                deallocation_count: 0,
                fragmentation_ratio: 0.0,
            }),
        };

        // Initialize pools
        allocator.initialize_pools();
        allocator
    }

    /// Initialize all memory pools
    fn initialize_pools(&self) {
        // Small object pools (8, 16, 32, 64, 128, 256, 512, 1024 bytes)
        let small_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];
        for (i, &size) in small_sizes.iter().enumerate() {
            let pool = Box::into_raw(Box::new(Pool::new(size, 1024)));
            self.small_pools[i].store(pool, Ordering::Release);
        }

        // Medium object pools (2KB, 4KB, 8KB, 16KB)
        let medium_sizes = [2048, 4096, 8192, 16384];
        for (i, &size) in medium_sizes.iter().enumerate() {
            let pool = Box::into_raw(Box::new(Pool::new(size, 256)));
            self.medium_pools[i].store(pool, Ordering::Release);
        }

        // Large object pools (32KB, 64KB)
        let large_sizes = [32768, 65536];
        for (i, &size) in large_sizes.iter().enumerate() {
            let pool = Box::into_raw(Box::new(Pool::new(size, 64)));
            self.large_pools[i].store(pool, Ordering::Release);
        }
    }

    /// Get the appropriate pool for a given size
    fn get_pool(&self, size: usize) -> Option<&AtomicPtr<Pool>> {
        match size {
            1..=8 => Some(&self.small_pools[0]),
            9..=16 => Some(&self.small_pools[1]),
            17..=32 => Some(&self.small_pools[2]),
            33..=64 => Some(&self.small_pools[3]),
            65..=128 => Some(&self.small_pools[4]),
            129..=256 => Some(&self.small_pools[5]),
            257..=512 => Some(&self.small_pools[6]),
            513..=1024 => Some(&self.small_pools[7]),
            1025..=2048 => Some(&self.medium_pools[0]),
            2049..=4096 => Some(&self.medium_pools[1]),
            4097..=8192 => Some(&self.medium_pools[2]),
            8193..=16384 => Some(&self.medium_pools[3]),
            16385..=32768 => Some(&self.large_pools[0]),
            32769..=65536 => Some(&self.large_pools[1]),
            _ => None,
        }
    }

    /// Allocate memory from the appropriate pool
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, AllocatorError> {
        let size = layout.size();

        // Check if we can use a pool
        if let Some(pool_ptr) = self.get_pool(size) {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let pool = &*pool;
                    if let Some(ptr) = pool.allocate() {
                        self.update_stats(|stats| {
                            stats.total_allocated += size;
                            stats.current_usage += size;
                            stats.allocation_count += 1;
                            if stats.current_usage > stats.peak_usage {
                                stats.peak_usage = stats.current_usage;
                            }
                        });
                        return Ok(ptr);
                    }
                }
            }
        }

        // Fallback to system allocator
        unsafe {
            let ptr = System.alloc(layout);
            if ptr.is_null() {
                Err(AllocatorError::OutOfMemory)
            } else {
                self.update_stats(|stats| {
                    stats.total_allocated += size;
                    stats.current_usage += size;
                    stats.allocation_count += 1;
                    if stats.current_usage > stats.peak_usage {
                        stats.peak_usage = stats.current_usage;
                    }
                });
                Ok(NonNull::new_unchecked(ptr))
            }
        }
    }

    /// Deallocate memory back to the appropriate pool
    pub fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();

        // Check if we can return to a pool
        if let Some(pool_ptr) = self.get_pool(size) {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let pool = &*pool;
                    if pool.deallocate(ptr) {
                        self.update_stats(|stats| {
                            stats.total_freed += size;
                            stats.current_usage = stats.current_usage.saturating_sub(size);
                            stats.deallocation_count += 1;
                        });
                        return;
                    }
                }
            }
        }

        // Fallback to system allocator
        unsafe {
            System.dealloc(ptr.as_ptr(), layout);
            self.update_stats(|stats| {
                stats.total_freed += size;
                stats.current_usage = stats.current_usage.saturating_sub(size);
                stats.deallocation_count += 1;
            });
        }
    }

    /// Get allocator statistics
    pub fn stats(&self) -> AllocatorStats {
        self.stats.lock().unwrap().clone()
    }

    /// Update statistics
    fn update_stats<F>(&self, f: F)
    where
        F: FnOnce(&mut AllocatorStats),
    {
        if let Ok(mut stats) = self.stats.lock() {
            f(&mut stats);
        }
    }
}

impl Pool {
    /// Create a new memory pool
    fn new(object_size: usize, objects_per_block: usize) -> Self {
        Self {
            object_size,
            objects_per_block,
            free_list: AtomicPtr::new(ptr::null_mut()),
            total_blocks: AtomicUsize::new(0),
            total_objects: AtomicUsize::new(0),
        }
    }

    /// Allocate an object from the pool
    unsafe fn allocate(&self) -> Option<NonNull<u8>> {
        // Try to get from free list
        let current = self.free_list.load(Ordering::Acquire);
        if !current.is_null() {
            let next = (*current).next.load(Ordering::Acquire);
            if self
                .free_list
                .compare_exchange_weak(current, next, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.total_objects.fetch_sub(1, Ordering::Relaxed);
                return Some(NonNull::new_unchecked(current as *mut u8));
            }
        }

        // Need to allocate a new block
        self.allocate_block()
    }

    /// Deallocate an object back to the pool
    unsafe fn deallocate(&self, ptr: NonNull<u8>) -> bool {
        // Check if pointer is within our pool
        // This is a simplified check - in reality, you'd need more sophisticated tracking
        let ptr = ptr.as_ptr() as *mut FreeBlock;
        let free_block = FreeBlock {
            next: AtomicPtr::new(ptr::null_mut()),
        };
        ptr::write(ptr, free_block);

        // Add to free list
        let current = self.free_list.load(Ordering::Acquire);
        loop {
            (*ptr).next.store(current, Ordering::Release);
            if self
                .free_list
                .compare_exchange_weak(current, ptr, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.total_objects.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
    }

    /// Allocate a new block for the pool
    unsafe fn allocate_block(&self) -> Option<NonNull<u8>> {
        let block_size = self.object_size * self.objects_per_block;
        let layout = Layout::from_size_align(block_size, 64).ok()?; // 64-byte alignment for cache lines

        let block_ptr = System.alloc(layout);
        if block_ptr.is_null() {
            return None;
        }

        self.total_blocks.fetch_add(1, Ordering::Relaxed);

        // Initialize free list for this block
        for i in 0..self.objects_per_block {
            let obj_ptr = block_ptr.add(i * self.object_size);
            let free_block = FreeBlock {
                next: AtomicPtr::new(ptr::null_mut()),
            };
            ptr::write(obj_ptr as *mut FreeBlock, free_block);

            // Add to free list
            let current = self.free_list.load(Ordering::Acquire);
            loop {
                unsafe {
                    let free_block_ptr = obj_ptr as *mut FreeBlock;
                    (*free_block_ptr).next.store(current, Ordering::Release);
                }
                if self
                    .free_list
                    .compare_exchange_weak(
                        current,
                        obj_ptr as *mut FreeBlock,
                        Ordering::Release,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    break;
                }
            }
        }

        // Return first object
        self.allocate()
    }
}

impl Drop for CacheEfficientAllocator {
    fn drop(&mut self) {
        // Clean up all pools
        for pool_ptr in &self.small_pools {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let _ = Box::from_raw(pool);
                }
            }
        }
        for pool_ptr in &self.medium_pools {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let _ = Box::from_raw(pool);
                }
            }
        }
        for pool_ptr in &self.large_pools {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let _ = Box::from_raw(pool);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_efficient_allocator() {
        let allocator = CacheEfficientAllocator::new();

        // Test small allocation
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = allocator.allocate(layout).unwrap();
        // Note: allocate() returns NonNull which can never be null

        // Test deallocation
        allocator.deallocate(ptr, layout);

        // Check stats
        let stats = allocator.stats();
        assert!(stats.allocation_count > 0);
        assert!(stats.deallocation_count > 0);
    }

    #[test]
    fn test_allocator_pool_selection() {
        let allocator = CacheEfficientAllocator::new();

        // Test different size allocations
        let sizes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

        for &size in &sizes {
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = allocator.allocate(layout).unwrap();
            // Note: allocate() returns NonNull which can never be null
            allocator.deallocate(ptr, layout);
        }
    }
}
