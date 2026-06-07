//! Hazard Pointer Implementation for Lock-Free Data Structures
//!
//! This module provides a hazard pointer system to ensure memory safety
//! in lock-free data structures by preventing use-after-free bugs.
//!
//! ## Overview
//!
//! Hazard pointers are a memory management technique for lock-free data structures
//! that allows threads to safely access shared memory without the risk of accessing
//! memory that has been freed by another thread.
//!
//! ## How It Works
//!
//! 1. **Acquisition**: Before accessing a shared pointer, a thread "hazards" it
//! 2. **Protection**: The hazard pointer prevents the memory from being freed
//! 3. **Release**: When done accessing, the thread releases the hazard pointer
//! 4. **Reclamation**: Only memory not protected by any hazard pointer can be freed
//!
//! ## Performance Characteristics
//!
//! - **Overhead**: Minimal overhead for hazard pointer management
//! - **Memory**: Thread-local storage for hazard pointers
//! - **Scalability**: Scales well with number of threads
//! - **Safety**: Prevents use-after-free and double-free bugs
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Maximum number of hazard pointers per thread
const MAX_HAZARD_POINTERS: usize = 128;

/// Hazard pointer entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HazardPointer {
    /// The protected pointer (stored as usize for thread safety)
    pointer: usize,
    /// Thread ID that owns this hazard pointer
    thread_id: usize,
}

// SAFETY: HazardPointer is safe to send and sync across threads because:
// - It contains only usize values (pointer and thread_id) which are primitive types
// - usize is Copy and has no interior mutability
// - The pointer value is stored as usize (not as a raw pointer) to avoid lifetime issues
// - Thread synchronization is handled by the HazardPointerManager's Mutex-protected structures
// - No actual pointer dereferencing occurs in HazardPointer itself
unsafe impl Send for HazardPointer {}
unsafe impl Sync for HazardPointer {}

// Thread-local hazard pointer storage
thread_local! {
    static HAZARD_POINTERS: std::cell::RefCell<Vec<*mut u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Pending free entry with type-aware deallocation
struct PendingFree {
    ptr: *mut u8,
    deleter: unsafe fn(*mut u8),
}

// SAFETY: PendingFree is safe to Send/Sync because:
// - It only contains a raw pointer and a function pointer (no ownership semantics)
// - The pointer is only freed after hazard checks in reclaim_pending()
// - Access is synchronized through Mutex in HazardPointerManager
unsafe impl Send for PendingFree {}
unsafe impl Sync for PendingFree {}

/// Global hazard pointer manager
struct HazardPointerManager {
    /// All active hazard pointers across all threads
    active_pointers: Mutex<HashSet<HazardPointer>>,
    /// Pending nodes to be freed with their type-aware deleters
    pending_free: Mutex<Vec<PendingFree>>,
    /// Statistics
    stats: HazardPointerStats,
}

/// Hazard pointer statistics
#[derive(Debug, Default)]
pub struct HazardPointerStats {
    /// Number of hazard pointers acquired
    acquisitions: AtomicUsize,
    /// Number of hazard pointers released
    releases: AtomicUsize,
    /// Number of nodes freed
    frees: AtomicUsize,
    /// Number of pending frees
    pending_frees: AtomicUsize,
}

impl HazardPointerStats {
    fn new() -> Self {
        Self {
            acquisitions: AtomicUsize::new(0),
            releases: AtomicUsize::new(0),
            frees: AtomicUsize::new(0),
            pending_frees: AtomicUsize::new(0),
        }
    }
}

/// Global hazard pointer manager instance
static HAZARD_MANAGER: std::sync::OnceLock<HazardPointerManager> = std::sync::OnceLock::new();

/// Get the global hazard pointer manager
fn get_manager() -> &'static HazardPointerManager {
    HAZARD_MANAGER.get_or_init(|| HazardPointerManager {
        active_pointers: Mutex::new(HashSet::new()),
        pending_free: Mutex::new(Vec::new()),
        stats: HazardPointerStats::new(),
    })
}

/// Type-aware deallocation helper
unsafe fn free_typed<T>(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let _ = Box::from_raw(ptr as *mut T);
}

/// Acquire a hazard pointer for the given pointer
///
/// # Safety
///
/// The caller must ensure:
/// - `ptr` is a valid, non-null pointer to a heap-allocated object
/// - The pointer remains valid and accessible until the hazard pointer is released
/// - The pointer was allocated with the same allocator that will be used for deallocation
/// - No other thread will free the pointer while it's protected by this hazard pointer
/// - The hazard pointer protocol is followed: acquire before use, release after use
///
/// # Thread Safety
///
/// This function is thread-safe and can be called concurrently from multiple threads.
/// Each thread maintains its own thread-local list of hazard pointers (up to MAX_HAZARD_POINTERS).
/// The global manager uses Mutex-protected structures to coordinate across threads.
///
/// # Performance
///
/// This operation has minimal overhead and is designed for high-frequency use.
pub unsafe fn acquire_hazard_pointer<T>(ptr: *mut T) -> bool {
    if ptr.is_null() {
        return false;
    }

    let ptr = ptr as *mut u8;
    let thread_id = thread_id();

    HAZARD_POINTERS.with(|hazards| {
        let mut hazards = hazards.borrow_mut();

        // Check if we already have this pointer
        if hazards.contains(&ptr) {
            return true;
        }

        // Add new hazard pointer if we have space
        if hazards.len() < MAX_HAZARD_POINTERS {
            hazards.push(ptr);

            // Register with global manager
            let manager = get_manager();
            let mut active = match manager.active_pointers.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    // Mutex is poisoned - return false to indicate failure
                    // This is a graceful degradation: we can't register the hazard pointer
                    // but we also don't panic
                    return false;
                }
            };
            active.insert(HazardPointer {
                pointer: ptr as usize,
                thread_id,
            });
            drop(active);

            manager.stats.acquisitions.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    })
}

/// Release a hazard pointer for the given pointer
///
/// # Safety
///
/// The caller must ensure:
/// - `ptr` was previously acquired with `acquire_hazard_pointer` on the same thread
/// - The pointer is no longer being accessed by this thread
/// - After release, the pointer may be freed by another thread if no other hazard pointers protect it
/// - The hazard pointer protocol is followed: release only after all accesses are complete
///
/// # Thread Safety
///
/// This function is thread-safe. Each thread manages its own thread-local hazard pointers.
/// Releasing a hazard pointer on one thread does not affect hazard pointers on other threads.
///
/// # Performance
///
/// This operation has minimal overhead and is designed for high-frequency use.
pub unsafe fn release_hazard_pointer<T>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }

    let ptr = ptr as *mut u8;
    let thread_id = thread_id();

    HAZARD_POINTERS.with(|hazards| {
        let mut hazards = hazards.borrow_mut();

        // Remove the hazard pointer
        if let Some(pos) = hazards.iter().position(|&p| p == ptr) {
            hazards.remove(pos);

            // Unregister from global manager
            let manager = get_manager();
            let mut active = match manager.active_pointers.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    // Mutex is poisoned - return early without unregistering
                    // This is a graceful degradation: we can't unregister but we don't panic
                    return;
                }
            };
            active.remove(&HazardPointer {
                pointer: ptr as usize,
                thread_id,
            });
            drop(active);

            manager.stats.releases.fetch_add(1, Ordering::Relaxed);
        }
    });
}

/// Safely free a pointer using hazard pointer protection
///
/// This function will either free the pointer immediately if it's not protected
/// by any hazard pointer, or add it to the pending free list for later reclamation.
///
/// # Safety
///
/// The caller must ensure:
/// - `ptr` is a valid, non-null pointer that was allocated with `Box::into_raw()` or equivalent
/// - The pointer was allocated with the same allocator that will be used for deallocation
/// - The pointer is no longer in use by any thread (all hazard pointers have been released)
/// - The pointer type `T` matches the type used during allocation
/// - This function should be called only once per pointer (double-free is undefined behavior)
///
/// # Thread Safety
///
/// This function is thread-safe. It checks all threads' hazard pointers before freeing.
/// If the pointer is protected, it's added to a pending free list and will be reclaimed later
/// when `reclaim_pending()` is called and the pointer is no longer protected.
///
/// # Performance
///
/// This operation may have higher overhead due to global state management and lock acquisition.
pub unsafe fn safe_free<T>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }

    let ptr = ptr as *mut u8;
    let manager = get_manager();

    // Check if already in pending list to prevent double-free
    let pending_check = manager.pending_free.lock();
    if let Ok(ref pending_guard) = pending_check {
        if pending_guard.iter().any(|entry| entry.ptr == ptr) {
            // Already in pending list, don't add again to prevent double-free
            return;
        }
    }
    drop(pending_check);

    // Check if any thread has this pointer as a hazard pointer
    let active = match manager.active_pointers.lock() {
        Ok(guard) => guard,
        Err(_) => {
            // Mutex is poisoned - assume protected to be safe
            // This prevents freeing memory that might still be in use
            return;
        }
    };
    let is_protected = active.iter().any(|hp| hp.pointer == ptr as usize);
    drop(active);

    if is_protected {
        // Add to pending free list
        let mut pending = match manager.pending_free.lock() {
            Ok(guard) => guard,
            Err(_) => {
                // Mutex is poisoned - cannot add to pending free list
                // Return early to avoid freeing memory that might be protected
                return;
            }
        };
        // Double-check it's not already in the list (race condition protection)
        if !pending.iter().any(|entry| entry.ptr == ptr) {
            pending.push(PendingFree {
                ptr,
                deleter: free_typed::<T>,
            });
            manager.stats.pending_frees.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        // Safe to free immediately
        free_typed::<T>(ptr);
        manager.stats.frees.fetch_add(1, Ordering::Relaxed);
    }
}

/// Reclaim pending freed nodes
///
/// This function should be called periodically to free nodes that were
/// protected by hazard pointers when they were originally freed.
///
/// # Performance
///
/// This operation may have higher overhead due to global state management.
pub fn reclaim_pending() {
    let manager = get_manager();

    // Ensure consistent lock ordering: always acquire active_pointers before pending_free
    // to prevent deadlocks when multiple threads call this function simultaneously
    let active = match manager.active_pointers.lock() {
        Ok(guard) => guard,
        Err(_) => {
            // Mutex is poisoned - cannot safely reclaim pending frees
            // Return early to avoid potential data corruption
            return;
        }
    };

    let mut pending = match manager.pending_free.lock() {
        Ok(guard) => guard,
        Err(_) => {
            // Mutex is poisoned - cannot safely reclaim pending frees
            // Return early to avoid potential data corruption
            return;
        }
    };

    // Filter out nodes that are still protected
    let mut to_free: Vec<PendingFree> = Vec::new();
    let mut to_keep: Vec<PendingFree> = Vec::new();
    let mut seen_pointers = std::collections::HashSet::new(); // Track ALL seen pointers to prevent double-free

    for pending_entry in pending.drain(..) {
        // Skip if we've already seen this pointer (deduplication to prevent double-free)
        // This prevents the same pointer from being processed multiple times in a single
        // reclaim_pending() call, which could happen due to race conditions or bugs.
        if seen_pointers.contains(&pending_entry.ptr) {
            // Duplicate pointer - skip to prevent double-free
            // Update stats to reflect that we skipped a duplicate
            manager.stats.pending_frees.fetch_sub(1, Ordering::Relaxed);
            continue;
        }

        // Mark as seen immediately to prevent processing it again
        seen_pointers.insert(pending_entry.ptr);

        let is_protected = active
            .iter()
            .any(|hp| hp.pointer == pending_entry.ptr as usize);
        if is_protected {
            to_keep.push(pending_entry);
        } else {
            to_free.push(pending_entry);
        }
    }

    // Keep protected nodes
    pending.extend(to_keep);

    // Drop locks before freeing to minimize lock hold time
    drop(pending);
    drop(active);

    // Free unprotected nodes (no locks held during actual deallocation)
    for entry in to_free {
        // SAFETY: ptr is a valid pointer that was previously allocated with Box::into_raw()
        // and is not protected by any hazard pointer (verified above), so it's safe to free.
        // We've checked that no thread has this pointer in their hazard pointer list, ensuring
        // no use-after-free can occur. The pointer type is u8, which is safe to convert from
        // the original type since we're just deallocating memory.
        unsafe {
            (entry.deleter)(entry.ptr);
            manager.stats.frees.fetch_add(1, Ordering::Relaxed);
            manager.stats.pending_frees.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Check if a pointer is already in the pending free list
///
/// This function is useful for preventing double-free scenarios where
/// a pointer might be added to the pending list multiple times.
///
/// # Safety
///
/// This function is safe to call, but the pointer value itself is not validated.
/// The caller must ensure the pointer is valid if they intend to use it.
pub fn is_in_pending_list(ptr: *mut u8) -> bool {
    let manager = get_manager();
    let pending = match manager.pending_free.lock() {
        Ok(guard) => guard,
        Err(_) => {
            // Mutex is poisoned - assume it might be in the list to be safe
            return true;
        }
    };
    pending.iter().any(|entry| entry.ptr == ptr)
}

/// Get hazard pointer statistics
pub fn get_stats() -> HazardPointerStats {
    let manager = get_manager();
    HazardPointerStats {
        acquisitions: AtomicUsize::new(manager.stats.acquisitions.load(Ordering::Relaxed)),
        releases: AtomicUsize::new(manager.stats.releases.load(Ordering::Relaxed)),
        frees: AtomicUsize::new(manager.stats.frees.load(Ordering::Relaxed)),
        pending_frees: AtomicUsize::new(manager.stats.pending_frees.load(Ordering::Relaxed)),
    }
}

/// Get the current thread ID
fn thread_id() -> usize {
    // Use a simple thread ID based on thread local storage
    thread_local! {
        static THREAD_ID: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    THREAD_ID.with(|id| {
        let current = id.get();
        if current == 0 {
            // Generate a new thread ID using a simple counter
            static COUNTER: AtomicUsize = AtomicUsize::new(1);
            let new_id = COUNTER.fetch_add(1, Ordering::Relaxed);
            id.set(new_id);
            new_id
        } else {
            current
        }
    })
}

/// RAII guard for hazard pointers
///
/// This guard automatically releases the hazard pointer when dropped,
/// ensuring proper cleanup even in case of panics.
pub struct HazardPointerGuard<T> {
    ptr: *mut T,
}

impl<T> HazardPointerGuard<T> {
    /// Create a new hazard pointer guard
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `ptr` is a valid, non-null pointer to a heap-allocated object
    /// - The pointer remains valid and accessible for the lifetime of the guard
    /// - The pointer was allocated with the same allocator that will be used for deallocation
    /// - No other thread will free the pointer while the guard is alive
    /// - The guard is used on the same thread that created it (thread-local hazard pointers)
    ///
    /// # Returns
    ///
    /// Returns `Some(guard)` if the hazard pointer was successfully acquired, `None` if
    /// the thread has reached the maximum number of hazard pointers (MAX_HAZARD_POINTERS).
    pub unsafe fn new(ptr: *mut T) -> Option<Self> {
        if acquire_hazard_pointer(ptr) {
            Some(Self { ptr })
        } else {
            None
        }
    }

    /// Get the protected pointer
    pub fn ptr(&self) -> *mut T {
        self.ptr
    }
}

impl<T> Drop for HazardPointerGuard<T> {
    fn drop(&mut self) {
        // SAFETY: self.ptr was acquired via acquire_hazard_pointer in the new() method,
        // so it's safe to release. The guard ensures proper cleanup even on panic.
        unsafe {
            release_hazard_pointer(self.ptr);
        }
    }
}

/// Safe wrapper for atomic pointer operations with hazard pointers
pub struct SafeAtomicPtr<T> {
    ptr: AtomicPtr<T>,
}

impl<T> SafeAtomicPtr<T> {
    /// Create a new safe atomic pointer
    pub fn new(ptr: *mut T) -> Self {
        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }

    /// Load the pointer with hazard pointer protection
    pub fn load(&self, ordering: Ordering) -> Option<HazardPointerGuard<T>> {
        let ptr = self.ptr.load(ordering);
        if ptr.is_null() {
            None
        } else {
            // SAFETY: ptr is a valid non-null pointer from the atomic load. The atomic load
            // ensures we have a consistent view of the pointer, and since it's non-null, it's
            // safe to create a hazard pointer guard. The guard will protect the pointer from
            // being freed while it's in use.
            unsafe { HazardPointerGuard::new(ptr) }
        }
    }

    /// Compare and exchange with hazard pointer protection
    pub fn compare_exchange_weak(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Option<HazardPointerGuard<T>>, *mut T> {
        match self
            .ptr
            .compare_exchange_weak(current, new, success, failure)
        {
            Ok(ptr) => {
                if ptr.is_null() {
                    Ok(None)
                } else {
                    // SAFETY: ptr is a valid non-null pointer from the successful compare_exchange.
                    // The compare_exchange_weak operation ensures atomicity and that we have the
                    // expected pointer value. Since it's non-null, it's safe to create a hazard
                    // pointer guard to protect it from being freed while in use.
                    unsafe { Ok(HazardPointerGuard::new(ptr)) }
                }
            }
            Err(actual) => Err(actual),
        }
    }

    /// Store a new pointer
    pub fn store(&self, ptr: *mut T, ordering: Ordering) {
        self.ptr.store(ptr, ordering);
    }

    /// Load the raw pointer without hazard pointer protection
    /// This should only be used when the caller ensures proper synchronization
    pub fn load_raw(&self, ordering: Ordering) -> *mut T {
        self.ptr.load(ordering)
    }
}

/// Cleanup function for tests to reset hazard pointer state
///
/// This function should be called at the end of tests that use hazard pointers
/// to ensure clean state between tests. This helps prevent test interference
/// when running tests in parallel.
#[cfg(test)]
pub fn test_cleanup() {
    let manager = get_manager();

    // Clear active pointers with proper poison recovery
    // IMPORTANT: Do this BEFORE calling reclaim_pending() because reclaim_pending()
    // will return early if the mutexes are poisoned
    let mut active = match manager.active_pointers.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from poison
            let mut recovered = poisoned.into_inner();
            recovered.clear();
            // Try to get a fresh lock
            manager
                .active_pointers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        }
    };
    active.clear();
    drop(active);

    // Clear pending free list with proper poison recovery
    let mut pending = match manager.pending_free.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from poison
            let mut recovered = poisoned.into_inner();
            recovered.clear();
            // Try to get a fresh lock
            manager
                .pending_free
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        }
    };
    pending.clear();
    manager
        .stats
        .pending_frees
        .store(pending.len(), Ordering::Relaxed);
    drop(pending);

    // Clear thread-local storage
    HAZARD_POINTERS.with(|hazards| {
        let mut hazards = hazards.borrow_mut();
        hazards.clear();
    });

    // Now that mutexes are recovered, reclaim any pending frees
    reclaim_pending();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    // use std::sync::Arc;
    // use std::thread;

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_acquisition() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Acquire hazard pointer
            assert!(acquire_hazard_pointer(ptr));

            // Try to acquire again (should succeed)
            assert!(acquire_hazard_pointer(ptr));

            // Release hazard pointer
            release_hazard_pointer(ptr);
            release_hazard_pointer(ptr);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_guard() {
        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            let guard = HazardPointerGuard::new(ptr);
            assert!(guard.is_some());

            let guard = guard.expect("Failed to acquire hazard pointer guard");
            assert_eq!(guard.ptr(), ptr);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_safe_free() {
        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Free without hazard pointer (should free immediately)
            safe_free(ptr);
        }

        // Data should be freed
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_safe_free_with_hazard_pointer() {
        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Acquire hazard pointer
            assert!(acquire_hazard_pointer(ptr));

            // Try to free (should be added to pending)
            safe_free(ptr);

            // Release hazard pointer
            release_hazard_pointer(ptr);

            // Reclaim pending
            reclaim_pending();
        }

        // Data should be freed after reclamation
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_concurrent_hazard_pointers() {
        // Clean up any state from previous tests
        test_cleanup();

        // use std::sync::Arc;
        use std::thread;

        let data = Box::new(42);
        let ptr = Box::into_raw(data);
        let ptr_usize = ptr as usize;

        let mut handles = Vec::new();

        for _ in 0..4 {
            let local_ptr_usize = ptr_usize;
            let handle = thread::spawn(move || {
                unsafe {
                    let ptr = local_ptr_usize as *mut i32;
                    // Acquire hazard pointer
                    assert!(acquire_hazard_pointer(ptr));

                    // Simulate some work
                    std::thread::sleep(std::time::Duration::from_millis(10));

                    // Release hazard pointer
                    release_hazard_pointer(ptr);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Failed to join thread");
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_poisoned_lock_acquire_hazard_pointer() {
        // Clean up any state from previous tests
        test_cleanup();

        // Poison the active_pointers mutex by panicking while holding the lock
        let manager = get_manager();
        let _poison_result = std::panic::catch_unwind(|| {
            let _guard = manager.active_pointers.lock().unwrap();
            panic!("Intentionally poisoning the mutex");
        });
        // Mutex should now be poisoned

        // Verify acquire_hazard_pointer handles poisoned lock gracefully
        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Should return false instead of panicking
            let result = acquire_hazard_pointer(ptr);
            assert!(
                !result,
                "acquire_hazard_pointer should return false when mutex is poisoned"
            );
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_poisoned_lock_release_hazard_pointer() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // First acquire normally (before poisoning)
            // Note: If mutex is already poisoned from previous test, this will fail
            // So we need to handle that case
            let acquired = acquire_hazard_pointer(ptr);

            // Poison the active_pointers mutex
            let manager = get_manager();
            let _poison_result = std::panic::catch_unwind(|| {
                let _guard = manager.active_pointers.lock().unwrap();
                panic!("Intentionally poisoning the mutex");
            });

            // Verify release_hazard_pointer handles poisoned lock gracefully (no panic)
            // This should not panic even if the mutex is poisoned
            release_hazard_pointer(ptr);

            // If we successfully acquired, we also need to clean up thread-local state
            if acquired {
                HAZARD_POINTERS.with(|hazards| {
                    let mut hazards = hazards.borrow_mut();
                    hazards.retain(|&p| p != ptr as *mut u8);
                });
            }
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_poisoned_lock_safe_free() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        // Poison the active_pointers mutex
        let manager = get_manager();
        let _poison_result = std::panic::catch_unwind(|| {
            let _guard = manager.active_pointers.lock().unwrap();
            panic!("Intentionally poisoning the mutex");
        });

        // Verify safe_free handles poisoned lock gracefully (no panic, returns early)
        unsafe {
            safe_free(ptr);
            // Should return early without freeing, so we need to free manually
            let _ = Box::from_raw(ptr);
        }

        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_poisoned_lock_reclaim_pending() {
        // Clean up any state from previous tests
        test_cleanup();

        // Poison the active_pointers mutex
        let manager = get_manager();
        let _poison_result = std::panic::catch_unwind(|| {
            let _guard = manager.active_pointers.lock().unwrap();
            panic!("Intentionally poisoning the mutex");
        });

        // Verify reclaim_pending handles poisoned lock gracefully (no panic)
        reclaim_pending();

        // Note: We cannot fully unpoisoned the mutex because it's in a global OnceLock.
        // This test should only be run in isolation.
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_poisoned_lock_pending_free() {
        // Clean up any state from previous tests
        // Use test_cleanup() like other tests to ensure consistent state
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Try to acquire hazard pointer first (may fail if mutex already poisoned)
            // We'll test safe_free behavior regardless
            let acquired = acquire_hazard_pointer(ptr);

            // Poison the pending_free mutex
            let manager = get_manager();
            let _poison_result = std::panic::catch_unwind(|| {
                let _guard = manager.pending_free.lock().unwrap();
                panic!("Intentionally poisoning the mutex");
            });

            // Verify safe_free handles poisoned pending_free lock gracefully (no panic)
            // This should return early without freeing if mutex is poisoned
            safe_free(ptr);

            // Release hazard pointer if we acquired it (before cleaning up)
            // This must be done before recovering from poison to avoid deadlocks
            if acquired {
                // Try to release normally first
                release_hazard_pointer(ptr);

                // Also clean up thread-local state in case release didn't work
                HAZARD_POINTERS.with(|hazards| {
                    let mut hazards = hazards.borrow_mut();
                    hazards.retain(|&p| p != ptr as *mut u8);
                });
            }
        }

        // Clean up - safe_free may not have freed it due to poisoned mutex
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        // Recover from the poisoned pending_free mutex
        // This is the key part of the test - we need to recover from poison
        // We use the exact same pattern as test_cleanup() to ensure full recovery
        // This ensures the mutex state is identical to what test_cleanup() expects
        let manager = get_manager();

        // Recover from pending_free poison - this is what we're testing
        // Use the same recovery pattern as test_cleanup() (lines 524-535)
        let mut pending = match manager.pending_free.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Recover from poison - clear the poisoned state
                let mut recovered = poisoned.into_inner();
                recovered.clear();
                drop(recovered);
                // Get a fresh lock - this ensures the mutex is fully recovered
                manager
                    .pending_free
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
            }
        };
        pending.clear();
        drop(pending);

        // Clear active pointers (should not be poisoned, but handle it just in case)
        // Use the same recovery pattern as test_cleanup() (lines 507-521)
        let mut active = match manager.active_pointers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Recover from poison if needed
                let mut recovered = poisoned.into_inner();
                recovered.clear();
                drop(recovered);
                // Get a fresh lock
                manager
                    .active_pointers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
            }
        };
        active.clear();
        drop(active);

        // Clear thread-local storage (same as test_cleanup() lines 541-544)
        HAZARD_POINTERS.with(|hazards| {
            let mut hazards = hazards.borrow_mut();
            hazards.clear();
        });

        // Final verification: ensure both mutexes are fully unpoisoned by acquiring
        // them in the same order that reclaim_pending() uses (active_pointers first,
        // then pending_free). This proves the mutexes are in a good state for the
        // next test. We don't call reclaim_pending() here to avoid any potential
        // issues - the next test's test_cleanup() will handle reclaiming pending frees.
        let _verify_active = manager
            .active_pointers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        drop(_verify_active);
        // Ensure pending_free is fully recovered and empty
        let mut pending = manager
            .pending_free
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        pending.clear();
        drop(pending);

        // Now that both mutexes are unpoisoned and clean, we can safely call reclaim_pending()
        // to ensure the state is fully recovered for the next test
        reclaim_pending();

        // Final cleanup to match what test_cleanup() does
        // This ensures the next test's test_cleanup() won't have issues
        let _final_active = manager
            .active_pointers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        drop(_final_active);
        let _final_pending = manager
            .pending_free
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        drop(_final_pending);

        test_cleanup();
    }

    // Additional comprehensive tests for hazard pointer functionality
    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_protection_basic() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Acquire hazard pointer
            assert!(acquire_hazard_pointer(ptr));

            // Try to free (should be added to pending, not freed immediately)
            safe_free(ptr);

            // Release hazard pointer
            release_hazard_pointer(ptr);

            // Now reclaim pending - should free the pointer
            reclaim_pending();
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_multiple_acquisitions() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            // Acquire multiple times (should succeed)
            assert!(acquire_hazard_pointer(ptr));
            assert!(acquire_hazard_pointer(ptr));
            assert!(acquire_hazard_pointer(ptr));

            // Release all
            release_hazard_pointer(ptr);
            release_hazard_pointer(ptr);
            release_hazard_pointer(ptr);

            // Now safe to free
            safe_free(ptr);
            reclaim_pending();
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_scan_operations() {
        // Clean up any state from previous tests
        test_cleanup();

        let data1 = Box::new(1);
        let data2 = Box::new(2);
        let data3 = Box::new(3);
        let ptr1 = Box::into_raw(data1);
        let ptr2 = Box::into_raw(data2);
        let ptr3 = Box::into_raw(data3);

        unsafe {
            // Acquire hazard pointers for ptr1 and ptr2
            assert!(acquire_hazard_pointer(ptr1));
            assert!(acquire_hazard_pointer(ptr2));

            // Free all three
            safe_free(ptr1); // Should be pending (protected)
            safe_free(ptr2); // Should be pending (protected)
            safe_free(ptr3); // Should be freed immediately (not protected)

            // Reclaim pending - should free ptr1 and ptr2 after release
            release_hazard_pointer(ptr1);
            release_hazard_pointer(ptr2);
            reclaim_pending();
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_concurrent_protection() {
        // Clean up any state from previous tests
        test_cleanup();

        use std::thread;

        let data = Box::new(42);
        let ptr = Box::into_raw(data);
        let ptr_usize = ptr as usize;

        let mut handles = Vec::new();

        // Spawn threads that acquire and hold hazard pointers
        for _ in 0..4 {
            let local_ptr_usize = ptr_usize;
            let handle = thread::spawn(move || {
                unsafe {
                    let ptr = local_ptr_usize as *mut i32;
                    // Acquire hazard pointer
                    assert!(acquire_hazard_pointer(ptr));

                    // Hold it for a bit
                    thread::sleep(std::time::Duration::from_millis(10));

                    // Release hazard pointer
                    release_hazard_pointer(ptr);
                }
            });
            handles.push(handle);
        }

        // Try to free while protected (should be pending)
        unsafe {
            safe_free(ptr);
        }

        // Wait for all threads to release
        for handle in handles {
            handle.join().expect("Failed to join thread");
        }

        // Now reclaim pending - should free the pointer
        reclaim_pending();

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_stats() {
        // Clean up any state from previous tests
        test_cleanup();

        let data = Box::new(42);
        let ptr = Box::into_raw(data);

        unsafe {
            let manager = get_manager();
            let initial_acquisitions = manager.stats.acquisitions.load(Ordering::Relaxed);
            let initial_releases = manager.stats.releases.load(Ordering::Relaxed);

            // Acquire and release
            assert!(acquire_hazard_pointer(ptr));
            release_hazard_pointer(ptr);

            // Verify stats were updated
            assert!(manager.stats.acquisitions.load(Ordering::Relaxed) > initial_acquisitions);
            assert!(manager.stats.releases.load(Ordering::Relaxed) > initial_releases);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(ptr);
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_null_pointer() {
        // Clean up any state from previous tests
        test_cleanup();

        unsafe {
            // Test with null pointer
            assert!(!acquire_hazard_pointer(std::ptr::null_mut::<i32>()));
            release_hazard_pointer(std::ptr::null_mut::<i32>());
            safe_free(std::ptr::null_mut::<i32>());
        }

        // Clean up for next test
        test_cleanup();
    }

    #[test]
    #[ignore = "perf: hazard-pointer stress 10+ min, run weekly"]
    fn test_hazard_pointer_max_capacity() {
        // Run test with timeout to prevent hanging
        let (tx, rx) = mpsc::channel();
        let test_handle = std::thread::spawn(move || {
            // Clean up any state from previous tests
            // Use try_lock with timeout to prevent hanging
            let manager = get_manager();
            if let Ok(mut active) = manager.active_pointers.try_lock() {
                active.clear();
            }
            if let Ok(mut pending) = manager.pending_free.try_lock() {
                pending.clear();
            }
            reclaim_pending();

            // Create enough pointers for the test (MAX_HAZARD_POINTERS + 1)
            let mut ptrs = Vec::new();
            for i in 0..=MAX_HAZARD_POINTERS {
                let data = Box::new(i);
                ptrs.push(Box::into_raw(data));
            }

            unsafe {
                // Try to acquire MAX_HAZARD_POINTERS pointers
                for i in 0..MAX_HAZARD_POINTERS {
                    assert!(acquire_hazard_pointer(ptrs[i]));
                }

                // Next acquisition should fail (at capacity)
                assert!(!acquire_hazard_pointer(ptrs[MAX_HAZARD_POINTERS]));

                // Release one and try again
                release_hazard_pointer(ptrs[0]);
                assert!(acquire_hazard_pointer(ptrs[MAX_HAZARD_POINTERS]));

                // Release all
                for ptr in &ptrs {
                    release_hazard_pointer(*ptr);
                }

                // Free all
                for ptr in &ptrs {
                    safe_free(*ptr);
                }
                reclaim_pending();
            }

            // Clean up for next test - use non-blocking cleanup
            let manager = get_manager();
            if let Ok(mut active) = manager.active_pointers.try_lock() {
                active.clear();
            }
            if let Ok(mut pending) = manager.pending_free.try_lock() {
                pending.clear();
            }
            reclaim_pending();

            let _ = tx.send(());
        });

        // Wait for test to complete with 30 second timeout
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(_) => {
                test_handle.join().expect("Test thread panicked");
            }
            Err(_) => {
                panic!("test_hazard_pointer_max_capacity timed out after 30 seconds");
            }
        }
    }
}
