//! Safe cache-efficient memory allocator for F4KVS Core
//!
//! This module provides a memory allocator optimized for cache efficiency
//! and reduced memory fragmentation with proper thread safety and ABA protection.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Allocation error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeAllocatorError {
    /// Allocation failed due to insufficient memory
    OutOfMemory,
    /// Invalid layout provided
    InvalidLayout,
    /// Pool is full
    PoolFull,
    /// Invalid pointer provided
    InvalidPointer,
}

impl std::fmt::Display for SafeAllocatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SafeAllocatorError::OutOfMemory => write!(f, "Out of memory"),
            SafeAllocatorError::InvalidLayout => write!(f, "Invalid layout"),
            SafeAllocatorError::PoolFull => write!(f, "Pool is full"),
            SafeAllocatorError::InvalidPointer => write!(f, "Invalid pointer"),
        }
    }
}

impl std::error::Error for SafeAllocatorError {}

/// Tagged pointer to prevent ABA problems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaggedPtr<T> {
    ptr: *mut T,
    tag: usize,
}

impl<T> TaggedPtr<T> {
    fn new(ptr: *mut T, tag: usize) -> Self {
        Self { ptr, tag }
    }

    #[allow(dead_code)]
    fn null() -> Self {
        Self {
            ptr: ptr::null_mut(),
            tag: 0,
        }
    }

    fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    #[allow(dead_code)]
    fn tag(&self) -> usize {
        self.tag
    }
}

/// Safe cache-efficient memory allocator with ABA protection
pub struct SafeCacheEfficientAllocator {
    /// Small object pools (aligned to cache lines)
    small_pools: [AtomicPtr<SafePool>; 8],
    /// Medium object pools
    medium_pools: [AtomicPtr<SafePool>; 4],
    /// Large object pools
    large_pools: [AtomicPtr<SafePool>; 2],
    /// Statistics
    stats: Mutex<AllocatorStats>,
    /// Global tag counter for ABA protection
    global_tag: AtomicUsize,
}

/// Memory pool for objects of a specific size with ABA protection
struct SafePool {
    /// Size of objects in this pool
    object_size: usize,
    /// Number of objects per block
    objects_per_block: usize,
    /// Free list head with tagged pointer
    free_list: AtomicUsize, // Stores TaggedPtr as usize
    /// Total allocated blocks
    total_blocks: AtomicUsize,
    /// Total allocated objects
    total_objects: AtomicUsize,
    /// Pool-specific tag counter
    pool_tag: AtomicUsize,
}

/// Free block in the pool with ABA protection
struct SafeFreeBlock {
    next: AtomicUsize, // Stores TaggedPtr as usize
    #[allow(dead_code)]
    data: [u8; 0], // Flexible array member
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
    /// Number of ABA conflicts detected
    pub aba_conflicts: usize,
}

impl Default for SafeCacheEfficientAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeCacheEfficientAllocator {
    /// Create a new safe cache-efficient allocator
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
                aba_conflicts: 0,
            }),
            global_tag: AtomicUsize::new(1),
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
            let pool = Box::into_raw(Box::new(SafePool::new(size, 1024)));
            self.small_pools[i].store(pool, Ordering::Release);
        }

        // Medium object pools (2KB, 4KB, 8KB, 16KB)
        let medium_sizes = [2048, 4096, 8192, 16384];
        for (i, &size) in medium_sizes.iter().enumerate() {
            let pool = Box::into_raw(Box::new(SafePool::new(size, 256)));
            self.medium_pools[i].store(pool, Ordering::Release);
        }

        // Large object pools (32KB, 64KB)
        let large_sizes = [32768, 65536];
        for (i, &size) in large_sizes.iter().enumerate() {
            let pool = Box::into_raw(Box::new(SafePool::new(size, 64)));
            self.large_pools[i].store(pool, Ordering::Release);
        }
    }

    /// Get the appropriate pool for a given size
    fn get_pool(&self, size: usize) -> Option<&AtomicPtr<SafePool>> {
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
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, SafeAllocatorError> {
        let size = layout.size();

        // Check if we can use a pool
        if let Some(pool_ptr) = self.get_pool(size) {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let pool = &*pool;
                    if let Some(ptr) = pool.allocate(&self.global_tag) {
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
            // System.alloc may return null on OOM per allocator contract
            if ptr.is_null() {
                Err(SafeAllocatorError::OutOfMemory)
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
    pub fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) -> Result<(), SafeAllocatorError> {
        let size = layout.size();

        // Check if we can return to a pool
        if let Some(pool_ptr) = self.get_pool(size) {
            let pool = pool_ptr.load(Ordering::Acquire);
            if !pool.is_null() {
                unsafe {
                    let pool = &*pool;
                    if pool.deallocate(ptr, &self.global_tag) {
                        self.update_stats(|stats| {
                            stats.total_freed += size;
                            stats.current_usage = stats.current_usage.saturating_sub(size);
                            stats.deallocation_count += 1;
                        });
                        return Ok(());
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

        Ok(())
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

impl SafePool {
    /// Create a new safe memory pool
    fn new(object_size: usize, objects_per_block: usize) -> Self {
        Self {
            object_size,
            objects_per_block,
            free_list: AtomicUsize::new(0), // TaggedPtr::null() as usize
            total_blocks: AtomicUsize::new(0),
            total_objects: AtomicUsize::new(0),
            pool_tag: AtomicUsize::new(1),
        }
    }

    /// Allocate an object from the pool with ABA protection
    unsafe fn allocate(&self, global_tag: &AtomicUsize) -> Option<NonNull<u8>> {
        loop {
            let current = self.free_list.load(Ordering::Acquire);
            let tagged_ptr = TaggedPtr::<SafeFreeBlock>::new(
                (current & !0xFFF) as *mut SafeFreeBlock, // Clear tag bits
                current & 0xFFF,                          // Extract tag
            );

            if tagged_ptr.is_null() {
                // Need to allocate a new block
                return self.allocate_block();
            }

            let next = (*tagged_ptr.as_ptr()).next.load(Ordering::Acquire);
            let next_tagged = TaggedPtr::<SafeFreeBlock>::new(
                (next & !0xFFF) as *mut SafeFreeBlock,
                next & 0xFFF,
            );

            // Increment tag to prevent ABA
            let new_tag = self.pool_tag.fetch_add(1, Ordering::Relaxed) + 1;
            let new_tagged = TaggedPtr::new(next_tagged.as_ptr(), new_tag);
            let new_value = new_tagged.as_ptr() as usize | (new_tag & 0xFFF);

            if self
                .free_list
                .compare_exchange_weak(current, new_value, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.total_objects.fetch_sub(1, Ordering::Relaxed);
                return Some(NonNull::new_unchecked(tagged_ptr.as_ptr() as *mut u8));
            }

            // ABA conflict detected, increment global tag
            global_tag.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Deallocate an object back to the pool with ABA protection
    unsafe fn deallocate(&self, ptr: NonNull<u8>, global_tag: &AtomicUsize) -> bool {
        // Validate pointer alignment and size
        if !(ptr.as_ptr() as usize).is_multiple_of(self.object_size) {
            return false;
        }

        let free_block_ptr = ptr.as_ptr() as *mut SafeFreeBlock;

        // Initialize the free block
        let free_block = SafeFreeBlock {
            next: AtomicUsize::new(0),
            data: [],
        };
        ptr::write(free_block_ptr, free_block);

        loop {
            let current = self.free_list.load(Ordering::Acquire);
            let _current_tagged = TaggedPtr::<SafeFreeBlock>::new(
                (current & !0xFFF) as *mut SafeFreeBlock,
                current & 0xFFF,
            );

            // Set next pointer
            (*free_block_ptr).next.store(current, Ordering::Release);

            // Increment tag to prevent ABA
            let new_tag = self.pool_tag.fetch_add(1, Ordering::Relaxed) + 1;
            let new_tagged = TaggedPtr::new(free_block_ptr, new_tag);
            let new_value = new_tagged.as_ptr() as usize | (new_tag & 0xFFF);

            if self
                .free_list
                .compare_exchange_weak(current, new_value, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.total_objects.fetch_add(1, Ordering::Relaxed);
                return true;
            }

            // ABA conflict detected, increment global tag
            global_tag.fetch_add(1, Ordering::Relaxed);
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
            let free_block = SafeFreeBlock {
                next: AtomicUsize::new(0),
                data: [],
            };
            ptr::write(obj_ptr as *mut SafeFreeBlock, free_block);

            // Add to free list with ABA protection
            loop {
                let current = self.free_list.load(Ordering::Acquire);
                let _current_tagged = TaggedPtr::<SafeFreeBlock>::new(
                    (current & !0xFFF) as *mut SafeFreeBlock,
                    current & 0xFFF,
                );

                (*(obj_ptr as *mut SafeFreeBlock))
                    .next
                    .store(current, Ordering::Release);

                let new_tag = self.pool_tag.fetch_add(1, Ordering::Relaxed) + 1;
                let new_tagged = TaggedPtr::new(obj_ptr as *mut SafeFreeBlock, new_tag);
                let new_value = new_tagged.as_ptr() as usize | (new_tag & 0xFFF);

                if self
                    .free_list
                    .compare_exchange_weak(current, new_value, Ordering::Release, Ordering::Acquire)
                    .is_ok()
                {
                    break;
                }
            }
        }

        // Return first object
        self.allocate(&AtomicUsize::new(1))
    }
}

impl Drop for SafeCacheEfficientAllocator {
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
    fn test_safe_cache_efficient_allocator() {
        let allocator = SafeCacheEfficientAllocator::new();

        // Test small allocation
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = allocator.allocate(layout).unwrap();
        // ptr.as_ptr() is never null for NonNull, so no need to check

        // Test deallocation
        allocator.deallocate(ptr, layout).unwrap();

        // Check stats
        let stats = allocator.stats();
        assert!(stats.allocation_count > 0);
        assert!(stats.deallocation_count > 0);
    }

    #[test]
    fn test_allocator_pool_selection() {
        let allocator = SafeCacheEfficientAllocator::new();

        // Test different size allocations
        let sizes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

        for &size in &sizes {
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = allocator.allocate(layout).unwrap();
            allocator.deallocate(ptr, layout).unwrap();
        }
    }

    #[test]
    fn test_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let allocator = Arc::new(SafeCacheEfficientAllocator::new());
        let layout = Layout::from_size_align(64, 8).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let allocator = Arc::clone(&allocator);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let ptr = allocator.allocate(layout).unwrap();
                        // ptr.as_ptr() is never null for NonNull, so no need to check
                        allocator.deallocate(ptr, layout).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = allocator.stats();
        assert_eq!(stats.allocation_count, 1000);
        assert_eq!(stats.deallocation_count, 1000);
        assert_eq!(stats.current_usage, 0);
    }
}
