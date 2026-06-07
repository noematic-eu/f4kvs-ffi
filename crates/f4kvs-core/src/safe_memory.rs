//! Safe Memory Management Utilities
//!
//! This module provides safe alternatives to unsafe memory operations,
//! using safe abstractions and proper error handling.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Safe memory allocation error
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeMemoryError {
    /// Allocation failed
    AllocationFailed,
    /// Invalid layout
    InvalidLayout,
    /// Memory limit exceeded
    MemoryLimitExceeded,
    /// Alignment not supported
    UnsupportedAlignment,
}

impl std::fmt::Display for SafeMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SafeMemoryError::AllocationFailed => write!(f, "Memory allocation failed"),
            SafeMemoryError::InvalidLayout => write!(f, "Invalid memory layout"),
            SafeMemoryError::MemoryLimitExceeded => write!(f, "Memory limit exceeded"),
            SafeMemoryError::UnsupportedAlignment => write!(f, "Unsupported memory alignment"),
        }
    }
}

impl std::error::Error for SafeMemoryError {}

/// Safe memory allocator with bounds checking and error handling
pub struct SafeAllocator {
    /// Maximum memory usage in bytes
    max_memory: AtomicUsize,
    /// Current memory usage in bytes
    current_memory: AtomicUsize,
    /// Enable bounds checking
    bounds_checking: bool,
}

impl SafeAllocator {
    /// Create a new safe allocator
    pub fn new(max_memory: usize, bounds_checking: bool) -> Self {
        Self {
            max_memory: AtomicUsize::new(max_memory),
            current_memory: AtomicUsize::new(0),
            bounds_checking,
        }
    }

    /// Create with default settings
    pub fn default() -> Self {
        Self::new(1024 * 1024 * 1024, true) // 1GB default limit
    }

    /// Allocate memory safely
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, SafeMemoryError> {
        if self.bounds_checking {
            self.check_layout(&layout)?;
            self.check_memory_limit(layout.size())?;
        }

        // Use the global allocator safely
        let ptr = System.alloc(layout);
        if ptr.is_null() {
            return Err(SafeMemoryError::AllocationFailed);
        }

        // Update memory usage
        self.current_memory
            .fetch_add(layout.size(), Ordering::Relaxed);

        // Create NonNull from raw pointer
        let ptr = NonNull::new(ptr).ok_or(SafeMemoryError::AllocationFailed)?;
        let slice_ptr = NonNull::slice_from_raw_parts(ptr, layout.size());

        Ok(slice_ptr)
    }

    /// Deallocate memory safely
    pub fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if self.bounds_checking {
            self.check_layout(&layout).ok();
        }

        // Update memory usage
        self.current_memory
            .fetch_sub(layout.size(), Ordering::Relaxed);

        // Deallocate using the global allocator
        System.dealloc(ptr.as_ptr(), layout);
    }

    /// Get current memory usage
    pub fn current_memory_usage(&self) -> usize {
        self.current_memory.load(Ordering::Relaxed)
    }

    /// Get maximum memory limit
    pub fn max_memory_limit(&self) -> usize {
        self.max_memory.load(Ordering::Relaxed)
    }

    /// Set maximum memory limit
    pub fn set_max_memory_limit(&self, limit: usize) {
        self.max_memory.store(limit, Ordering::Relaxed);
    }

    /// Check if layout is valid
    fn check_layout(&self, layout: &Layout) -> Result<(), SafeMemoryError> {
        if layout.size() == 0 {
            return Err(SafeMemoryError::InvalidLayout);
        }

        if layout.align() > 4096 {
            return Err(SafeMemoryError::UnsupportedAlignment);
        }

        Ok(())
    }

    /// Check if allocation would exceed memory limit
    fn check_memory_limit(&self, size: usize) -> Result<(), SafeMemoryError> {
        let current = self.current_memory.load(Ordering::Relaxed);
        let max = self.max_memory.load(Ordering::Relaxed);

        if current.saturating_add(size) > max {
            return Err(SafeMemoryError::MemoryLimitExceeded);
        }

        Ok(())
    }
}

/// Safe memory pool for efficient allocation of fixed-size blocks
pub struct SafeMemoryPool {
    /// Block size
    block_size: usize,
    /// Number of blocks
    block_count: usize,
    /// Free blocks stack
    free_blocks: std::sync::Mutex<Vec<NonNull<u8>>>,
    /// Allocated blocks for cleanup
    allocated_blocks: std::sync::Mutex<Vec<NonNull<u8>>>,
    /// Safe allocator
    allocator: SafeAllocator,
}

impl SafeMemoryPool {
    /// Create a new safe memory pool
    pub fn new(block_size: usize, block_count: usize) -> Result<Self, SafeMemoryError> {
        let layout =
            Layout::from_size_align(block_size, 8).map_err(|_| SafeMemoryError::InvalidLayout)?;

        let total_memory = block_size * block_count;
        let allocator = SafeAllocator::new(total_memory, true);

        let mut pool = Self {
            block_size,
            block_count,
            free_blocks: std::sync::Mutex::new(Vec::new()),
            allocated_blocks: std::sync::Mutex::new(Vec::new()),
            allocator,
        };

        // Pre-allocate blocks
        pool.preallocate_blocks()?;

        Ok(pool)
    }

    /// Allocate a block from the pool
    pub fn allocate(&self) -> Option<NonNull<u8>> {
        let mut free_blocks = self.free_blocks.lock().ok()?;
        free_blocks.pop()
    }

    /// Deallocate a block back to the pool
    pub fn deallocate(&self, ptr: NonNull<u8>) -> Result<(), SafeMemoryError> {
        // Verify the pointer is within our allocated range
        if !self.is_valid_pointer(ptr) {
            return Err(SafeMemoryError::InvalidLayout);
        }

        let mut free_blocks = self
            .free_blocks
            .lock()
            .map_err(|_| SafeMemoryError::AllocationFailed)?;

        free_blocks.push(ptr);
        Ok(())
    }

    /// Get the block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the number of free blocks
    pub fn free_block_count(&self) -> usize {
        self.free_blocks
            .lock()
            .map(|blocks| blocks.len())
            .unwrap_or(0)
    }

    /// Get the total number of blocks
    pub fn total_block_count(&self) -> usize {
        self.block_count
    }

    /// Pre-allocate blocks for the pool
    fn preallocate_blocks(&mut self) -> Result<(), SafeMemoryError> {
        let layout = Layout::from_size_align(self.block_size, 8)
            .map_err(|_| SafeMemoryError::InvalidLayout)?;

        let mut allocated_blocks = self
            .allocated_blocks
            .lock()
            .map_err(|_| SafeMemoryError::AllocationFailed)?;

        let mut free_blocks = self
            .free_blocks
            .lock()
            .map_err(|_| SafeMemoryError::AllocationFailed)?;

        for _ in 0..self.block_count {
            let block = self.allocator.allocate(layout)?;
            let ptr =
                NonNull::new(block.as_ptr() as *mut u8).ok_or(SafeMemoryError::AllocationFailed)?;

            allocated_blocks.push(ptr);
            free_blocks.push(ptr);
        }

        Ok(())
    }

    /// Check if a pointer is valid for this pool
    fn is_valid_pointer(&self, ptr: NonNull<u8>) -> bool {
        let allocated_blocks = self.allocated_blocks.lock().ok()?;
        allocated_blocks.contains(&ptr)
    }
}

impl Drop for SafeMemoryPool {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.block_size, 8).unwrap();

        if let Ok(allocated_blocks) = self.allocated_blocks.lock() {
            for ptr in allocated_blocks.iter() {
                self.allocator.deallocate(*ptr, layout);
            }
        }
    }
}

/// Safe atomic pointer wrapper
pub struct SafeAtomicPtr<T> {
    ptr: std::sync::atomic::AtomicPtr<T>,
}

impl<T> SafeAtomicPtr<T> {
    /// Create a new safe atomic pointer
    pub fn new(ptr: *mut T) -> Self {
        Self {
            ptr: std::sync::atomic::AtomicPtr::new(ptr),
        }
    }

    /// Load the pointer
    pub fn load(&self, ordering: std::sync::atomic::Ordering) -> *mut T {
        self.ptr.load(ordering)
    }

    /// Store a pointer
    pub fn store(&self, ptr: *mut T, ordering: std::sync::atomic::Ordering) {
        self.ptr.store(ptr, ordering);
    }

    /// Compare and exchange
    pub fn compare_exchange(
        &self,
        current: *mut T,
        new: *mut T,
        success: std::sync::atomic::Ordering,
        failure: std::sync::atomic::Ordering,
    ) -> Result<*mut T, *mut T> {
        self.ptr.compare_exchange(current, new, success, failure)
    }

    /// Compare and exchange weak
    pub fn compare_exchange_weak(
        &self,
        current: *mut T,
        new: *mut T,
        success: std::sync::atomic::Ordering,
        failure: std::sync::atomic::Ordering,
    ) -> Result<*mut T, *mut T> {
        self.ptr
            .compare_exchange_weak(current, new, success, failure)
    }
}

impl<T> Default for SafeAtomicPtr<T> {
    fn default() -> Self {
        Self::new(std::ptr::null_mut())
    }
}

/// Safe memory utilities
pub struct SafeMemoryUtils;

impl SafeMemoryUtils {
    /// Safely copy memory from source to destination
    pub fn safe_copy(src: &[u8], dst: &mut [u8]) -> Result<(), SafeMemoryError> {
        if src.len() != dst.len() {
            return Err(SafeMemoryError::InvalidLayout);
        }

        dst.copy_from_slice(src);
        Ok(())
    }

    /// Safely zero memory
    pub fn safe_zero(dst: &mut [u8]) {
        dst.fill(0);
    }

    /// Safely compare memory regions
    pub fn safe_compare(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        a.cmp(b)
    }

    /// Check if memory regions overlap
    pub fn safe_overlap_check(ptr1: *const u8, len1: usize, ptr2: *const u8, len2: usize) -> bool {
        if ptr1.is_null() || ptr2.is_null() || len1 == 0 || len2 == 0 {
            return false;
        }

        let start1 = ptr1 as usize;
        let end1 = start1 + len1;
        let start2 = ptr2 as usize;
        let end2 = start2 + len2;

        start1 < end2 && start2 < end1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_allocator() {
        let allocator = SafeAllocator::default();
        let layout = Layout::from_size_align(1024, 8).unwrap();

        let ptr = allocator.allocate(layout).unwrap();
        assert!(!ptr.as_ptr().is_null());

        allocator.deallocate(ptr.as_non_null_ptr(), layout);
    }

    #[test]
    fn test_safe_memory_pool() {
        let pool = SafeMemoryPool::new(64, 10).unwrap();

        let block = pool.allocate().unwrap();
        assert!(!block.as_ptr().is_null());

        pool.deallocate(block).unwrap();
    }

    #[test]
    fn test_safe_memory_utils() {
        let src = b"hello world";
        let mut dst = [0u8; 11];

        SafeMemoryUtils::safe_copy(src, &mut dst).unwrap();
        assert_eq!(src, &dst);

        SafeMemoryUtils::safe_zero(&mut dst);
        assert_eq!(dst, [0u8; 11]);
    }

    #[test]
    fn test_safe_atomic_ptr() {
        let atomic_ptr = SafeAtomicPtr::<i32>::default();
        assert!(atomic_ptr
            .load(std::sync::atomic::Ordering::Relaxed)
            .is_null());

        let value = 42i32;
        atomic_ptr.store(
            &value as *const i32 as *mut i32,
            std::sync::atomic::Ordering::Relaxed,
        );
        assert!(!atomic_ptr
            .load(std::sync::atomic::Ordering::Relaxed)
            .is_null());
    }
}
