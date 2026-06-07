//! User Story: Basic Database Operations
//!
//! As a developer
//! I want to perform basic CRUD operations
//! So that I can store and retrieve data

use f4kvs_core::{F4KVSCore, Value};

#[tokio::test]
async fn test_user_story_basic_crud() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");
    let test_key = "user:1";
    let test_value = Value::String("Alice".to_string());

    // When: I put a value
    engine
        .put(test_key, &test_value)
        .await
        .expect("Failed to put value");

    // Then: I can retrieve it
    let retrieved = engine.get(test_key).await.expect("Failed to get value");
    assert_eq!(retrieved, Some(test_value.clone()));

    // When: I check if the key exists
    let exists = engine
        .exists(test_key)
        .await
        .expect("Failed to check existence");
    assert!(exists, "Key should exist");

    // When: I delete the key
    engine.delete(test_key).await.expect("Failed to delete key");

    // Then: The key should no longer exist
    let deleted_value = engine.get(test_key).await.expect("Failed to get value");
    assert_eq!(deleted_value, None, "Deleted key should return None");

    let exists_after_delete = engine
        .exists(test_key)
        .await
        .expect("Failed to check existence");
    assert!(!exists_after_delete, "Key should not exist after deletion");
}

#[tokio::test]
async fn test_user_story_empty_values() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I put an empty string
    let empty_string_key = "empty_string";
    let empty_string_value = Value::String(String::new());
    engine
        .put(empty_string_key, &empty_string_value)
        .await
        .expect("Failed to put empty string");

    // Then: I can retrieve it
    let retrieved = engine
        .get(empty_string_key)
        .await
        .expect("Failed to get empty string");
    assert_eq!(retrieved, Some(empty_string_value));

    // When: I put a null value
    let null_key = "null_value";
    let null_value = Value::Null;
    engine
        .put(null_key, &null_value)
        .await
        .expect("Failed to put null value");

    // Then: I can retrieve it
    let retrieved_null = engine
        .get(null_key)
        .await
        .expect("Failed to get null value");
    assert_eq!(retrieved_null, Some(Value::Null));
    assert!(retrieved_null.unwrap().is_null(), "Value should be null");
}

#[tokio::test]
async fn test_user_story_large_values() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I put a large value (1MB+)
    let large_key = "large_value";
    let large_data = "x".repeat(1024 * 1024); // 1MB string
    let large_value = Value::String(large_data.clone());
    engine
        .put(large_key, &large_value)
        .await
        .expect("Failed to put large value");

    // Then: I can retrieve it correctly
    let retrieved = engine
        .get(large_key)
        .await
        .expect("Failed to get large value");
    match retrieved {
        Some(Value::String(s)) => {
            assert_eq!(s.len(), large_data.len(), "Large value size should match");
            assert_eq!(s, large_data, "Large value content should match");
        }
        _ => panic!("Retrieved value should be a String"),
    }

    // When: I put an even larger value (9MB to stay under 10MB limit)
    let very_large_key = "very_large_value";
    let very_large_data = "y".repeat(9 * 1024 * 1024); // 9MB string (under 10MB limit)
    let very_large_value = Value::String(very_large_data.clone());
    engine
        .put(very_large_key, &very_large_value)
        .await
        .expect("Failed to put very large value");

    // Then: I can retrieve it correctly
    let retrieved_very_large = engine
        .get(very_large_key)
        .await
        .expect("Failed to get very large value");
    match retrieved_very_large {
        Some(Value::String(s)) => {
            assert_eq!(
                s.len(),
                very_large_data.len(),
                "Very large value size should match"
            );
        }
        _ => panic!("Retrieved value should be a String"),
    }
}

#[tokio::test]
async fn test_user_story_special_keys() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I use Unicode keys
    let unicode_key = "user:测试:用户";
    let unicode_value = Value::String("Unicode test".to_string());
    engine
        .put(unicode_key, &unicode_value)
        .await
        .expect("Failed to put Unicode key");
    let retrieved = engine
        .get(unicode_key)
        .await
        .expect("Failed to get Unicode key");
    assert_eq!(retrieved, Some(unicode_value.clone()));

    // When: I use special characters in keys
    let special_chars_key = "user:test-key_123.test@example";
    let special_value = Value::String("Special chars test".to_string());
    engine
        .put(special_chars_key, &special_value)
        .await
        .expect("Failed to put special chars key");
    let retrieved_special = engine
        .get(special_chars_key)
        .await
        .expect("Failed to get special chars key");
    assert_eq!(retrieved_special, Some(special_value.clone()));

    // When: I use emoji in keys
    let emoji_key = "user:😀🎉🚀";
    let emoji_value = Value::String("Emoji test".to_string());
    engine
        .put(emoji_key, &emoji_value)
        .await
        .expect("Failed to put emoji key");
    let retrieved_emoji = engine
        .get(emoji_key)
        .await
        .expect("Failed to get emoji key");
    assert_eq!(retrieved_emoji, Some(emoji_value.clone()));

    // When: I use a very long key
    let long_key = "user:".to_string() + &"a".repeat(1000);
    let long_key_value = Value::String("Long key test".to_string());
    engine
        .put(&long_key, &long_key_value)
        .await
        .expect("Failed to put long key");
    let retrieved_long = engine.get(&long_key).await.expect("Failed to get long key");
    assert_eq!(retrieved_long, Some(long_key_value.clone()));
}

#[tokio::test]
async fn test_user_story_key_validation() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I try to use an empty key
    let empty_key = "";
    let test_value = Value::String("test".to_string());
    let result = engine.put(empty_key, &test_value).await;
    // Then: It should fail with validation error
    assert!(result.is_err(), "Empty key should be rejected");

    // When: I try to get a non-existent key
    let non_existent_key = "non_existent_key_12345";
    let retrieved = engine
        .get(non_existent_key)
        .await
        .expect("Get should succeed even for non-existent keys");
    // Then: It should return None
    assert_eq!(retrieved, None, "Non-existent key should return None");

    // When: I try to delete a non-existent key
    let delete_result = engine.delete(non_existent_key).await;
    // Then: It should succeed (idempotent operation)
    assert!(
        delete_result.is_ok(),
        "Deleting non-existent key should succeed"
    );

    // When: I check existence of non-existent key
    let exists = engine
        .exists(non_existent_key)
        .await
        .expect("Exists check should succeed");
    // Then: It should return false
    assert!(!exists, "Non-existent key should not exist");
}

#[tokio::test]
async fn test_user_story_different_value_types() {
    // Given: A new F4KVS instance
    let engine = F4KVSCore::new().expect("Failed to create F4KVS instance");

    // When: I store different value types
    let string_key = "value:string";
    let string_value = Value::String("Hello World".to_string());
    engine
        .put(string_key, &string_value)
        .await
        .expect("Failed to put string");

    let int_key = "value:int64";
    let int_value = Value::Int64(-42);
    engine
        .put(int_key, &int_value)
        .await
        .expect("Failed to put int64");

    let uint_key = "value:uint64";
    let uint_value = Value::UInt64(42);
    engine
        .put(uint_key, &uint_value)
        .await
        .expect("Failed to put uint64");

    let float_key = "value:float64";
    let float_value = Value::Float64(3.14159);
    engine
        .put(float_key, &float_value)
        .await
        .expect("Failed to put float64");

    let bool_key = "value:bool";
    let bool_value = Value::Bool(true);
    engine
        .put(bool_key, &bool_value)
        .await
        .expect("Failed to put bool");

    let bytes_key = "value:bytes";
    let bytes_value = Value::Bytes(vec![1, 2, 3, 4, 5]);
    engine
        .put(bytes_key, &bytes_value)
        .await
        .expect("Failed to put bytes");

    // Then: I can retrieve all of them correctly
    assert_eq!(
        engine.get(string_key).await.expect("Failed to get string"),
        Some(string_value)
    );
    assert_eq!(
        engine.get(int_key).await.expect("Failed to get int64"),
        Some(int_value)
    );
    assert_eq!(
        engine.get(uint_key).await.expect("Failed to get uint64"),
        Some(uint_value)
    );
    assert_eq!(
        engine.get(float_key).await.expect("Failed to get float64"),
        Some(float_value)
    );
    assert_eq!(
        engine.get(bool_key).await.expect("Failed to get bool"),
        Some(bool_value)
    );
    assert_eq!(
        engine.get(bytes_key).await.expect("Failed to get bytes"),
        Some(bytes_value)
    );
}
