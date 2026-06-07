//! Concurrency and race condition tests for F4KVS Core
//!
//! These tests verify thread safety, atomicity, and race condition handling
//! under high concurrency scenarios.

use f4kvs_core::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::timeout;

/// Test concurrent writes to the same key
#[tokio::test]
async fn test_concurrent_writes_same_key() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 1000;
    let concurrent_tasks = 10;

    let mut handles = JoinSet::new();

    // Spawn multiple tasks writing to the same key
    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            for i in 0..iterations {
                let key = "concurrent_key";
                let value = Value::String(format!("task_{}_iteration_{}", task_id, i));

                // This should either succeed or fail gracefully, never panic
                let result = engine_clone.put(key, &value).await;
                assert!(result.is_ok() || matches!(result, Err(F4KvsError::Storage { .. })));
            }
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }

    // Verify final state is consistent
    let final_value = engine.get("concurrent_key").await.unwrap();
    assert!(final_value.is_some());
}

/// Test atomicity of batch operations under concurrency
#[tokio::test]
async fn test_batch_operations_atomicity() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let batch_size = 100;
    let concurrent_batches = 5;

    let mut handles = JoinSet::new();

    for batch_id in 0..concurrent_batches {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            let mut batch = Vec::new();

            // Create batch with unique keys per batch
            for i in 0..batch_size {
                let key = format!("batch_{}_key_{}", batch_id, i);
                let value = Value::String(format!("batch_{}_value_{}", batch_id, i));
                batch.push((key, value));
            }

            // Batch should be atomic - either all succeed or all fail
            let result = engine_clone.batch_put(batch).await;
            assert!(result.is_ok());
        });
    }

    // Wait for all batches to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }

    // Verify all keys exist (atomicity check)
    for batch_id in 0..concurrent_batches {
        for i in 0..batch_size {
            let key = format!("batch_{}_key_{}", batch_id, i);
            let value = engine.get(&key).await.unwrap();
            assert!(value.is_some());
        }
    }
}

/// Test lock-free operations under high contention
#[tokio::test]
async fn test_lock_free_operations() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 10000;
    let concurrent_readers = 20;
    let concurrent_writers = 5;

    let counter = Arc::new(AtomicU32::new(0));
    let mut handles = JoinSet::new();

    // Spawn readers
    for _ in 0..concurrent_readers {
        let engine_clone = engine.clone();
        let counter_clone = counter.clone();
        handles.spawn(async move {
            for _ in 0..iterations {
                let key = format!(
                    "read_key_{}",
                    counter_clone.fetch_add(1, Ordering::Relaxed) % 100
                );
                let _ = engine_clone.get(&key).await;
            }
        });
    }

    // Spawn writers
    for _ in 0..concurrent_writers {
        let engine_clone = engine.clone();
        let counter_clone = counter.clone();
        handles.spawn(async move {
            for _ in 0..iterations {
                let key = format!(
                    "write_key_{}",
                    counter_clone.fetch_add(1, Ordering::Relaxed) % 100
                );
                let value =
                    Value::String(format!("value_{}", counter_clone.load(Ordering::Relaxed)));
                let _ = engine_clone.put(&key, &value).await;
            }
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }
}

/// Test race conditions during concurrent get and delete operations
#[tokio::test]
async fn test_get_delete_race_condition() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 1000;
    let concurrent_tasks = 10;

    // Pre-populate with some data
    for i in 0..100 {
        let key = format!("race_key_{}", i);
        let value = Value::String(format!("race_value_{}", i));
        engine.put(&key, &value).await.unwrap();
    }

    let mut handles = JoinSet::new();

    // Spawn tasks that both read and delete
    for _task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            for i in 0..iterations {
                let key = format!("race_key_{}", i % 100);

                // Randomly choose to read or delete
                if i % 2 == 0 {
                    let _ = engine_clone.get(&key).await;
                } else {
                    let _ = engine_clone.delete(&key).await;
                }
            }
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }
}

/// Test memory ordering and visibility in concurrent operations
#[tokio::test]
async fn test_memory_ordering_visibility() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 1000;
    let concurrent_tasks = 5;

    let mut handles = JoinSet::new();

    // Writer task
    let engine_clone = engine.clone();
    handles.spawn(async move {
        for i in 0..iterations {
            let key = "ordering_key";
            let value = Value::String(format!("value_{}", i));
            engine_clone.put(key, &value).await.unwrap();

            // Small delay to allow readers to see intermediate states
            tokio::time::sleep(Duration::from_micros(1)).await;
        }
    });

    // Reader tasks
    for _ in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            for _ in 0..iterations {
                let value = engine_clone.get("ordering_key").await.unwrap();
                // Value should be consistent (not partially written)
                if let Some(Value::String(s)) = value {
                    assert!(s.starts_with("value_"));
                }
            }
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }
}

/// Test timeout handling under high contention
#[tokio::test]
async fn test_timeout_under_contention() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 100;
    let concurrent_tasks = 50;

    let mut handles = JoinSet::new();

    // Spawn many concurrent tasks
    for _ in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            for i in 0..iterations {
                let key = format!("timeout_key_{}", i % 10);
                let value = Value::String(format!("timeout_value_{}", i));

                // Operations should complete within reasonable time
                let result = timeout(Duration::from_secs(1), engine_clone.put(&key, &value)).await;

                assert!(result.is_ok());
            }
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }
}

/// Test deadlock prevention
#[tokio::test]
async fn test_deadlock_prevention() {
    let engine = Arc::new(F4KVSCore::new().unwrap());
    let iterations = 100;
    let concurrent_tasks = 10;

    let mut handles = JoinSet::new();

    // Create tasks that might cause deadlocks
    for task_id in 0..concurrent_tasks {
        let engine_clone = engine.clone();
        handles.spawn(async move {
            for i in 0..iterations {
                let keys = vec![
                    format!("deadlock_key_{}", i % 5),
                    format!("deadlock_key_{}", (i + 1) % 5),
                ];

                // Operations on multiple keys should not deadlock
                for key in keys {
                    let value = Value::String(format!("task_{}_value_{}", task_id, i));
                    let result = engine_clone.put(&key, &value).await;
                    assert!(result.is_ok());
                }
            }
        });
    }

    // Wait for all operations to complete
    while let Some(result) = handles.join_next().await {
        result.unwrap();
    }
}
