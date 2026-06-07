//! Comprehensive coverage tests for lock-free queue and stack
//!
//! This module provides extensive tests for LockFreeQueue and LockFreeStack
//! to increase code coverage, including concurrent operations, edge cases,
//! and error handling.

use f4kvs_core::lockfree::{LockFreeQueue, LockFreeStack};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Test suite for LockFreeStack
#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn test_stack_creation() {
        let stack: LockFreeStack<i32> = LockFreeStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_default() {
        let stack: LockFreeStack<i32> = LockFreeStack::default();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push_pop_single() {
        let stack = LockFreeStack::new();
        stack.push(42);
        assert!(!stack.is_empty());
        assert_eq!(stack.len(), 1);

        let value = stack.pop();
        assert_eq!(value, Some(42));
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push_pop_multiple() {
        let stack = LockFreeStack::new();

        // Push multiple values
        for i in 0..10 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 10);

        // Pop in reverse order (LIFO)
        for i in (0..10).rev() {
            let value = stack.pop();
            assert_eq!(value, Some(i));
        }

        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);

        // Pop from empty stack
        let value = stack.pop();
        assert_eq!(value, None);
    }

    #[test]
    fn test_stack_interleaved_operations() {
        let stack = LockFreeStack::new();

        stack.push(1);
        stack.push(2);
        assert_eq!(stack.pop(), Some(2));
        stack.push(3);
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_stack_concurrent_push() {
        let stack = Arc::new(LockFreeStack::new());
        let mut handles = vec![];

        // Spawn 10 threads, each pushing 100 items
        for _ in 0..10 {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    stack_clone.push(i);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were pushed
        assert_eq!(stack.len(), 1000);
    }

    #[test]
    fn test_stack_concurrent_pop() {
        let stack = Arc::new(LockFreeStack::new());

        // Pre-populate stack
        for i in 0..1000 {
            stack.push(i);
        }

        let mut handles = vec![];
        let popped_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Spawn 10 threads, each popping items
        for _ in 0..10 {
            let stack_clone = Arc::clone(&stack);
            let count_clone = Arc::clone(&popped_count);
            let handle = thread::spawn(move || {
                while let Some(_) = stack_clone.pop() {
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were popped
        assert_eq!(
            popped_count.load(std::sync::atomic::Ordering::Relaxed),
            1000
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_concurrent_push_pop() {
        let stack = Arc::new(LockFreeStack::new());
        let mut handles = vec![];
        let pushed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let popped_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Spawn pusher threads
        for _ in 0..5 {
            let stack_clone = Arc::clone(&stack);
            let count_clone = Arc::clone(&pushed_count);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    stack_clone.push(i);
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        // Spawn popper threads
        for _ in 0..5 {
            let stack_clone = Arc::clone(&stack);
            let count_clone = Arc::clone(&popped_count);
            let handle = thread::spawn(move || {
                let mut popped = 0;
                let mut attempts = 0;
                while popped < 50 && attempts < 10000 {
                    if let Some(_) = stack_clone.pop() {
                        popped += 1;
                        count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        thread::sleep(Duration::from_micros(10));
                    }
                    attempts += 1;
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify operations completed (may have some variation due to concurrency)
        let pushed = pushed_count.load(std::sync::atomic::Ordering::Relaxed);
        let popped = popped_count.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(pushed, 250);
        assert!(popped <= pushed);
    }

    #[test]
    fn test_stack_large_data() {
        let stack = LockFreeStack::new();
        let large_data = vec![0u8; 1024 * 1024]; // 1MB

        stack.push(large_data.clone());
        let retrieved = stack.pop().unwrap();
        assert_eq!(retrieved.len(), large_data.len());
    }

    #[test]
    fn test_stack_string_data() {
        let stack = LockFreeStack::new();
        let test_string = "Hello, World!".to_string();

        stack.push(test_string.clone());
        let retrieved = stack.pop().unwrap();
        assert_eq!(retrieved, test_string);
    }

    #[test]
    fn test_stack_drop_cleanup() {
        let stack = LockFreeStack::new();

        // Push many items
        for i in 0..1000 {
            stack.push(i);
        }

        // Drop should clean up all nodes
        drop(stack);
        // Test passes if no panic/leak occurs
    }

    #[test]
    fn test_stack_empty_after_pop_all() {
        let stack = LockFreeStack::new();

        stack.push(1);
        stack.push(2);
        stack.push(3);

        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_rapid_operations() {
        let stack = LockFreeStack::new();

        // Rapid push/pop
        for _ in 0..1000 {
            stack.push(42);
            assert_eq!(stack.pop(), Some(42));
        }

        assert!(stack.is_empty());
    }
}

/// Test suite for LockFreeQueue
#[cfg(test)]
mod queue_tests {
    use super::*;

    #[test]
    fn test_queue_creation() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_default() {
        let queue: LockFreeQueue<i32> = LockFreeQueue::default();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_enqueue_dequeue_single() {
        let queue = LockFreeQueue::new();
        queue.enqueue(42);
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let value = queue.dequeue();
        assert_eq!(value, Some(42));
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_enqueue_dequeue_multiple() {
        let queue = LockFreeQueue::new();

        // Enqueue multiple values
        for i in 0..10 {
            queue.enqueue(i);
        }

        assert_eq!(queue.len(), 10);

        // Dequeue in order (FIFO)
        for i in 0..10 {
            let value = queue.dequeue();
            assert_eq!(value, Some(i));
        }

        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        // Dequeue from empty queue
        let value = queue.dequeue();
        assert_eq!(value, None);
    }

    #[test]
    fn test_queue_fifo_order() {
        let queue = LockFreeQueue::new();

        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_queue_interleaved_operations() {
        let queue = LockFreeQueue::new();

        queue.enqueue(1);
        queue.enqueue(2);
        assert_eq!(queue.dequeue(), Some(1));
        queue.enqueue(3);
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_queue_concurrent_enqueue() {
        let queue = Arc::new(LockFreeQueue::new());
        let mut handles = vec![];

        // Spawn 10 threads, each enqueueing 100 items
        for _ in 0..10 {
            let queue_clone = Arc::clone(&queue);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    queue_clone.enqueue(i);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were enqueued
        assert_eq!(queue.len(), 1000);
    }

    #[test]
    fn test_queue_concurrent_dequeue() {
        let queue = Arc::new(LockFreeQueue::new());

        // Pre-populate queue
        for i in 0..1000 {
            queue.enqueue(i);
        }

        let mut handles = vec![];
        let dequeued_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Spawn 10 threads, each dequeuing items
        for _ in 0..10 {
            let queue_clone = Arc::clone(&queue);
            let count_clone = Arc::clone(&dequeued_count);
            let handle = thread::spawn(move || {
                while let Some(_) = queue_clone.dequeue() {
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were dequeued
        assert_eq!(
            dequeued_count.load(std::sync::atomic::Ordering::Relaxed),
            1000
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_concurrent_enqueue_dequeue() {
        let queue = Arc::new(LockFreeQueue::new());
        let mut handles = vec![];
        let enqueued_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dequeued_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Spawn enqueuer threads
        for _ in 0..5 {
            let queue_clone = Arc::clone(&queue);
            let count_clone = Arc::clone(&enqueued_count);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    queue_clone.enqueue(i);
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        // Spawn dequeuer threads
        for _ in 0..5 {
            let queue_clone = Arc::clone(&queue);
            let count_clone = Arc::clone(&dequeued_count);
            let handle = thread::spawn(move || {
                let mut dequeued = 0;
                let mut attempts = 0;
                while dequeued < 50 && attempts < 10000 {
                    if let Some(_) = queue_clone.dequeue() {
                        dequeued += 1;
                        count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        thread::sleep(Duration::from_micros(10));
                    }
                    attempts += 1;
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify operations completed (may have some variation due to concurrency)
        let enqueued = enqueued_count.load(std::sync::atomic::Ordering::Relaxed);
        let dequeued = dequeued_count.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(enqueued, 250);
        assert!(dequeued <= enqueued);
    }

    #[test]
    fn test_queue_large_data() {
        let queue = LockFreeQueue::new();
        let large_data = vec![0u8; 1024 * 1024]; // 1MB

        queue.enqueue(large_data.clone());
        let retrieved = queue.dequeue().unwrap();
        assert_eq!(retrieved.len(), large_data.len());
    }

    #[test]
    fn test_queue_string_data() {
        let queue = LockFreeQueue::new();
        let test_string = "Hello, World!".to_string();

        queue.enqueue(test_string.clone());
        let retrieved = queue.dequeue().unwrap();
        assert_eq!(retrieved, test_string);
    }

    #[test]
    fn test_queue_drop_cleanup() {
        let queue = LockFreeQueue::new();

        // Enqueue many items
        for i in 0..1000 {
            queue.enqueue(i);
        }

        // Drop should clean up all nodes including dummy
        drop(queue);
        // Test passes if no panic/leak occurs
    }

    #[test]
    fn test_queue_empty_after_dequeue_all() {
        let queue = LockFreeQueue::new();

        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_rapid_operations() {
        let queue = LockFreeQueue::new();

        // Rapid enqueue/dequeue
        for i in 0..1000 {
            queue.enqueue(i);
            assert_eq!(queue.dequeue(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_maintains_order_under_load() {
        let queue = Arc::new(LockFreeQueue::new());

        // Enqueue items concurrently
        let mut handles = vec![];
        for i in 0..100 {
            let queue_clone = Arc::clone(&queue);
            let handle = thread::spawn(move || {
                queue_clone.enqueue(i);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Dequeue and verify all items are present
        let mut dequeued = Vec::new();
        while let Some(item) = queue.dequeue() {
            dequeued.push(item);
        }

        assert_eq!(dequeued.len(), 100);
        // Verify all items 0-99 are present (order may vary due to concurrency)
        dequeued.sort();
        assert_eq!(dequeued, (0..100).collect::<Vec<_>>());
    }
}
