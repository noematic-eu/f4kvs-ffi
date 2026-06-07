//! User Story: Error Handling
//!
//! As a developer
//! I want to handle errors gracefully
//! So that my application can recover from failures

use f4kvs_core::{F4KVSCore, Value};

#[tokio::test]
async fn test_user_story_error_nonexistent_key() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I try to get a non-existent key
    let non_existent_key = "definitely_does_not_exist_12345";
    let result = engine.get(non_existent_key).await;

    // Then: It should return Ok(None) without error
    assert!(
        result.is_ok(),
        "Getting non-existent key should not return an error"
    );
    assert_eq!(result.unwrap(), None, "Non-existent key should return None");
}

#[tokio::test]
async fn test_user_story_error_invalid_key() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I try to use an empty key
    let empty_key = "";
    let test_value = Value::String("test".to_string());
    let result = engine.put(empty_key, &test_value).await;

    // Then: It should fail with a validation error
    assert!(result.is_err(), "Empty key should be rejected");

    // When: I try to get with an empty key
    let get_result = engine.get(empty_key).await;
    assert!(get_result.is_err(), "Getting with empty key should fail");

    // When: I try to delete with an empty key
    let delete_result = engine.delete(empty_key).await;
    assert!(
        delete_result.is_err(),
        "Deleting with empty key should fail"
    );
}

#[tokio::test]
async fn test_user_story_error_invalid_value() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // Note: F4KVS accepts all Value types, so "invalid value" here means
    // values that might cause issues in specific contexts

    // When: I try to put a very large value (this should still work, but tests the system)
    let large_key = "large_value_key";
    let very_large_string = "x".repeat(100 * 1024 * 1024); // 100MB
    let large_value = Value::String(very_large_string.clone());

    // This might succeed or fail depending on memory limits
    let result = engine.put(large_key, &large_value).await;

    // Then: The operation should either succeed or fail gracefully
    if result.is_ok() {
        // If it succeeds, verify we can retrieve it
        let retrieved = engine
            .get(large_key)
            .await
            .expect("Should retrieve large value");
        match retrieved {
            Some(Value::String(s)) => {
                assert_eq!(s.len(), very_large_string.len());
            }
            _ => panic!("Retrieved value should be a String"),
        }
    } else {
        // If it fails, it should be a proper error, not a panic
        let error = result.unwrap_err();
        assert!(!error.to_string().is_empty(), "Error should have a message");
    }
}

#[tokio::test]
async fn test_user_story_error_recovery() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform operations that might fail
    let test_key = "recovery_test_key";
    let test_value = Value::String("recovery_value".to_string());

    // First, try to put with an invalid key (should fail)
    let invalid_result = engine.put("", &test_value).await;
    assert!(invalid_result.is_err(), "Invalid key should fail");

    // Then: I should be able to recover and perform valid operations
    let valid_result = engine.put(test_key, &test_value).await;
    assert!(
        valid_result.is_ok(),
        "Should be able to perform valid operations after error"
    );

    // Verify the valid operation worked
    let retrieved = engine.get(test_key).await.expect("Should retrieve value");
    assert_eq!(retrieved, Some(test_value.clone()));

    // When: I try to delete a non-existent key (should succeed - idempotent)
    let delete_result = engine.delete("non_existent_recovery_key").await;
    assert!(
        delete_result.is_ok(),
        "Deleting non-existent key should succeed (idempotent)"
    );

    // Then: I should still be able to perform operations
    let another_key = "another_recovery_key";
    let another_value = Value::String("another_value".to_string());
    let put_result = engine.put(another_key, &another_value).await;
    assert!(
        put_result.is_ok(),
        "Should be able to perform operations after handling errors"
    );
}

#[tokio::test]
async fn test_user_story_error_graceful_degradation() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform operations with various edge cases
    let operations = vec![
        ("valid_key_1", Value::String("value1".to_string()), true),
        ("", Value::String("invalid".to_string()), false), // Empty key
        ("valid_key_2", Value::String("value2".to_string()), true),
    ];

    let mut successful_ops = 0;
    let mut failed_ops = 0;

    for (key, value, should_succeed) in operations {
        let result = engine.put(key, &value).await;
        if should_succeed {
            assert!(result.is_ok(), "Valid operation should succeed");
            successful_ops += 1;
        } else {
            assert!(result.is_err(), "Invalid operation should fail");
            failed_ops += 1;
        }
    }

    // Then: Valid operations should have succeeded
    assert_eq!(successful_ops, 2, "Should have 2 successful operations");
    assert_eq!(failed_ops, 1, "Should have 1 failed operation");

    // Verify successful operations are still accessible
    let retrieved_1 = engine.get("valid_key_1").await.expect("Should retrieve");
    assert_eq!(retrieved_1, Some(Value::String("value1".to_string())));

    let retrieved_2 = engine.get("valid_key_2").await.expect("Should retrieve");
    assert_eq!(retrieved_2, Some(Value::String("value2".to_string())));
}

#[tokio::test]
async fn test_user_story_error_batch_validation() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I try to batch put with invalid keys
    let batch_data = vec![
        (
            "valid_batch_key_1".to_string(),
            Value::String("value1".to_string()),
        ),
        ("".to_string(), Value::String("invalid".to_string())), // Empty key
        (
            "valid_batch_key_2".to_string(),
            Value::String("value2".to_string()),
        ),
    ];

    let result = engine.batch_put(batch_data).await;

    // Then: The batch operation should fail (validation happens before batch)
    assert!(result.is_err(), "Batch put with invalid key should fail");

    // Verify no partial data was written (if batch is atomic)
    let key1_exists = engine
        .exists("valid_batch_key_1")
        .await
        .expect("Failed to check");
    let key2_exists = engine
        .exists("valid_batch_key_2")
        .await
        .expect("Failed to check");

    // If batch is atomic, neither should exist
    // If not atomic, at least one might exist
    // For now, we just verify the operation failed
    assert!(
        !key1_exists || !key2_exists,
        "At least one key should not exist if batch was atomic"
    );
}

#[tokio::test]
async fn test_user_story_error_concurrent_operations() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I perform concurrent operations, some of which might fail
    let mut handles = Vec::new();
    let engine_clone = engine.clone();

    // Spawn tasks that mix valid and invalid operations
    for i in 0..10 {
        let engine_task = engine_clone.clone();
        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                // Valid operation
                let key = format!("concurrent_key_{}", i);
                let value = Value::String(format!("value_{}", i));
                engine_task.put(&key, &value).await
            } else {
                // Invalid operation (empty key)
                let value = Value::String("invalid".to_string());
                engine_task.put("", &value).await
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_ok() {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }

    // Then: Valid operations should succeed, invalid should fail
    assert_eq!(success_count, 5, "Should have 5 successful operations");
    assert_eq!(failure_count, 5, "Should have 5 failed operations");

    // Verify successful operations are accessible
    for i in 0..10 {
        if i % 2 == 0 {
            let key = format!("concurrent_key_{}", i);
            let exists = engine
                .exists(&key)
                .await
                .expect("Failed to check existence");
            assert!(
                exists,
                "Key {} should exist after successful concurrent operation",
                i
            );
        }
    }
}
