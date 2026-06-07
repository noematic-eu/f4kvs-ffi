//! Comprehensive cache tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for cache scenarios including:
//! - Cache performance under load
//! - Eviction strategy validation
//! - Memory pressure scenarios
//! - Cache warming strategies

use f4kvs_core::cache::{CacheConfig, LruCache};
use f4kvs_core::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_cache_performance_under_load() {
    let cache = Arc::new(LruCache::new());

    // Spawn many concurrent operations
    let mut handles = Vec::new();
    for i in 0..100 {
        let cache_clone = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for j in 0..50 {
                let key = format!("key_{}_{}", i, j);
                let value = Value::String(format!("value_{}_{}", i, j));
                cache_clone.put(key.clone(), value.clone()).await;
                let _ = cache_clone.get(&key).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify cache state
    let stats = cache.stats().await;
    assert!(stats.entries > 0);
    assert!(stats.hits > 0);
}

#[tokio::test]
async fn test_eviction_strategy_validation() {
    let config = CacheConfig {
        max_entries: 5,
        max_age: Duration::from_secs(60),
        max_idle_time: Duration::from_secs(30),
        auto_cleanup: false,
        cleanup_interval: Duration::from_secs(30),
    };
    let cache = LruCache::with_config(config);

    // Fill cache
    for i in 0..5 {
        cache
            .put(format!("key_{}", i), Value::String(format!("value_{}", i)))
            .await;
    }

    // Access keys in specific order to test LRU
    let _ = cache.get("key_0").await; // Most recently used
    let _ = cache.get("key_1").await; // Second most recently used
                                      // key_2, key_3, key_4 are least recently used

    // Add new key, should evict key_2 (oldest of unused)
    cache
        .put("key_5".to_string(), Value::String("value_5".to_string()))
        .await;

    // key_2 should be evicted
    assert_eq!(cache.get("key_2").await, None);
    // Others should still be present
    assert!(cache.get("key_0").await.is_some());
    assert!(cache.get("key_1").await.is_some());
    assert!(cache.get("key_3").await.is_some());
    assert!(cache.get("key_4").await.is_some());
    assert!(cache.get("key_5").await.is_some());
}

#[tokio::test]
async fn test_memory_pressure_scenarios() {
    let config = CacheConfig {
        max_entries: 10,
        max_age: Duration::from_secs(60),
        max_idle_time: Duration::from_secs(30),
        auto_cleanup: false,
        cleanup_interval: Duration::from_secs(30),
    };
    let cache = LruCache::with_config(config);

    // Add many entries to trigger evictions
    for i in 0..100 {
        cache
            .put(
                format!("key_{}", i),
                Value::String("x".repeat(1000)), // Large values
            )
            .await;
    }

    // Cache should not exceed max_entries
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 10);
    assert!(stats.evictions > 0);
}

#[tokio::test]
async fn test_cache_warming_strategies() {
    let cache = LruCache::new();

    // Warm cache with frequently accessed keys
    let warm_keys = vec!["hot_key1", "hot_key2", "hot_key3"];
    for key in &warm_keys {
        cache
            .put(
                key.to_string(),
                Value::String(format!("warm_value_{}", key)),
            )
            .await;
    }

    // Access warm keys multiple times
    for _ in 0..10 {
        for key in &warm_keys {
            let _ = cache.get(key).await;
        }
    }

    // Verify warm keys have high hit rate
    let stats = cache.stats().await;
    assert!(stats.hits >= 30); // 10 iterations * 3 keys
    assert!(stats.hit_rate() > 0.9); // High hit rate
}

#[tokio::test]
async fn test_ttl_expiration_cleanup() {
    let config = CacheConfig {
        max_entries: 100,
        max_age: Duration::from_millis(1000),
        max_idle_time: Duration::from_secs(60),
        auto_cleanup: false,
        cleanup_interval: Duration::from_millis(100),
    };
    let cache = LruCache::with_config(config);

    // Add entries with different ages
    cache
        .put("key1".to_string(), Value::String("value1".to_string()))
        .await;
    sleep(Duration::from_millis(50)).await;
    cache
        .put("key2".to_string(), Value::String("value2".to_string()))
        .await;

    // Wait for key1 to expire (key2 should still be valid since it was added later)
    sleep(Duration::from_millis(950)).await;

    // Cleanup should remove expired entries
    cache.cleanup().await;

    // key1 should be gone, key2 should still be there
    assert_eq!(cache.get("key1").await, None);
    assert_eq!(
        cache.get("key2").await,
        Some(Value::String("value2".to_string()))
    );
}

#[tokio::test]
async fn test_concurrent_eviction() {
    let config = CacheConfig {
        max_entries: 5,
        max_age: Duration::from_secs(60),
        max_idle_time: Duration::from_secs(30),
        auto_cleanup: false,
        cleanup_interval: Duration::from_secs(30),
    };
    let cache = Arc::new(LruCache::with_config(config));

    // Spawn concurrent operations that will trigger evictions
    let mut handles = Vec::new();
    for i in 0..20 {
        let cache_clone = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            cache_clone
                .put(format!("key_{}", i), Value::String(format!("value_{}", i)))
                .await;
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap();
    }

    // Cache should not exceed max_entries
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 5);
    assert!(stats.evictions > 0);
}

#[tokio::test]
async fn test_cache_hit_rate_optimization() {
    let cache = LruCache::new();

    // Add entries
    for i in 0..10 {
        cache
            .put(format!("key_{}", i), Value::String(format!("value_{}", i)))
            .await;
    }

    // Access same keys repeatedly (should increase hit rate)
    for _ in 0..100 {
        for i in 0..10 {
            let _ = cache.get(&format!("key_{}", i)).await;
        }
    }

    let stats = cache.stats().await;
    assert!(stats.hit_rate() > 0.9); // Very high hit rate
    assert!(stats.hits > 900); // Most accesses should be hits
}

#[tokio::test]
async fn test_cache_memory_usage_under_pressure() {
    let config = CacheConfig {
        max_entries: 20,
        max_age: Duration::from_secs(60),
        max_idle_time: Duration::from_secs(30),
        auto_cleanup: false,
        cleanup_interval: Duration::from_secs(30),
    };
    let cache = LruCache::with_config(config);

    // Add entries with varying sizes
    for i in 0..50 {
        let value_size = if i % 2 == 0 { 100 } else { 1000 };
        cache
            .put(format!("key_{}", i), Value::String("x".repeat(value_size)))
            .await;
    }

    // Memory usage should be bounded by evictions
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 20);
    assert!(stats.memory_usage > 0);
}

#[tokio::test]
async fn test_cache_entry_access_count() {
    let cache = LruCache::new();

    cache
        .put("key1".to_string(), Value::String("value1".to_string()))
        .await;

    // Access multiple times
    for _ in 0..10 {
        let _ = cache.get("key1").await;
    }

    // Entry should still be accessible
    assert_eq!(
        cache.get("key1").await,
        Some(Value::String("value1".to_string()))
    );

    let stats = cache.stats().await;
    assert!(stats.hits >= 10);
}

#[tokio::test]
async fn test_cache_with_cached_storage_engine() {
    use f4kvs_core::cache::CachedStorageEngine;
    use f4kvs_core::hashmap::HashMapStorage;
    use f4kvs_core::storage_traits::StorageEngine;
    use std::sync::Arc;

    let storage = Arc::new(HashMapStorage::new());
    let cached = CachedStorageEngine::new(storage);

    // Put value
    cached
        .put("key1", &Value::String("value1".to_string()))
        .await
        .unwrap();

    // First get (cache miss, then cache)
    let result1 = cached.get("key1").await.unwrap();
    assert_eq!(result1, Some(Value::String("value1".to_string())));

    // Second get (cache hit)
    let result2 = cached.get("key1").await.unwrap();
    assert_eq!(result2, Some(Value::String("value1".to_string())));

    // Verify cache stats
    let cache_stats = cached.cache_stats().await;
    assert!(cache_stats.hits > 0);
}
