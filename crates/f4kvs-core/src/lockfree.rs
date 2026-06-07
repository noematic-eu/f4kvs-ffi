//! Lock-free data structure tests (safe shim only)
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
//!
//! The legacy unsafe implementations have been removed. The public API is served
//! by the compatibility shim in `lib.rs`, which routes calls to the safe
//! concurrency wrappers (DashMap-backed hash map, crossbeam queue, mutex stack).
//! This file keeps a small set of regression tests to ensure the shim remains
//! functional and threadsafe.

#[cfg(test)]
mod tests {
    use crate::lockfree::{LockFreeHashMap, LockFreeHashMapConfig, LockFreeQueue, LockFreeStack};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn hashmap_basic_operations() {
        let map = LockFreeHashMap::new(LockFreeHashMapConfig::default());

        assert_eq!(map.insert("k1".to_string(), "v1".to_string()), None);
        assert_eq!(map.get(&"k1".to_string()), Some("v1".to_string()));

        assert_eq!(
            map.insert("k1".to_string(), "v2".to_string()),
            Some("v1".to_string())
        );
        assert_eq!(map.remove(&"k1".to_string()), Some("v2".to_string()));
        assert_eq!(map.get(&"k1".to_string()), None);
    }

    #[test]
    fn stack_basic_operations() {
        let stack = LockFreeStack::new();

        assert!(stack.is_empty());
        stack.push(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
    }

    #[test]
    fn queue_basic_operations() {
        let queue = LockFreeQueue::new();

        assert!(queue.is_empty());
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_concurrent_producers_consumers() {
        let queue = Arc::new(LockFreeQueue::new());

        // Producers
        let mut producers = vec![];
        for tid in 0..4 {
            let q = Arc::clone(&queue);
            producers.push(thread::spawn(move || {
                for i in 0..250 {
                    q.enqueue(tid * 1000 + i);
                }
            }));
        }
        for handle in producers {
            handle.join().unwrap();
        }

        // Consumers
        let mut consumers = vec![];
        for _ in 0..4 {
            let q = Arc::clone(&queue);
            consumers.push(thread::spawn(move || {
                let mut count = 0;
                loop {
                    if q.dequeue().is_some() {
                        count += 1;
                    } else if q.is_empty() {
                        break;
                    } else {
                        thread::yield_now();
                    }
                }
                count
            }));
        }

        let total: usize = consumers.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 1000);
        assert!(queue.is_empty());
    }
}




