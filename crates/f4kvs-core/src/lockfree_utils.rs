//! Lock-free data structure utility functions
//!
//! This module contains utility functions and helpers for lock-free data structures,
//! extracted to improve code organization and maintainability.

/// Helper function to safely acquire hazard pointer with error handling
///
/// This function protects a raw pointer from being freed by another thread while it's in use.
/// The hazard pointer must be released by calling `safe_release_hazard_pointer` when done.
///
/// # Safety
///
/// This function is unsafe because it operates on raw pointers. The caller must ensure:
///
/// ## Safety Invariants
/// - The pointer `ptr` must be valid and point to properly allocated memory
/// - The pointer must remain valid for the entire duration that the hazard pointer is held
/// - The pointer must not be freed while the hazard pointer is held
///
/// ## Preconditions
/// - The pointer may be null (checked internally, returns `false` if null)
/// - The pointer must be a valid pointer to a `T` instance
/// - The caller must have obtained the pointer through safe means (e.g., from an atomic load)
///
/// ## Postconditions
/// - If the function returns `true`, the pointer is protected from being freed
/// - If the function returns `false`, the pointer is not protected (null or limit reached)
/// - The caller must call `safe_release_hazard_pointer` with the same pointer when done
///
/// ## Lifetime Requirements
/// - The pointer must remain valid until `safe_release_hazard_pointer` is called
/// - The memory pointed to must not be deallocated while the hazard pointer is held
/// - The pointer must not be modified to point to different memory while protected
///
/// ## Thread Safety
/// - This function is thread-safe and can be called concurrently from multiple threads
/// - Each thread has its own hazard pointer storage (thread-local)
/// - Multiple threads can protect the same pointer simultaneously
/// - The function uses thread-local storage and global synchronization internally
///
/// ## Error Conditions
/// - Returns `false` if the pointer is null (checked internally)
/// - Returns `false` if the thread has reached the maximum number of hazard pointers (MAX_HAZARD_POINTERS)
/// - Returns `true` if the hazard pointer was successfully acquired
/// - Never panics or causes undefined behavior (safe error handling)
///
/// # Example
///
/// ```rust,no_run
/// use f4kvs_core::lockfree_utils::*;
/// use std::sync::atomic::{AtomicPtr, Ordering};
///
/// let atomic = AtomicPtr::new(Box::into_raw(Box::new(42)));
/// let ptr = atomic.load(Ordering::Acquire);
///
/// unsafe {
///     // Acquire hazard pointer to protect from concurrent deallocation
///     if safe_acquire_hazard_pointer(ptr) {
///         // Safe to use ptr here - it's protected from being freed
///         if !ptr.is_null() {
///             println!("Value: {}", *ptr);
///         }
///         // Must release when done
///         safe_release_hazard_pointer(ptr);
///     }
/// }
/// ```
pub unsafe fn safe_acquire_hazard_pointer<T>(ptr: *mut T) -> bool {
    if ptr.is_null() {
        return false;
    }

    // SAFETY: We've checked that ptr is not null
    crate::hazard_pointers::acquire_hazard_pointer(ptr)
}

/// Helper function to safely release hazard pointer
///
/// This function releases a previously acquired hazard pointer, allowing the protected
/// memory to be freed if no other threads are protecting it.
///
/// # Safety
///
/// This function is unsafe because it operates on raw pointers. The caller must ensure:
///
/// ## Safety Invariants
/// - The pointer `ptr` must be the exact same pointer that was passed to `safe_acquire_hazard_pointer`
/// - The pointer must have been successfully acquired (returned `true`) before calling this function
/// - The pointer must not be used after this function returns (unless protected again)
///
/// ## Preconditions
/// - The pointer must have been previously acquired with `safe_acquire_hazard_pointer`
/// - The pointer may be null (checked internally, no-op if null)
/// - The pointer must be the same value that was used for acquisition (not a different pointer to the same memory)
///
/// ## Postconditions
/// - The hazard pointer is released and no longer protects the pointer
/// - The memory may be freed if no other threads are protecting it
/// - The pointer is no longer safe to use unless protected again
/// - The function never panics (safe to call even if pointer was never acquired)
///
/// ## Lifetime Requirements
/// - The pointer must not be used after this function returns
/// - The memory may be deallocated immediately after this call if no other hazard pointers protect it
/// - The caller must ensure no other references to the memory exist after release
///
/// ## Thread Safety
/// - This function is thread-safe and can be called concurrently from multiple threads
/// - Each thread manages its own hazard pointers independently
/// - Releasing a hazard pointer in one thread does not affect other threads' protection
/// - The function uses thread-local storage and global synchronization internally
///
/// ## Error Conditions
/// - Safe to call with null pointer (no-op, returns immediately)
/// - Safe to call even if pointer was never acquired (no-op)
/// - Safe to call multiple times (idempotent, but not recommended)
/// - Never panics or causes undefined behavior
///
/// # Example
///
/// ```rust,no_run
/// use f4kvs_core::lockfree_utils::*;
/// use std::sync::atomic::{AtomicPtr, Ordering};
///
/// let atomic = AtomicPtr::new(Box::into_raw(Box::new(42)));
/// let ptr = atomic.load(Ordering::Acquire);
///
/// unsafe {
///     if safe_acquire_hazard_pointer(ptr) {
///         // Use ptr safely
///         if !ptr.is_null() {
///             println!("Value: {}", *ptr);
///         }
///         // Must release when done
///         safe_release_hazard_pointer(ptr);
///         // ptr is no longer protected - don't use it here
///     }
/// }
/// ```
pub unsafe fn safe_release_hazard_pointer<T>(ptr: *mut T) {
    if !ptr.is_null() {
        // SAFETY: We've checked that ptr is not null
        crate::hazard_pointers::release_hazard_pointer(ptr)
    }
}

// NOTE: The following placeholder functions were removed as they were not used
// and cannot be implemented generically without knowing the specific type structure:
// - safe_load_atomic_ptr
// - safe_store_atomic_ptr
// - safe_compare_exchange_weak
// - safe_compare_exchange_strong
//
// If you need these functions, implement them for specific types that have
// AtomicPtr fields, or use the AtomicPtr type directly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_acquire_hazard_pointer() {
        // Test with null pointer
        unsafe {
            assert!(!safe_acquire_hazard_pointer(std::ptr::null_mut::<i32>()));
        }
    }

    #[test]
    fn test_safe_release_hazard_pointer() {
        // Test with null pointer - should not panic
        unsafe {
            safe_release_hazard_pointer(std::ptr::null_mut::<i32>());
        }
    }
}
