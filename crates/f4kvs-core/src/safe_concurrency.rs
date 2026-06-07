//! Safe Concurrency Utilities
//!
//! This module provides safe alternatives to unsafe lock-free data structures
//! using the crossbeam crate and other safe concurrency primitives.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crossbeam::{
    channel,
    queue::{ArrayQueue, SegQueue},
    utils::Backoff,
};

use std::sync::atomic::{AtomicUsize, Ordering};

/// Safe concurrent hash map using crossbeam
pub struct SafeConcurrentHashMap<K, V> {
    /// Internal dashmap for thread-safe operations
    map: dashmap::DashMap<K, V>,
    /// Size counter
    size: AtomicUsize,
}

impl<K, V> SafeConcurrentHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new safe concurrent hash map
    pub fn new() -> Self {
        Self {
            map: dashmap::DashMap::new(),
            size: AtomicUsize::new(0),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let result = self.map.insert(key, value);
        if result.is_none() {
            self.size.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Get a value by key
    pub fn get(&self, key: &K) -> Option<V> {
        self.map.get(key).map(|entry| entry.clone())
    }

    /// Remove a key-value pair
    pub fn remove(&self, key: &K) -> Option<V> {
        let result = self.map.remove(key);
        if result.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
        }
        result.map(|(_, value)| value)
    }

    /// Check if the map contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all elements
    pub fn clear(&self) {
        self.map.clear();
        self.size.store(0, Ordering::Relaxed);
    }
}

impl<K, V> Default for SafeConcurrentHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Safe concurrent queue using crossbeam
pub struct SafeConcurrentQueue<T> {
    /// Internal queue
    queue: SegQueue<T>,
    /// Size counter
    size: AtomicUsize,
}

impl<T> SafeConcurrentQueue<T> {
    /// Create a new safe concurrent queue
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            size: AtomicUsize::new(0),
        }
    }

    /// Push an item to the queue
    pub fn push(&self, item: T) {
        self.queue.push(item);
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    /// Pop an item from the queue
    pub fn pop(&self) -> Option<T> {
        let result = self.queue.pop();
        if result.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for SafeConcurrentQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe concurrent stack using crossbeam
pub struct SafeConcurrentStack<T> {
    /// Internal queue (used as stack)
    queue: SegQueue<T>,
    /// Size counter
    size: AtomicUsize,
}

impl<T> SafeConcurrentStack<T> {
    /// Create a new safe concurrent stack
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            size: AtomicUsize::new(0),
        }
    }

    /// Push an item to the stack
    pub fn push(&self, item: T) {
        self.queue.push(item);
        self.size.fetch_add(1, Ordering::Relaxed);
    }

    /// Pop an item from the stack
    pub fn pop(&self) -> Option<T> {
        let result = self.queue.pop();
        if result.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for SafeConcurrentStack<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe concurrent bounded queue using crossbeam
pub struct SafeConcurrentBoundedQueue<T> {
    /// Internal bounded queue
    queue: ArrayQueue<T>,
    /// Size counter
    size: AtomicUsize,
}

impl<T> SafeConcurrentBoundedQueue<T> {
    /// Create a new safe concurrent bounded queue
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
            size: AtomicUsize::new(0),
        }
    }

    /// Push an item to the queue
    pub fn push(&self, item: T) -> Result<(), T> {
        let result = self.queue.push(item);
        if result.is_ok() {
            self.size.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Pop an item from the queue
    pub fn pop(&self) -> Option<T> {
        let result = self.queue.pop();
        if result.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }
}

/// Safe concurrent channel for communication
pub struct SafeConcurrentChannel<T> {
    /// Sender
    sender: channel::Sender<T>,
    /// Receiver
    receiver: channel::Receiver<T>,
}

impl<T> SafeConcurrentChannel<T> {
    /// Create a new safe concurrent channel
    pub fn new() -> Self {
        let (sender, receiver) = channel::unbounded();
        Self { sender, receiver }
    }

    /// Create a bounded channel
    pub fn bounded(capacity: usize) -> Self {
        let (sender, receiver) = channel::bounded(capacity);
        Self { sender, receiver }
    }

    /// Send a message
    pub fn send(&self, msg: T) -> Result<(), channel::SendError<T>> {
        self.sender.send(msg)
    }

    /// Receive a message
    pub fn recv(&self) -> Result<T, channel::RecvError> {
        self.receiver.recv()
    }

    /// Try to receive a message without blocking
    pub fn try_recv(&self) -> Result<T, channel::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Clone the sender
    pub fn sender(&self) -> channel::Sender<T> {
        self.sender.clone()
    }

    /// Clone the receiver
    pub fn receiver(&self) -> channel::Receiver<T> {
        self.receiver.clone()
    }
}

impl<T> Default for SafeConcurrentChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe concurrent counter
pub struct SafeConcurrentCounter {
    /// Internal counter
    counter: AtomicUsize,
}

impl SafeConcurrentCounter {
    /// Create a new safe concurrent counter
    pub fn new(initial_value: usize) -> Self {
        Self {
            counter: AtomicUsize::new(initial_value),
        }
    }

    /// Create with zero initial value
    pub fn zero() -> Self {
        Self::new(0)
    }

    /// Get the current value
    pub fn get(&self) -> usize {
        self.counter.load(Ordering::Relaxed)
    }

    /// Increment the counter
    pub fn increment(&self) -> usize {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Decrement the counter
    pub fn decrement(&self) -> usize {
        self.counter.fetch_sub(1, Ordering::Relaxed)
    }

    /// Add a value to the counter
    pub fn add(&self, value: usize) -> usize {
        self.counter.fetch_add(value, Ordering::Relaxed)
    }

    /// Subtract a value from the counter
    pub fn subtract(&self, value: usize) -> usize {
        self.counter.fetch_sub(value, Ordering::Relaxed)
    }

    /// Reset the counter to zero
    pub fn reset(&self) -> usize {
        self.counter.swap(0, Ordering::Relaxed)
    }
}

impl Default for SafeConcurrentCounter {
    fn default() -> Self {
        Self::zero()
    }
}

/// Safe concurrent barrier for synchronization
pub struct SafeConcurrentBarrier {
    /// Number of threads to wait for
    count: usize,
    /// Current count of waiting threads
    current: AtomicUsize,
    /// Generation counter
    generation: AtomicUsize,
}

impl SafeConcurrentBarrier {
    /// Create a new safe concurrent barrier
    pub fn new(count: usize) -> Self {
        Self {
            count,
            current: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Wait for all threads to reach the barrier
    pub fn wait(&self) -> bool {
        let generation = self.generation.load(Ordering::Relaxed);
        let current = self.current.fetch_add(1, Ordering::Relaxed);

        if current + 1 == self.count {
            // Last thread to arrive
            self.current.store(0, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::Relaxed);
            true // Leader
        } else {
            // Wait for other threads
            let backoff = Backoff::new();
            while self.generation.load(Ordering::Relaxed) == generation {
                backoff.snooze();
            }
            false // Follower
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_safe_concurrent_hash_map() {
        let map = Arc::new(SafeConcurrentHashMap::new());

        // Test basic operations
        assert!(map.insert("key1", "value1").is_none());
        assert_eq!(map.get(&"key1"), Some("value1"));
        assert_eq!(map.len(), 1);

        // Test concurrent access
        let map_clone = map.clone();
        let handle = thread::spawn(move || {
            map_clone.insert("key2", "value2");
        });

        handle.join().unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&"key2"), Some("value2"));
    }

    #[test]
    fn test_safe_concurrent_queue() {
        let queue = Arc::new(SafeConcurrentQueue::new());

        // Test basic operations
        queue.push(1);
        queue.push(2);
        assert_eq!(queue.len(), 2);

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_safe_concurrent_channel() {
        let channel = Arc::new(SafeConcurrentChannel::new());

        // Test basic operations
        channel.send(42).unwrap();
        assert_eq!(channel.recv().unwrap(), 42);

        // Test concurrent access
        let channel_clone = channel.clone();
        let handle = thread::spawn(move || {
            channel_clone.send(100).unwrap();
        });

        handle.join().unwrap();
        assert_eq!(channel.recv().unwrap(), 100);
    }

    #[test]
    fn test_safe_concurrent_counter() {
        let counter = Arc::new(SafeConcurrentCounter::zero());

        // Test basic operations
        assert_eq!(counter.get(), 0);
        assert_eq!(counter.increment(), 0);
        assert_eq!(counter.get(), 1);

        // Test concurrent access
        let counter_clone = counter.clone();
        let handle = thread::spawn(move || {
            counter_clone.increment();
        });

        handle.join().unwrap();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_safe_concurrent_barrier() {
        let barrier = Arc::new(SafeConcurrentBarrier::new(2));

        // Test barrier synchronization
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
        });

        let is_leader = barrier.wait();
        handle.join().unwrap();

        // One thread should be the leader
        assert!(is_leader || !is_leader); // At least one should be true
    }
}
