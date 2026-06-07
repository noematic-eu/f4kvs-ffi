//! Comprehensive coverage tests for hazard pointers
//!
//! This module provides extensive tests for hazard pointer functionality
//! including memory protection, retirement, scanning, and concurrent scenarios.

use f4kvs_core::hazard_pointers;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Test suite for hazard pointer basic operations
#[cfg(test)]
mod basic_tests {
    use super::*;

    #[test]
    fn test_acquire_null_pointer() {
        // Acquiring hazard pointer for null should fail
        let result =
            unsafe { hazard_pointers::acquire_hazard_pointer(std::ptr::null_mut::<i32>()) };
        assert!(!result);
    }

    #[test]
    fn test_acquire_release_pointer() {
        let data = Box::into_raw(Box::new(42i32));

        // Acquire hazard pointer
        let acquired = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
        assert!(acquired);

        // Release hazard pointer
        unsafe {
            hazard_pointers::release_hazard_pointer(data);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }

    #[test]
    fn test_acquire_same_pointer_twice() {
        let data = Box::into_raw(Box::new(100i32));

        // Acquire first time
        let acquired1 = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
        assert!(acquired1);

        // Acquire second time (should succeed as it's the same pointer)
        let acquired2 = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
        assert!(acquired2);

        // Release twice
        unsafe {
            hazard_pointers::release_hazard_pointer(data);
            hazard_pointers::release_hazard_pointer(data);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }

    #[test]
    fn test_safe_free_with_hazard() {
        let data = Box::into_raw(Box::new(200i32));

        // Acquire hazard pointer
        let acquired = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
        assert!(acquired);

        // Try to free (should be deferred)
        unsafe {
            hazard_pointers::safe_free(data);
        }

        // Release hazard pointer
        unsafe {
            hazard_pointers::release_hazard_pointer(data);
        }

        // Reclaim pending (should free now)
        hazard_pointers::reclaim_pending();

        // Data should be freed (we can't verify this directly, but no crash means success)
    }

    #[test]
    fn test_safe_free_without_hazard() {
        let data = Box::into_raw(Box::new(300i32));

        // Free without hazard (should be deferred)
        unsafe {
            hazard_pointers::safe_free(data);
        }

        // Reclaim pending (should free now)
        hazard_pointers::reclaim_pending();
    }

    #[test]
    fn test_get_stats() {
        let stats = hazard_pointers::get_stats();
        // Stats should be accessible (just verify it doesn't panic)
        // Fields are private, so we can't access them directly
        drop(stats);
    }

    #[test]
    fn test_reclaim_pending() {
        // Create some pointers and free them
        let ptrs: Vec<*mut i32> = (0..10).map(|i| Box::into_raw(Box::new(i))).collect();

        // Free all pointers
        for ptr in &ptrs {
            unsafe {
                hazard_pointers::safe_free(*ptr);
            }
        }

        // Reclaim pending
        hazard_pointers::reclaim_pending();

        // No crash means success
    }
}

/// Test suite for HazardPointerGuard
#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn test_hazard_pointer_guard_creation() {
        let data = Box::into_raw(Box::new(500i32));

        // Create guard
        let guard = unsafe { hazard_pointers::HazardPointerGuard::new(data) };
        assert!(guard.is_some());

        // Get pointer from guard
        if let Some(guard) = guard {
            let ptr = guard.ptr();
            assert_eq!(ptr, data);

            // Guard is dropped here, releasing hazard pointer
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }

    #[test]
    fn test_hazard_pointer_guard_null() {
        // Creating guard with null should fail
        let guard =
            unsafe { hazard_pointers::HazardPointerGuard::new(std::ptr::null_mut::<i32>()) };
        assert!(guard.is_none());
    }

    #[test]
    fn test_hazard_pointer_guard_drop_releases() {
        let data = Box::into_raw(Box::new(600i32));

        {
            // Create guard
            let guard = unsafe { hazard_pointers::HazardPointerGuard::new(data) };
            assert!(guard.is_some());
            // Guard is dropped here, should release hazard pointer
        }

        // Now safe to free
        unsafe {
            hazard_pointers::safe_free(data);
        }

        hazard_pointers::reclaim_pending();
    }
}

/// Test suite for SafeAtomicPtr
#[cfg(test)]
mod atomic_ptr_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_safe_atomic_ptr_load() {
        let data = Box::into_raw(Box::new(700i32));
        let atomic = hazard_pointers::SafeAtomicPtr::new(data);

        // Load with guard
        let guard = atomic.load(Ordering::Acquire);
        assert!(guard.is_some());

        if let Some(guard) = guard {
            assert_eq!(guard.ptr(), data);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }

    #[test]
    fn test_safe_atomic_ptr_load_null() {
        let atomic = hazard_pointers::SafeAtomicPtr::<i32>::new(std::ptr::null_mut());

        // Load should return None for null
        let guard = atomic.load(Ordering::Acquire);
        assert!(guard.is_none());
    }

    #[test]
    fn test_safe_atomic_ptr_store() {
        let data1 = Box::into_raw(Box::new(800i32));
        let data2 = Box::into_raw(Box::new(900i32));
        let atomic = hazard_pointers::SafeAtomicPtr::new(data1);

        // Store new value
        atomic.store(data2, Ordering::Release);

        // Load should return new value
        let guard = atomic.load(Ordering::Acquire);
        assert!(guard.is_some());
        if let Some(guard) = guard {
            assert_eq!(guard.ptr(), data2);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data1);
            let _ = Box::from_raw(data2);
        }
    }

    #[test]
    fn test_safe_atomic_ptr_compare_exchange() {
        let data1 = Box::into_raw(Box::new(1000i32));
        let data2 = Box::into_raw(Box::new(1100i32));
        let data3 = Box::into_raw(Box::new(1200i32));
        let atomic = hazard_pointers::SafeAtomicPtr::new(data1);

        // Successful exchange
        let result =
            atomic.compare_exchange_weak(data1, data2, Ordering::AcqRel, Ordering::Acquire);
        assert!(result.is_ok());

        // Failed exchange (current value is data2, not data1)
        let result =
            atomic.compare_exchange_weak(data1, data3, Ordering::AcqRel, Ordering::Acquire);
        assert!(result.is_err());

        // Load should still be data2
        let guard = atomic.load(Ordering::Acquire);
        assert!(guard.is_some());
        if let Some(guard) = guard {
            assert_eq!(guard.ptr(), data2);
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data1);
            let _ = Box::from_raw(data2);
            let _ = Box::from_raw(data3);
        }
    }

    #[test]
    fn test_safe_atomic_ptr_load_raw() {
        let data = Box::into_raw(Box::new(1300i32));
        let atomic = hazard_pointers::SafeAtomicPtr::new(data);

        // Load raw (without guard)
        let ptr = atomic.load_raw(Ordering::Acquire);
        assert_eq!(ptr, data);

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }
}

/// Test suite for concurrent hazard pointer operations
#[cfg(test)]
mod concurrent_tests {
    use super::*;

    #[test]
    fn test_concurrent_acquire_release() {
        // Each thread creates its own pointer to avoid Send issues
        let mut handles = vec![];

        // Spawn multiple threads acquiring and releasing hazard pointers
        for thread_id in 0..10 {
            let handle = thread::spawn(move || {
                let data = Box::into_raw(Box::new(1400i32 + thread_id));
                for _ in 0..100 {
                    let acquired = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
                    assert!(acquired);

                    thread::sleep(Duration::from_micros(1));

                    unsafe {
                        hazard_pointers::release_hazard_pointer(data);
                    }
                }
                // Clean up
                unsafe {
                    let _ = Box::from_raw(data);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_safe_free() {
        let mut handles = vec![];

        // Spawn threads that create and free pointers
        for thread_id in 0..10 {
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let data = Box::into_raw(Box::new(thread_id * 1000 + i));

                    // Acquire hazard pointer
                    let acquired = unsafe { hazard_pointers::acquire_hazard_pointer(data) };
                    assert!(acquired);

                    thread::sleep(Duration::from_micros(1));

                    // Release
                    unsafe {
                        hazard_pointers::release_hazard_pointer(data);
                    }

                    // Free
                    unsafe {
                        hazard_pointers::safe_free(data);
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Reclaim pending
        hazard_pointers::reclaim_pending();
    }

    #[test]
    fn test_concurrent_safe_atomic_ptr() {
        let data = Box::into_raw(Box::new(2000i32));
        let atomic = Arc::new(hazard_pointers::SafeAtomicPtr::<i32>::new(data));
        let mut handles = vec![];

        // Spawn reader threads
        for _ in 0..5 {
            let atomic_clone = Arc::clone(&atomic);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let guard = atomic_clone.load(std::sync::atomic::Ordering::Acquire);
                    assert!(guard.is_some());
                    thread::sleep(Duration::from_micros(1));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(data);
        }
    }
}

/// Test suite for edge cases
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_max_hazard_pointers() {
        // Create multiple pointers and try to acquire hazard pointers for all
        let ptrs: Vec<*mut i32> = (0..10).map(|i| Box::into_raw(Box::new(i))).collect();

        let mut acquired_count = 0;

        // Try to acquire hazard pointers (max is 4 per thread)
        for ptr in &ptrs {
            let acquired = unsafe { hazard_pointers::acquire_hazard_pointer(*ptr) };
            if acquired {
                acquired_count += 1;
            }
        }

        // Should have acquired at least 4 (max per thread)
        assert!(acquired_count >= 4);

        // Release all
        for ptr in &ptrs {
            unsafe {
                hazard_pointers::release_hazard_pointer(*ptr);
            }
        }

        // Clean up
        for ptr in ptrs {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    }

    #[test]
    fn test_release_null_pointer() {
        // Releasing null should be safe (no-op)
        unsafe {
            hazard_pointers::release_hazard_pointer(std::ptr::null_mut::<i32>());
        }
    }

    #[test]
    fn test_safe_free_null_pointer() {
        // Freeing null should be safe (no-op)
        unsafe {
            hazard_pointers::safe_free(std::ptr::null_mut::<i32>());
        }
    }

    #[test]
    fn test_multiple_reclaims() {
        let data = Box::into_raw(Box::new(3000i32));

        unsafe {
            hazard_pointers::safe_free(data);
        }

        // Multiple reclaims should be safe
        hazard_pointers::reclaim_pending();
        hazard_pointers::reclaim_pending();
        hazard_pointers::reclaim_pending();
    }
}
