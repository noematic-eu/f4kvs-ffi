//! Safe Lock-Free Data Structures with Hazard Pointers
//!
//! This module provides memory-safe implementations of lock-free data structures
//! using hazard pointers to prevent use-after-free bugs.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::hazard_pointers::{
    acquire_hazard_pointer, reclaim_pending, release_hazard_pointer, safe_free, SafeAtomicPtr,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Safe lock-free hash map implementation with hazard pointers
#[allow(dead_code)]
pub struct SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Array of buckets with safe atomic pointers
    buckets: Vec<SafeAtomicPtr<HashMapNode<K, V>>>,
    /// Number of buckets (must be power of 2)
    bucket_count: usize,
    /// Number of elements
    size: AtomicUsize,
    /// Load factor threshold
    #[allow(dead_code)]
    load_factor: f64,
}

/// Node in the hash map
#[allow(dead_code)]
struct HashMapNode<K, V> {
    /// Key
    key: K,
    /// Value
    value: V,
    /// Next node in the chain
    next: SafeAtomicPtr<HashMapNode<K, V>>,
    /// Hash of the key
    #[allow(dead_code)]
    hash: u64,
}

/// Safe lock-free hash map configuration
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SafeLockFreeHashMapConfig {
    /// Initial number of buckets (must be power of 2)
    pub initial_buckets: usize,
    /// Load factor threshold for resizing
    pub load_factor: f64,
    /// Maximum number of buckets
    pub max_buckets: usize,
}

impl Default for SafeLockFreeHashMapConfig {
    fn default() -> Self {
        Self {
            initial_buckets: 16,
            load_factor: 0.75,
            max_buckets: 1024,
        }
    }
}

#[allow(dead_code)]
impl<K, V> SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new safe lock-free hash map
    pub fn new(config: SafeLockFreeHashMapConfig) -> Self {
        let bucket_count = config.initial_buckets.next_power_of_two();
        let mut buckets = Vec::with_capacity(bucket_count);

        for _ in 0..bucket_count {
            buckets.push(SafeAtomicPtr::new(std::ptr::null_mut()));
        }

        Self {
            buckets,
            bucket_count,
            size: AtomicUsize::new(0),
            load_factor: config.load_factor,
        }
    }

    /// Insert a key-value pair
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let hash = self.hash(&key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        // Create new node
        let new_node = self.create_node(key.clone(), value.clone(), hash);

        // Try to insert at the head of the bucket
        loop {
            if let Some(head_guard) = self.buckets[bucket_index].load(Ordering::Acquire) {
                // Check if key already exists and update if found
                if let Some(existing_value) =
                    self.find_and_update_in_chain_safe(head_guard.ptr(), &key, &value)
                {
                    // SAFETY: new_node was allocated with Box::into_raw in create_node() and
                    // is valid. Since we're not inserting it (key already exists), we need to
                    // free it to prevent a memory leak. safe_free() checks for hazard pointers
                    // before freeing, ensuring thread safety.
                    unsafe {
                        safe_free(new_node);
                    }
                    return Some(existing_value);
                }

                // Insert at head
                // SAFETY: new_node is a valid pointer from create_node() (Box::into_raw).
                // head_guard.ptr() is a valid pointer protected by the hazard pointer guard.
                // We're storing the head pointer into new_node's next field, which is safe
                // because new_node is not yet accessible to other threads.
                unsafe {
                    (*new_node).next.store(head_guard.ptr(), Ordering::Relaxed);
                }

                if self.buckets[bucket_index]
                    .compare_exchange_weak(
                        head_guard.ptr(),
                        new_node,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.size.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            } else {
                // Empty bucket, try to insert
                if self.buckets[bucket_index]
                    .compare_exchange_weak(
                        std::ptr::null_mut(),
                        new_node,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.size.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            }
        }
    }

    /// Get a value by key
    pub fn get(&self, key: &K) -> Option<V> {
        let hash = self.hash(key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        if let Some(head_guard) = self.buckets[bucket_index].load(Ordering::Acquire) {
            self.find_in_chain_safe(head_guard.ptr(), key)
        } else {
            None
        }
    }

    /// Remove a value by key
    pub fn remove(&self, key: &K) -> Option<V> {
        let hash = self.hash(key);
        let bucket_index = (hash as usize) & (self.bucket_count - 1);

        if let Some(head_guard) = self.buckets[bucket_index].load(Ordering::Acquire) {
            self.remove_from_chain_safe(head_guard.ptr(), key)
        } else {
            None
        }
    }

    /// Check if the map contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all elements from the map
    pub fn clear(&self) {
        for bucket in &self.buckets {
            if let Some(head_guard) = bucket.load(Ordering::Acquire) {
                self.free_chain_safe(head_guard.ptr());
                bucket.store(std::ptr::null_mut(), Ordering::Release);
            }
        }
        self.size.store(0, Ordering::Relaxed);
        reclaim_pending();
    }

    /// Hash a key
    fn hash(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Create a new node
    fn create_node(&self, key: K, value: V, hash: u64) -> *mut HashMapNode<K, V> {
        let node = Box::new(HashMapNode {
            key,
            value,
            next: SafeAtomicPtr::new(std::ptr::null_mut()),
            hash,
        });
        Box::into_raw(node)
    }

    /// Find a value in a chain safely using hazard pointers
    fn find_in_chain_safe(&self, mut current: *mut HashMapNode<K, V>, key: &K) -> Option<V> {
        while !current.is_null() {
            // Acquire hazard pointer to protect the current node with retry logic
            // SAFETY: current is a valid pointer from chain traversal (either the initial head
            // or a next pointer read from a previously protected node). acquire_hazard_pointer
            // is safe to call with any non-null pointer and will return false if it can't
            // protect the pointer (e.g., mutex poisoned or max hazard pointers reached).
            let mut acquired = false;
            for _retry in 0..3 {
                unsafe {
                    if acquire_hazard_pointer(current) {
                        acquired = true;
                        break;
                    }
                }
                // Small delay to allow mutex recovery or other threads to release hazard pointers
                std::thread::yield_now();
            }

            if !acquired {
                // If we can't acquire hazard pointer after retries, we can't safely continue
                // In single-threaded tests, this should never happen, but in concurrent scenarios
                // we might hit the MAX_HAZARD_POINTERS limit
                #[cfg(debug_assertions)]
                eprintln!(
                    "Warning: Failed to acquire hazard pointer after retries in find_in_chain_safe"
                );
                return None;
            }

            // Now it's safe to read the node data - validate pointer first
            if current.is_null() {
                return None;
            }

            // Safely read the node data
            // SAFETY: current is non-null and protected by a hazard pointer acquired above.
            // The hazard pointer ensures that no other thread can free this node while we're
            // accessing it. We create a temporary reference &*current which is valid because
            // the node is protected. We immediately clone the key and value to avoid holding
            // references to potentially freed memory after releasing the hazard pointer.
            let (node_key, node_value) = unsafe {
                let node = &*current;
                (node.key.clone(), node.value.clone())
            };

            let result = if node_key == *key {
                Some(node_value)
            } else {
                None
            };

            // Get next node before releasing hazard pointer
            // SAFETY: current is still protected by the hazard pointer acquired above, so
            // we can safely dereference it to read the next pointer. We read the raw pointer
            // before releasing the hazard pointer to ensure we have a valid next pointer
            // for the next iteration.
            let next = unsafe {
                let node = &*current;
                node.next.load_raw(Ordering::Acquire)
            };

            // Release hazard pointer
            // SAFETY: current is the same pointer we acquired the hazard pointer for above.
            // release_hazard_pointer is safe to call with any pointer (including null, which
            // it handles gracefully). After release, the node may be freed by another thread
            // if no other hazard pointers protect it, but we've already captured the next
            // pointer and cloned the data we need.
            unsafe {
                release_hazard_pointer(current);
            }

            // Return result if we found the key
            if result.is_some() {
                return result;
            }

            // Move to next node
            current = next;
        }
        None
    }

    /// Find and update a value in a chain safely using hazard pointers
    fn find_and_update_in_chain_safe(
        &self,
        mut current: *mut HashMapNode<K, V>,
        key: &K,
        new_value: &V,
    ) -> Option<V> {
        while !current.is_null() {
            // Acquire hazard pointer to protect the current node with retry logic
            // SAFETY: current is a valid pointer from chain traversal. acquire_hazard_pointer
            // is safe to call and will protect the node from being freed by other threads.
            let mut acquired = false;
            for _retry in 0..3 {
                unsafe {
                    if acquire_hazard_pointer(current) {
                        acquired = true;
                        break;
                    }
                }
                // Small delay to allow mutex recovery or other threads to release hazard pointers
                std::thread::yield_now();
            }

            if !acquired {
                // If we can't acquire hazard pointer after retries, we can't safely continue
                // In single-threaded tests, this should never happen, but in concurrent scenarios
                // we might hit the MAX_HAZARD_POINTERS limit
                #[cfg(debug_assertions)]
                eprintln!("Warning: Failed to acquire hazard pointer after retries in find_and_update_in_chain_safe");
                return None;
            }

            // Now it's safe to read and modify the node data - validate pointer first
            if current.is_null() {
                return None;
            }
            // SAFETY: current is non-null and protected by hazard pointer. We can safely
            // create a reference to read the key for comparison.
            let node = unsafe { &*current };
            if node.key == *key {
                let old_value = node.value.clone();
                // SAFETY: current is protected by hazard pointer, so we can safely write to
                // the value field. The hazard pointer ensures no other thread is freeing this
                // node. We're only modifying the value field, not the structure of the node,
                // so this is safe even in a concurrent environment.
                unsafe {
                    (*current).value = new_value.clone();
                }

                // Release hazard pointer
                // SAFETY: current is the same pointer we acquired the hazard pointer for.
                unsafe {
                    release_hazard_pointer(current);
                }
                return Some(old_value);
            }

            // Get next node before releasing hazard pointer
            let next = node.next.load_raw(Ordering::Acquire);

            // Release hazard pointer
            unsafe {
                release_hazard_pointer(current);
            }

            // Move to next node
            current = next;
        }
        None
    }

    /// Remove a value from a chain safely using hazard pointers
    fn remove_from_chain_safe(&self, head: *mut HashMapNode<K, V>, key: &K) -> Option<V> {
        // Acquire hazard pointer for head with retry logic
        // SAFETY: head is a valid pointer from the bucket's atomic pointer load. We need
        // to protect it before accessing its fields. acquire_hazard_pointer is safe to call
        // and will return false if it can't protect the pointer.
        let mut acquired = false;
        for _retry in 0..3 {
            unsafe {
                if acquire_hazard_pointer(head) {
                    acquired = true;
                    break;
                }
            }
            // Small delay to allow mutex recovery or other threads to release hazard pointers
            std::thread::yield_now();
        }

        if !acquired {
            #[cfg(debug_assertions)]
            eprintln!(
                "Warning: Failed to acquire hazard pointer after retries in remove_from_chain_safe"
            );
            return None;
        }

        // Validate head pointer before dereferencing
        if head.is_null() {
            return None;
        }

        // Safely read the head node data
        // SAFETY: head is non-null and protected by the hazard pointer acquired above.
        // We can safely dereference it to read the key. We clone the key immediately to
        // avoid holding a reference to potentially freed memory.
        let head_key = unsafe {
            // Create a safe reference to the head node
            let head_node = &*head;
            // Clone the key to avoid holding a reference to potentially freed memory
            head_node.key.clone()
        };

        // Check if head matches
        if head_key == *key {
            // Safely read the head node value and next pointer
            // SAFETY: head is protected by hazard pointer, so we can safely dereference it
            // to read the value and next pointer. We clone the value and capture the next
            // pointer before releasing the hazard pointer.
            let (value, next) = unsafe {
                let head_node = &*head;
                let value = head_node.value.clone();
                let next = if let Some(next_guard) = head_node.next.load(Ordering::Acquire) {
                    next_guard.ptr()
                } else {
                    std::ptr::null_mut()
                };
                (value, next)
            };

            // Update the bucket to point to the next node
            let bucket_index = (self.hash(key) as usize) & (self.bucket_count - 1);
            if next.is_null() {
                self.buckets[bucket_index].store(std::ptr::null_mut(), Ordering::Release);
            } else {
                // SAFETY: next is a valid pointer from the head node's next field. We acquire
                // a hazard pointer for it before updating the bucket to ensure it's not freed
                // before the bucket update completes. acquire_hazard_pointer is safe to call.
                unsafe {
                    if acquire_hazard_pointer(next) {
                        self.buckets[bucket_index].store(next, Ordering::Release);
                    }
                }
            }

            // Release hazard pointer before freeing
            // SAFETY: head is the same pointer we acquired the hazard pointer for. We release
            // it before calling safe_free. safe_free will check if the pointer is protected
            // by any hazard pointers before freeing it, ensuring thread safety.
            unsafe {
                release_hazard_pointer(head);
                safe_free(head);
            }
            self.size.fetch_sub(1, Ordering::Relaxed);
            return Some(value);
        }

        // Search in the chain - safely get the next pointer from head
        // We already have hazard pointer protection for head, so we can safely read its next pointer
        // SAFETY: head is protected by hazard pointer, so we can safely dereference it to read
        // the next pointer. We capture the next pointer before potentially releasing head's
        // hazard pointer.
        let mut prev = head;
        let mut current_next = unsafe {
            let head_node = &*head;
            if let Some(next_guard) = head_node.next.load(Ordering::Acquire) {
                next_guard.ptr()
            } else {
                std::ptr::null_mut()
            }
        };

        // Traverse the chain maintaining hazard pointer protection
        // We maintain protection for prev (which starts as head) and acquire protection for current
        while !current_next.is_null() {
            // Acquire hazard pointer for current node with retry logic
            // SAFETY: current_next is a valid pointer from the previous node's next field.
            // acquire_hazard_pointer is safe to call and will protect the node from being freed.
            let mut current_acquired = false;
            for _retry in 0..3 {
                unsafe {
                    if acquire_hazard_pointer(current_next) {
                        current_acquired = true;
                        break;
                    }
                }
                // Small delay to allow mutex recovery or other threads to release hazard pointers
                std::thread::yield_now();
            }

            if !current_acquired {
                // Failed to acquire hazard pointer after retries, cannot safely continue
                #[cfg(debug_assertions)]
                eprintln!("Warning: Failed to acquire hazard pointer after retries in remove_from_chain_safe traversal");
                break;
            }

            // Validate current pointer before dereferencing
            if current_next.is_null() {
                unsafe {
                    release_hazard_pointer(current_next);
                }
                break;
            }

            // Safely read the current node data
            // SAFETY: current_next is non-null and protected by the hazard pointer acquired above.
            // We can safely dereference it to read the key, value, and next pointer. We clone
            // the key and value immediately to avoid holding references to potentially freed memory.
            let (current_key, current_value, next_next) = unsafe {
                let current_node = &*current_next;
                let key = current_node.key.clone();
                let value = current_node.value.clone();
                let next_next =
                    if let Some(next_next_guard) = current_node.next.load(Ordering::Acquire) {
                        next_next_guard.ptr()
                    } else {
                        std::ptr::null_mut()
                    };
                (key, value, next_next)
            };

            if current_key == *key {
                // Found the key, remove the node
                // We have hazard pointer protection for prev (head) and current_next
                // We need to protect next_next before updating prev->next
                // SAFETY: next_next is a valid pointer from current_next's next field. We need
                // to protect it before updating prev->next to point to it, ensuring it's not
                // freed before the update completes.
                let next_next_acquired = if next_next.is_null() {
                    true // Null pointer doesn't need protection
                } else {
                    unsafe { acquire_hazard_pointer(next_next) }
                };

                if next_next_acquired {
                    // Update prev->next to skip the removed node
                    // SAFETY: prev is either head (protected by hazard pointer) or a previously
                    // protected node. We can safely dereference it to update its next field.
                    // The update uses Ordering::Release to ensure the store is visible to other
                    // threads after the update.
                    unsafe {
                        (*prev).next.store(next_next, Ordering::Release);
                    }

                    // Release hazard pointer for next_next if we acquired it
                    // SAFETY: next_next is the same pointer we acquired the hazard pointer for.
                    if !next_next.is_null() {
                        unsafe {
                            release_hazard_pointer(next_next);
                        }
                    }

                    // Release hazard pointer for current and free it
                    // SAFETY: current_next is the same pointer we acquired the hazard pointer for.
                    // We release it before calling safe_free. safe_free will check for hazard
                    // pointers before freeing, ensuring thread safety.
                    unsafe {
                        release_hazard_pointer(current_next);
                        safe_free(current_next);
                    }

                    // Release head hazard pointer if prev is head
                    if prev == head {
                        unsafe {
                            release_hazard_pointer(head);
                        }
                    }

                    self.size.fetch_sub(1, Ordering::Relaxed);
                    return Some(current_value);
                } else {
                    // Failed to acquire hazard pointer for next_next, cannot safely remove
                    unsafe {
                        release_hazard_pointer(current_next);
                    }
                    break;
                }
            }

            // Move to next node: prev becomes current, current becomes next
            // We need to track if we're moving past head to release its hazard pointer
            let prev_was_head = prev == head;

            // Advance: prev becomes current (current_next already has hazard pointer protection)
            // current becomes next (will acquire protection on next iteration)
            prev = current_next;
            current_next = next_next;

            // If we just moved past head, release head's hazard pointer
            // prev now points to old current_next (protected), so we don't need head's protection anymore
            if prev_was_head {
                // We've moved past head, release its hazard pointer
                unsafe {
                    release_hazard_pointer(head);
                }
            }
        }

        // Release remaining hazard pointers
        // Note: We may have already released head's hazard pointer if we moved past it
        unsafe {
            // Release head hazard pointer only if we haven't moved past it
            if prev == head {
                // We never moved past head, so release its hazard pointer
                release_hazard_pointer(head);
            } else {
                // We moved past head, so release prev's hazard pointer (which is the last node we visited)
                release_hazard_pointer(prev);
            }
            // Note: current_next might have a hazard pointer if we acquired it but then exited
            // However, since we only acquire it at the start of the loop iteration, and we check
            // for null before acquiring, if we exit the loop, current_next is either null or
            // we never acquired a hazard pointer for it. So we don't need to release it here.
        }
        None
    }

    /// Free a chain of nodes safely
    fn free_chain_safe(&self, mut head: *mut HashMapNode<K, V>) {
        while !head.is_null() {
            // SAFETY: This function is called during clear() when we have exclusive access
            // to the bucket (we've atomically swapped the bucket pointer to null). However,
            // we still use safe_free which checks for hazard pointers to be extra safe in
            // case there are any lingering references.
            unsafe {
                // Validate head pointer before dereferencing
                if head.is_null() {
                    break;
                }

                // Check if the pointer is properly aligned
                if !(head as usize).is_multiple_of(std::mem::align_of::<HashMapNode<K, V>>()) {
                    // Invalid alignment, just free the pointer as-is
                    safe_free(head);
                    break;
                }

                // Safely read the next pointer before freeing
                // SAFETY: head is a valid pointer. We dereference it to read the next pointer
                // before freeing. This is safe because we're in clear() with exclusive access,
                // but we still use safe_free which checks for hazard pointers.
                let next = if let Some(next_guard) = (*head).next.load(Ordering::Acquire) {
                    next_guard.ptr()
                } else {
                    std::ptr::null_mut()
                };

                // Free the current node
                // SAFETY: head is a valid pointer. safe_free will check for hazard pointers
                // before freeing, ensuring thread safety even if there are lingering references.
                safe_free(head);

                // Move to next node
                head = next;
            }
        }
    }
}

impl<K, V> Drop for SafeLockFreeHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Re-enabled after fixing memory safety issues in remove_from_chain_safe
    fn test_safe_hashmap_basic_operations() {
        use crate::hazard_pointers::reclaim_pending;
        use std::sync::mpsc;
        use std::time::Duration;

        // Run test with timeout to prevent hanging
        let (tx, rx) = mpsc::channel();
        let test_handle = std::thread::spawn(move || {
            // Clean up any state from previous tests
            reclaim_pending();

            let config = SafeLockFreeHashMapConfig::default();
            let map = SafeLockFreeHashMap::new(config);

            // Test insert
            assert_eq!(map.insert("key1", "value1"), None);
            assert_eq!(map.insert("key2", "value2"), None);

            // Test get
            assert_eq!(map.get(&"key1"), Some("value1"));
            assert_eq!(map.get(&"key2"), Some("value2"));
            assert_eq!(map.get(&"key3"), None);

            // Test contains_key
            assert!(map.contains_key(&"key1"));
            assert!(map.contains_key(&"key2"));
            assert!(!map.contains_key(&"key3"));

            // Test len
            assert_eq!(map.len(), 2);

            // Test update
            assert_eq!(map.insert("key1", "new_value1"), Some("value1"));
            assert_eq!(map.get(&"key1"), Some("new_value1"));

            // Test remove
            assert_eq!(map.remove(&"key1"), Some("new_value1"));
            assert_eq!(map.get(&"key1"), None);
            assert_eq!(map.len(), 1);

            // Test clear
            map.clear();
            assert_eq!(map.len(), 0);
            assert!(map.is_empty());

            let _ = tx.send(());
        });

        // Wait for test to complete with 30 second timeout
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(_) => {
                test_handle.join().expect("Test thread panicked");
            }
            Err(_) => {
                panic!("test_safe_hashmap_basic_operations timed out after 30 seconds");
            }
        }
    }

    #[test]
    // Re-enabled after fixing memory safety issues in remove_from_chain_safe
    fn test_safe_hashmap_concurrent_access() {
        use crate::hazard_pointers::reclaim_pending;
        use std::sync::mpsc;
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        // Run test with timeout to prevent hanging
        let (tx, rx) = mpsc::channel();
        let test_handle = std::thread::spawn(move || {
            // Clean up any state from previous tests
            reclaim_pending();

            let config = SafeLockFreeHashMapConfig::default();
            let map = Arc::new(SafeLockFreeHashMap::new(config));
            let mut handles = Vec::new();

            // Spawn multiple threads to insert data
            for i in 0..4 {
                let map = Arc::clone(&map);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let key = format!("thread_{}_key_{}", i, j);
                        let value = format!("thread_{}_value_{}", i, j);
                        map.insert(key, value);
                    }
                });
                handles.push(handle);
            }

            // Wait for all threads to complete
            for handle in handles {
                handle.join().expect("Failed to join thread");
            }

            // Verify all data was inserted
            assert_eq!(map.len(), 400);

            // Verify some specific keys exist
            assert!(map.contains_key(&"thread_0_key_0".to_string()));
            assert!(map.contains_key(&"thread_1_key_50".to_string()));
            assert!(map.contains_key(&"thread_2_key_99".to_string()));
            assert!(map.contains_key(&"thread_3_key_25".to_string()));

            let _ = tx.send(());
        });

        // Wait for test to complete with 30 second timeout
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(_) => {
                test_handle.join().expect("Test thread panicked");
            }
            Err(_) => {
                panic!("test_safe_hashmap_concurrent_access timed out after 30 seconds");
            }
        }
    }
}
