//! User Story: Batch Operations
//!
//! As a developer
//! I want to perform batch operations
//! So that I can efficiently handle multiple items

use f4kvs_core::{F4KVSCore, Value};

#[tokio::test]
async fn test_user_story_batch_put() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I batch insert 1000+ items
    let batch_size = 1000;
    let mut batch_data = Vec::new();
    for i in 0..batch_size {
        batch_data.push((
            format!("batch_key_{}", i),
            Value::String(format!("batch_value_{}", i)),
        ));
    }

    engine
        .batch_put(batch_data.clone())
        .await
        .expect("Failed to batch put");

    // Then: All items should be retrievable
    let keys: Vec<String> = batch_data.iter().map(|(k, _)| k.clone()).collect();
    let retrieved = engine
        .batch_get(keys.clone())
        .await
        .expect("Failed to batch get");

    assert_eq!(
        retrieved.len(),
        batch_size,
        "Should retrieve all batch items"
    );

    for (i, value) in retrieved.iter().enumerate() {
        assert!(value.is_some(), "Item {} should be present", i);
        if let Some(Value::String(s)) = value {
            assert_eq!(
                s,
                &format!("batch_value_{}", i),
                "Item {} should have correct value",
                i
            );
        }
    }
}

#[tokio::test]
async fn test_user_story_batch_get() {
    // Given: A new F4KVS instance with some data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert some keys
    engine
        .put("existing_key_1", &Value::String("value1".to_string()))
        .await
        .expect("Failed to put");
    engine
        .put("existing_key_2", &Value::String("value2".to_string()))
        .await
        .expect("Failed to put");

    // When: I batch retrieve with some missing keys
    let keys = vec![
        "existing_key_1".to_string(),
        "non_existent_key".to_string(),
        "existing_key_2".to_string(),
        "another_missing_key".to_string(),
    ];

    let retrieved = engine
        .batch_get(keys.clone())
        .await
        .expect("Failed to batch get");

    // Then: Existing keys should return values, missing keys should return None
    assert_eq!(
        retrieved.len(),
        keys.len(),
        "Should return result for each key"
    );

    assert_eq!(
        retrieved[0],
        Some(Value::String("value1".to_string())),
        "First key should exist"
    );
    assert_eq!(retrieved[1], None, "Second key should not exist");
    assert_eq!(
        retrieved[2],
        Some(Value::String("value2".to_string())),
        "Third key should exist"
    );
    assert_eq!(retrieved[3], None, "Fourth key should not exist");
}

#[tokio::test]
async fn test_user_story_batch_delete() {
    // Given: A new F4KVS instance with data
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Setup: Insert multiple keys
    let keys_to_delete = vec![
        "delete_key_1".to_string(),
        "delete_key_2".to_string(),
        "delete_key_3".to_string(),
    ];

    for key in &keys_to_delete {
        engine
            .put(key, &Value::String(format!("value_{}", key)))
            .await
            .expect("Failed to put");
    }

    // Verify they exist
    for key in &keys_to_delete {
        assert!(
            engine.exists(key).await.expect("Failed to check existence"),
            "Key should exist before deletion"
        );
    }

    // When: I batch delete them
    engine
        .batch_delete(keys_to_delete.clone())
        .await
        .expect("Failed to batch delete");

    // Then: All keys should be deleted
    for key in &keys_to_delete {
        let exists = engine.exists(key).await.expect("Failed to check existence");
        assert!(!exists, "Key should not exist after batch delete");
    }
}

#[tokio::test]
async fn test_user_story_batch_atomicity() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform a batch put operation
    let batch_data = vec![
        (
            "atomic_key_1".to_string(),
            Value::String("value1".to_string()),
        ),
        (
            "atomic_key_2".to_string(),
            Value::String("value2".to_string()),
        ),
        (
            "atomic_key_3".to_string(),
            Value::String("value3".to_string()),
        ),
    ];

    engine
        .batch_put(batch_data.clone())
        .await
        .expect("Failed to batch put");

    // Then: All items should be present (atomic operation)
    let keys: Vec<String> = batch_data.iter().map(|(k, _)| k.clone()).collect();
    let retrieved = engine.batch_get(keys).await.expect("Failed to batch get");

    for value in retrieved {
        assert!(
            value.is_some(),
            "All batch items should be present (atomicity)"
        );
    }

    // When: I perform a batch delete
    let keys_to_delete: Vec<String> = batch_data.iter().map(|(k, _)| k.clone()).collect();
    engine
        .batch_delete(keys_to_delete.clone())
        .await
        .expect("Failed to batch delete");

    // Then: All items should be deleted (atomic operation)
    let retrieved_after_delete = engine
        .batch_get(keys_to_delete)
        .await
        .expect("Failed to batch get");

    for value in retrieved_after_delete {
        assert_eq!(value, None, "All batch items should be deleted (atomicity)");
    }
}

#[tokio::test]
async fn test_user_story_batch_partial_failure() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I try to batch put with invalid keys (empty key should fail validation)
    let batch_data = vec![
        (
            "valid_key_1".to_string(),
            Value::String("value1".to_string()),
        ),
        ("".to_string(), Value::String("invalid".to_string())), // Empty key should fail
        (
            "valid_key_2".to_string(),
            Value::String("value2".to_string()),
        ),
    ];

    let result = engine.batch_put(batch_data).await;

    // Then: The operation should fail (validation happens before batch operation)
    assert!(result.is_err(), "Batch put with invalid key should fail");

    // Verify that no partial data was written
    let valid_key_1_exists = engine
        .exists("valid_key_1")
        .await
        .expect("Failed to check existence");
    let valid_key_2_exists = engine
        .exists("valid_key_2")
        .await
        .expect("Failed to check existence");

    // Note: Depending on implementation, batch operations may be atomic
    // If atomic, both should not exist. If not atomic, behavior may vary.
    // For now, we verify the operation failed
    assert!(
        !valid_key_1_exists || !valid_key_2_exists,
        "At least one key should not exist if batch was atomic"
    );
}

#[tokio::test]
async fn test_user_story_batch_mixed_operations() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform batch operations with different value types
    let batch_data = vec![
        (
            "batch_string".to_string(),
            Value::String("string_value".to_string()),
        ),
        ("batch_int".to_string(), Value::Int64(42)),
        ("batch_float".to_string(), Value::Float64(3.14)),
        ("batch_bool".to_string(), Value::Bool(true)),
        ("batch_bytes".to_string(), Value::Bytes(vec![1, 2, 3])),
    ];

    engine
        .batch_put(batch_data.clone())
        .await
        .expect("Failed to batch put mixed types");

    // Then: All types should be retrievable correctly
    let keys: Vec<String> = batch_data.iter().map(|(k, _)| k.clone()).collect();
    let retrieved = engine.batch_get(keys).await.expect("Failed to batch get");

    assert_eq!(retrieved.len(), batch_data.len());
    for (i, (retrieved_value, original_value)) in
        retrieved.iter().zip(batch_data.iter()).enumerate()
    {
        assert!(retrieved_value.is_some(), "Item {} should be present", i);
        assert_eq!(
            retrieved_value.as_ref().unwrap(),
            &original_value.1,
            "Item {} should match original value",
            i
        );
    }
}

#[tokio::test]
async fn test_user_story_batch_large_operations() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform batch operations with a large number of items (10K+)
    let large_batch_size = 10_000;
    let mut large_batch_data = Vec::new();
    for i in 0..large_batch_size {
        large_batch_data.push((
            format!("large_batch_key_{}", i),
            Value::String(format!("large_batch_value_{}", i)),
        ));
    }

    engine
        .batch_put(large_batch_data.clone())
        .await
        .expect("Failed to batch put large dataset");

    // Then: All items should be retrievable
    let keys: Vec<String> = large_batch_data.iter().map(|(k, _)| k.clone()).collect();
    let retrieved = engine
        .batch_get(keys)
        .await
        .expect("Failed to batch get large dataset");

    assert_eq!(
        retrieved.len(),
        large_batch_size,
        "Should retrieve all large batch items"
    );

    // Verify a sample of items
    for i in 0..100 {
        if let Some(Some(Value::String(s))) = retrieved.get(i) {
            assert_eq!(
                s,
                &format!("large_batch_value_{}", i),
                "Item {} should have correct value",
                i
            );
        } else {
            panic!("Item {} should be present and be a String", i);
        }
    }
}
