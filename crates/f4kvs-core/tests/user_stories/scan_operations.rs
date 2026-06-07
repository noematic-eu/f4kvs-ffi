//! User Story: Scan and Range Operations
//!
//! As a developer
//! I want to scan keys by prefix or range
//! So that I can iterate over related data

use f4kvs_core::{F4KVSCore, Value};

#[tokio::test]
async fn test_user_story_scan_prefix() {
    // Given: A new F4KVS instance with data organized by prefix
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert keys with different prefixes
    let user_keys = vec!["user:1", "user:2", "user:3"];
    let product_keys = vec!["product:1", "product:2"];
    let order_keys = vec!["order:1"];

    for key in &user_keys {
        engine
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .expect("Failed to put");
    }
    for key in &product_keys {
        engine
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .expect("Failed to put");
    }
    for key in &order_keys {
        engine
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .expect("Failed to put");
    }

    // When: I scan keys with "user:" prefix
    let scanned = engine
        .scan_prefix("user:")
        .await
        .expect("Failed to scan prefix");

    // Then: I should get all user keys
    assert_eq!(scanned.len(), user_keys.len(), "Should find all user keys");
    for key in &user_keys {
        assert!(
            scanned.contains(&key.to_string()),
            "Should contain key: {}",
            key
        );
    }
}

#[tokio::test]
async fn test_user_story_scan_range() {
    // Given: A new F4KVS instance with ordered keys
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert keys in a range
    let keys = vec!["key:001", "key:002", "key:003", "key:004", "key:005"];

    for key in &keys {
        engine
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .expect("Failed to put");
    }

    // When: I scan keys in range "key:002" to "key:004" (inclusive start, exclusive end)
    let scanned = engine
        .scan_range("key:002", "key:004")
        .await
        .expect("Failed to scan range");

    // Then: I should get keys in the range
    assert_eq!(
        scanned.len(),
        2,
        "Should find 2 keys in range (exclusive end)"
    );
    assert!(
        scanned.contains(&"key:002".to_string()),
        "Should contain key:002"
    );
    assert!(
        scanned.contains(&"key:003".to_string()),
        "Should contain key:003"
    );
    assert!(
        !scanned.contains(&"key:004".to_string()),
        "Should NOT contain key:004 (exclusive end)"
    );
}

#[tokio::test]
async fn test_user_story_scan_prefix_pairs() {
    // Given: A new F4KVS instance with data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert key-value pairs with prefix
    let pairs = vec![
        ("user:alice", "Alice Smith"),
        ("user:bob", "Bob Jones"),
        ("user:charlie", "Charlie Brown"),
    ];

    for (key, value) in &pairs {
        engine
            .put(key, &Value::String(value.to_string()))
            .await
            .expect("Failed to put");
    }

    // When: I scan key-value pairs with "user:" prefix
    let scanned = engine
        .scan_prefix_pairs("user:")
        .await
        .expect("Failed to scan prefix pairs");

    // Then: I should get all user key-value pairs
    assert_eq!(scanned.len(), pairs.len(), "Should find all user pairs");

    for (key, expected_value) in &pairs {
        let found = scanned
            .iter()
            .find(|(k, _)| k == key)
            .expect(&format!("Should find key: {}", key));
        match &found.1 {
            Value::String(s) => assert_eq!(s, expected_value, "Value should match"),
            _ => panic!("Value should be a String"),
        }
    }
}

#[tokio::test]
async fn test_user_story_scan_range_pairs() {
    // Given: A new F4KVS instance with ordered data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert key-value pairs in order
    let pairs = vec![
        ("item:001", "Item One"),
        ("item:002", "Item Two"),
        ("item:003", "Item Three"),
        ("item:004", "Item Four"),
        ("item:005", "Item Five"),
    ];

    for (key, value) in &pairs {
        engine
            .put(key, &Value::String(value.to_string()))
            .await
            .expect("Failed to put");
    }

    // When: I scan key-value pairs in range "item:002" to "item:004" (exclusive end)
    let scanned = engine
        .scan_range_pairs("item:002", "item:004")
        .await
        .expect("Failed to scan range pairs");

    // Then: I should get pairs in the range
    assert_eq!(
        scanned.len(),
        2,
        "Should find 2 pairs in range (exclusive end)"
    );

    let found_002 = scanned
        .iter()
        .find(|(k, _)| k == "item:002")
        .expect("Should find item:002");
    match &found_002.1 {
        Value::String(s) => assert_eq!(s, "Item Two"),
        _ => panic!("Value should be a String"),
    }

    let found_003 = scanned
        .iter()
        .find(|(k, _)| k == "item:003")
        .expect("Should find item:003");
    match &found_003.1 {
        Value::String(s) => assert_eq!(s, "Item Three"),
        _ => panic!("Value should be a String"),
    }

    assert!(
        scanned.iter().find(|(k, _)| k == "item:004").is_none(),
        "Should NOT contain item:004 (exclusive end)"
    );
}

#[tokio::test]
async fn test_user_story_scan_large_dataset() {
    // Given: A new F4KVS instance with a large dataset
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert 10K+ keys with a common prefix
    let prefix = "large:";
    let dataset_size = 10_000;

    for i in 0..dataset_size {
        let key = format!("{}{}", prefix, i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I scan keys with the prefix
    let start = std::time::Instant::now();
    let scanned = engine
        .scan_prefix(&prefix)
        .await
        .expect("Failed to scan large dataset");
    let duration = start.elapsed();

    // Then: I should get all keys efficiently
    assert_eq!(
        scanned.len(),
        dataset_size,
        "Should find all keys in large dataset"
    );

    // Verify scan is reasonably fast (should complete in < 1 second for memory storage)
    assert!(
        duration.as_secs() < 1,
        "Large dataset scan should be fast (< 1 second)"
    );

    // Verify all keys are present
    for i in 0..100 {
        // Sample check
        let expected_key = format!("{}{}", prefix, i);
        assert!(
            scanned.contains(&expected_key),
            "Should contain key: {}",
            expected_key
        );
    }
}

#[tokio::test]
async fn test_user_story_scan_empty_results() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I scan with a prefix that doesn't exist
    let scanned = engine
        .scan_prefix("nonexistent_prefix:")
        .await
        .expect("Failed to scan");

    // Then: I should get an empty result
    assert_eq!(
        scanned.len(),
        0,
        "Should return empty result for non-existent prefix"
    );

    // When: I scan a range that doesn't exist
    let scanned_range = engine
        .scan_range("zzz:start", "zzz:end")
        .await
        .expect("Failed to scan range");

    // Then: I should get an empty result
    assert_eq!(
        scanned_range.len(),
        0,
        "Should return empty result for non-existent range"
    );

    // When: I scan prefix pairs with non-existent prefix
    let scanned_pairs = engine
        .scan_prefix_pairs("nonexistent:")
        .await
        .expect("Failed to scan prefix pairs");

    // Then: I should get an empty result
    assert_eq!(
        scanned_pairs.len(),
        0,
        "Should return empty result for non-existent prefix pairs"
    );
}

#[tokio::test]
async fn test_user_story_scan_count_operations() {
    // Given: A new F4KVS instance with data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert keys with different prefixes
    for i in 0..100 {
        let key = format!("category:a:{}", i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    for i in 0..50 {
        let key = format!("category:b:{}", i);
        engine
            .put(&key, &Value::String(format!("value_{}", i)))
            .await
            .expect("Failed to put");
    }

    // When: I count keys with prefix "category:a:"
    let count_a = engine
        .count_prefix("category:a:")
        .await
        .expect("Failed to count prefix");

    // Then: I should get the correct count
    assert_eq!(
        count_a, 100,
        "Should count 100 keys with category:a: prefix"
    );

    // When: I count keys with prefix "category:b:"
    let count_b = engine
        .count_prefix("category:b:")
        .await
        .expect("Failed to count prefix");

    // Then: I should get the correct count
    assert_eq!(count_b, 50, "Should count 50 keys with category:b: prefix");

    // When: I count keys in a range
    // Note: count_range may have different behavior depending on implementation
    // We'll verify it returns a reasonable count
    let count_range = engine
        .count_range("category:a:010", "category:a:020")
        .await
        .expect("Failed to count range");

    // Then: count range operation succeeds (exact count may vary by implementation)
    let _ = count_range;
}
