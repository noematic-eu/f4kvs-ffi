//! Property-based testing for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
//!
//! This module provides comprehensive property-based testing using proptest
//! to ensure correctness under various conditions and edge cases.

use crate::Value;
#[cfg(feature = "proptest")]
use crate::{Config, F4KVSCore, Result, StorageMode};
#[cfg(feature = "proptest")]
use proptest::prelude::*;
#[cfg(feature = "proptest")]
use std::collections::HashMap;
#[cfg(feature = "proptest")]
use std::sync::Arc;

/// Operations for property-based testing
#[derive(Debug, Clone, PartialEq)]
pub enum TestOperation {
    /// Put operation with key and value
    Put(String, Value),
    /// Get operation with key
    Get(String),
    /// Delete operation with key
    Delete(String),
    /// Exists operation with key
    Exists(String),
    /// Batch put operation
    BatchPut(Vec<(String, Value)>),
    /// Batch get operation
    BatchGet(Vec<String>),
    /// Batch delete operation
    BatchDelete(Vec<String>),
}

/// Property-based test suite
#[cfg(feature = "proptest")]
pub struct PropertyTestSuite;

#[cfg(feature = "proptest")]
impl PropertyTestSuite {
    /// Run comprehensive property tests
    pub async fn run_all_tests() -> Result<()> {
        // Test with different storage modes
        for storage_mode in [StorageMode::HashMap, StorageMode::BTreeMap] {
            let config = Config::new().with_storage_mode(storage_mode);
            let engine = F4KVSCore::with_config(config)?;

            // Run all property tests
            Self::test_key_value_properties(&engine).await?;
            Self::test_concurrent_properties(&engine).await?;
            Self::test_error_properties(&engine).await?;
            Self::test_memory_properties(&engine).await?;
            Self::test_config_properties(&engine).await?;
            Self::test_serialization_properties(&engine).await?;
        }

        Ok(())
    }

    /// Test basic key-value operation properties
    async fn test_key_value_properties(engine: &F4KVSCore) -> Result<()> {
        proptest!(ProptestConfig::with_cases(100), |(operations: Vec<TestOperation>)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut expected_state = HashMap::new();

                for operation in operations {
                    match operation {
                        TestOperation::Put(key, value) => {
                            engine.put(&key, &value).await.unwrap();
                            expected_state.insert(key, value);
                        }
                        TestOperation::Get(key) => {
                            let result = engine.get(&key).await.unwrap();
                            let expected = expected_state.get(&key).cloned();
                            assert_eq!(result, expected);
                        }
                        TestOperation::Delete(key) => {
                            engine.delete(&key).await.unwrap();
                            expected_state.remove(&key);
                        }
                        TestOperation::Exists(key) => {
                            let result = engine.exists(&key).await.unwrap();
                            let expected = expected_state.contains_key(&key);
                            assert_eq!(result, expected);
                        }
                        TestOperation::BatchPut(items) => {
                            for (key, value) in items {
                                engine.put(&key, &value).await.unwrap();
                                expected_state.insert(key, value);
                            }
                        }
                        TestOperation::BatchGet(keys) => {
                            for key in keys {
                                let result = engine.get(&key).await.unwrap();
                                let expected = expected_state.get(&key).cloned();
                                assert_eq!(result, expected);
                            }
                        }
                        TestOperation::BatchDelete(keys) => {
                            for key in keys {
                                engine.delete(&key).await.unwrap();
                                expected_state.remove(&key);
                            }
                        }
                    }
                }

                // Verify final state consistency
                for (key, expected_value) in &expected_state {
                    let actual_value = engine.get(key).await.unwrap();
                    assert_eq!(actual_value, Some(expected_value.clone()));
                }
            });
        });

        Ok(())
    }

    /// Test concurrent access properties
    async fn test_concurrent_properties(engine: &F4KVSCore) -> Result<()> {
        let engine = engine.clone();
        proptest!(ProptestConfig::with_cases(100), |(operations: Vec<TestOperation>)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let engine = Arc::new(engine.clone());
                let mut handles = Vec::new();

                // Spawn multiple tasks to test concurrent access
                for _i in 0..4 {
                    let engine = Arc::clone(&engine);
                    let ops = operations.clone();
                    let handle = tokio::spawn(async move {
                        let mut local_state = HashMap::new();

                        for operation in ops {
                            match operation {
                                TestOperation::Put(key, value) => {
                                    engine.put(&key, &value).await.unwrap();
                                    local_state.insert(key, value);
                                }
                                TestOperation::Get(key) => {
                                    let _result = engine.get(&key).await.unwrap();
                                }
                                TestOperation::Delete(key) => {
                                    engine.delete(&key).await.unwrap();
                                    local_state.remove(&key);
                                }
                                TestOperation::Exists(key) => {
                                    let _result = engine.exists(&key).await.unwrap();
                                }
                                _ => {} // Skip batch operations in concurrent test
                            }
                        }

                        local_state
                    });
                    handles.push(handle);
                }

                // Wait for all tasks to complete
                for handle in handles {
                    let _result = handle.await.unwrap();
                }
            });
        });

        Ok(())
    }

    /// Test error handling properties
    async fn test_error_properties(engine: &F4KVSCore) -> Result<()> {
        proptest!(ProptestConfig::with_cases(100), |(key: String, value: Value)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Test with invalid inputs
                if key.len() > 1024 {
                    // Key too long should be handled gracefully
                    let result = engine.put(&key, &value).await;
                    // Should either succeed or fail gracefully, not panic
                    let _ = result;
                } else {
                    // Valid key should always succeed
                    engine.put(&key, &value).await.unwrap();
                }
            });
        });

        Ok(())
    }

    /// Test memory management properties
    async fn test_memory_properties(engine: &F4KVSCore) -> Result<()> {
        proptest!(ProptestConfig::with_cases(100), |(operations: Vec<TestOperation>)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let initial_stats = engine.stats().await.unwrap();

                for operation in operations {
                    match operation {
                        TestOperation::Put(key, value) => {
                            engine.put(&key, &value).await.unwrap();
                        }
                        TestOperation::Get(key) => {
                            let _result = engine.get(&key).await.unwrap();
                        }
                        TestOperation::Delete(key) => {
                            engine.delete(&key).await.unwrap();
                        }
                        TestOperation::Exists(key) => {
                            let _result = engine.exists(&key).await.unwrap();
                        }
                        _ => {} // Skip batch operations in memory test
                    }
                }

                let final_stats = engine.stats().await.unwrap();

                // Memory usage should be reasonable
                assert!(final_stats.memory_usage >= initial_stats.memory_usage);
                assert!(final_stats.memory_usage < 1024 * 1024 * 1024); // Less than 1GB
            });
        });

        Ok(())
    }

    /// Test configuration properties
    async fn test_config_properties(_engine: &F4KVSCore) -> Result<()> {
        proptest!(ProptestConfig::with_cases(100), |(max_key_size: u32, max_value_size: u32)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let config = Config::new()
                    .with_max_key_size(max_key_size as usize)
                    .with_max_value_size(max_value_size as usize);

                let engine = F4KVSCore::with_config(config).unwrap();

                // Test with keys and values of various sizes
                let key = "a".repeat((max_key_size / 2) as usize);
                let value = Value::String("b".repeat((max_value_size / 2) as usize));

                engine.put(&key, &value).await.unwrap();
                let result = engine.get(&key).await.unwrap();
                assert_eq!(result, Some(value));
            });
        });

        Ok(())
    }

    /// Test serialization/deserialization properties
    async fn test_serialization_properties(engine: &F4KVSCore) -> Result<()> {
        proptest!(ProptestConfig::with_cases(100), |(value: Value)| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let key = "test_key";

                // Put and get should preserve value exactly
                engine.put(key, &value).await.unwrap();
                let retrieved = engine.get(key).await.unwrap();
                assert_eq!(retrieved, Some(value));
            });
        });

        Ok(())
    }
}

/// Proptest generators for F4KVS types
#[cfg(feature = "proptest")]
pub mod generators {
    use super::*;

    /// Generate random keys
    pub fn key() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_]{1,100}"
    }

    /// Generate random values
    pub fn value() -> impl Strategy<Value = Value> {
        prop_oneof![
            // String values
            "[a-zA-Z0-9_]{1,1000}".prop_map(Value::String),
            // Integer values
            (-1000i64..1000i64).prop_map(Value::Int64),
            // Float values
            (-1000.0f64..1000.0f64).prop_map(Value::Float64),
            // Boolean values
            prop::bool::ANY.prop_map(Value::Bool),
            // Null values
            Just(Value::Null),
            // Bytes values
            prop::collection::vec(prop::num::u8::ANY, 1..1000).prop_map(Value::Bytes),
        ]
    }

    /// Generate random test operations
    pub fn operation() -> impl Strategy<Value = TestOperation> {
        prop_oneof![
            // Put operation
            (key(), value()).prop_map(|(k, v)| TestOperation::Put(k, v)),
            // Get operation
            key().prop_map(TestOperation::Get),
            // Delete operation
            key().prop_map(TestOperation::Delete),
            // Exists operation
            key().prop_map(TestOperation::Exists),
            // Batch put operation
            prop::collection::vec((key(), value()), 1..10).prop_map(TestOperation::BatchPut),
            // Batch get operation
            prop::collection::vec(key(), 1..10).prop_map(TestOperation::BatchGet),
            // Batch delete operation
            prop::collection::vec(key(), 1..10).prop_map(TestOperation::BatchDelete),
        ]
    }

    /// Generate sequences of operations
    pub fn operation_sequence() -> impl Strategy<Value = Vec<TestOperation>> {
        prop::collection::vec(operation(), 1..50)
    }

    /// Generate random configurations
    pub fn config() -> impl Strategy<Value = Config> {
        (1u32..10000, 1u32..10000000, prop::bool::ANY).prop_map(
            |(max_key, max_value, strict_validation)| {
                Config::new()
                    .with_max_key_size(max_key as usize)
                    .with_max_value_size(max_value as usize)
                    .with_strict_key_validation(strict_validation)
            },
        )
    }
}

#[cfg(test)]
#[cfg(feature = "proptest")]
impl Arbitrary for TestOperation {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            (any::<String>(), any::<Value>())
                .prop_map(|(key, value)| TestOperation::Put(key, value)),
            any::<String>().prop_map(TestOperation::Get),
            any::<String>().prop_map(TestOperation::Delete),
            any::<String>().prop_map(TestOperation::Exists),
            prop::collection::vec((any::<String>(), any::<Value>()), 0..10)
                .prop_map(TestOperation::BatchPut),
            prop::collection::vec(any::<String>(), 0..10).prop_map(TestOperation::BatchGet),
            prop::collection::vec(any::<String>(), 0..10).prop_map(TestOperation::BatchDelete),
        ]
        .boxed()
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    #[cfg(feature = "proptest")]
    // Re-enabled: async/proptest compatibility should be resolved
    async fn test_property_test_suite() {
        PropertyTestSuite::run_all_tests().await.unwrap();
    }

    #[test]
    #[cfg(feature = "proptest")]
    fn test_key_generator() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(&generators::key(), |key| {
                assert!(!key.is_empty());
                assert!(key.len() <= 100);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    #[cfg(feature = "proptest")]
    fn test_value_generator() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(&generators::value(), |value| {
                // All generated values should be valid
                match value {
                    Value::String(s) => assert!(s.len() <= 1000),
                    Value::Bytes(b) => assert!(b.len() <= 1000),
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    #[cfg(feature = "proptest")]
    fn test_operation_generator() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(&generators::operation(), |operation| {
                // All generated operations should be valid
                match operation {
                    TestOperation::Put(key, _) => assert!(!key.is_empty()),
                    TestOperation::Get(key) => assert!(!key.is_empty()),
                    TestOperation::Delete(key) => assert!(!key.is_empty()),
                    TestOperation::Exists(key) => assert!(!key.is_empty()),
                    TestOperation::BatchPut(items) => assert!(!items.is_empty()),
                    TestOperation::BatchGet(keys) => assert!(!keys.is_empty()),
                    TestOperation::BatchDelete(keys) => assert!(!keys.is_empty()),
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    #[cfg(feature = "proptest")]
    fn test_config_generator() {
        let mut runner = proptest::test_runner::TestRunner::default();
        runner
            .run(&generators::config(), |config| {
                // All generated configs should be valid
                assert!(config.max_key_size > 0);
                assert!(config.max_value_size > 0);
                Ok(())
            })
            .unwrap();
    }
}
