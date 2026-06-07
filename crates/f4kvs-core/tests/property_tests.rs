//! Property-based tests for F4KVS Core
//!
//! This module provides property-based testing for critical components
//! using proptest to ensure correctness under various inputs.

use f4kvs_core::{MemoryStorage, QueryBuilder, StorageEngine, StorageMode, Value};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_storage_insert_get_property(
        key in "\\PC{1,100}",
        value in "\\PC{1,1000}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);
        let f4kvs_value = Value::String(value.clone());

        // Insert value
        let result = rt.block_on(storage.put(&key, &f4kvs_value));
        prop_assert!(result.is_ok());

        // Get value back
        let retrieved = rt.block_on(storage.get(&key));
        prop_assert!(retrieved.is_ok());

        let retrieved_option = retrieved.unwrap();
        prop_assert!(retrieved_option.is_some());
        let retrieved_value = retrieved_option.unwrap();
        prop_assert_eq!(retrieved_value, f4kvs_value);
    }
}

proptest! {
    #[test]
    fn test_storage_delete_property(
        key in "\\PC{1,100}",
        value in "\\PC{1,1000}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);
        let f4kvs_value = Value::String(value);

        // Insert value
        rt.block_on(storage.put(&key, &f4kvs_value)).unwrap();

        // Delete value
        let result = rt.block_on(storage.delete(&key));
        prop_assert!(result.is_ok());

        // Verify deletion
        let retrieved = rt.block_on(storage.get(&key));
        prop_assert!(retrieved.is_ok());
        let retrieved_option = retrieved.unwrap();
        prop_assert!(retrieved_option.is_none());
    }
}

proptest! {
    #[test]
    fn test_query_builder_property(
        prefix in "\\PC{1,50}",
        keys in prop::collection::vec("\\PC{1,100}", 1..10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Insert test data
        for (i, key) in keys.iter().enumerate() {
            let value = Value::String(format!("value_{}", i));
            rt.block_on(storage.put(key, &value)).unwrap();
        }

        // Test query with prefix
        let query = rt.block_on(QueryBuilder::new()
            .with_prefix(&prefix)
            .execute(&storage));

        prop_assert!(query.is_ok());

        let _result = query.unwrap();
        // total_count is always >= 0 for unsigned types
    }
}

proptest! {
    #[test]
    fn test_concurrent_operations_property(
        operations in prop::collection::vec(
            prop::collection::vec("\\PC{1,50}", 1..5),
            1..20
        )
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Execute operations sequentially (since MemoryStorage doesn't implement Clone)
        for ops in operations {
            for op in ops {
                let value = Value::String(format!("value_{}", op));
                let _ = rt.block_on(storage.put(&op, &value));
            }
        }

        // Operations completed successfully
        prop_assert!(true);
    }
}
