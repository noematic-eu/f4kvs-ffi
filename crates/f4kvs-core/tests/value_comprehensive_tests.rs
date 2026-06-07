//! Comprehensive Value type tests for F4KVS Core
//!
//! This module provides extensive test coverage for all Value type operations,
//! conversions, serialization, and edge cases.

use f4kvs_core::Value;

#[test]
fn test_value_type_names() {
    assert_eq!(Value::String("test".to_string()).type_name(), "String");
    assert_eq!(Value::Int64(42).type_name(), "Int64");
    assert_eq!(Value::UInt64(100).type_name(), "UInt64");
    assert_eq!(Value::Float64(3.14).type_name(), "Float64");
    assert_eq!(Value::Bool(true).type_name(), "Bool");
    assert_eq!(Value::Bytes(vec![1, 2, 3]).type_name(), "Bytes");
    assert_eq!(
        Value::Json(serde_json::json!({"key": "value"})).type_name(),
        "Json"
    );
    assert_eq!(Value::Null.type_name(), "Null");
}

#[test]
fn test_value_is_null() {
    assert!(Value::Null.is_null());
    assert!(!Value::String("test".to_string()).is_null());
    assert!(!Value::Int64(42).is_null());
    assert!(!Value::Bool(false).is_null());
}

#[test]
fn test_value_serialized_size() {
    assert_eq!(Value::String("hello".to_string()).serialized_size(), 5);
    assert_eq!(Value::Int64(42).serialized_size(), 8);
    assert_eq!(Value::UInt64(100).serialized_size(), 8);
    assert_eq!(Value::Float64(3.14).serialized_size(), 8);
    assert_eq!(Value::Bool(true).serialized_size(), 1);
    assert_eq!(Value::Bytes(vec![1, 2, 3, 4, 5]).serialized_size(), 5);
    assert_eq!(Value::Null.serialized_size(), 0);

    let json_val = Value::Json(serde_json::json!({"key": "value"}));
    assert!(json_val.serialized_size() > 0);
}

#[test]
fn test_value_to_bytes() {
    assert_eq!(
        Value::String("hello".to_string()).to_bytes(),
        b"hello".to_vec()
    );
    assert_eq!(Value::Int64(42).to_bytes(), 42i64.to_le_bytes().to_vec());
    assert_eq!(Value::UInt64(100).to_bytes(), 100u64.to_le_bytes().to_vec());
    assert_eq!(
        Value::Float64(3.14).to_bytes(),
        3.14f64.to_le_bytes().to_vec()
    );
    assert_eq!(Value::Bool(true).to_bytes(), vec![1]);
    assert_eq!(Value::Bool(false).to_bytes(), vec![0]);
    assert_eq!(Value::Bytes(vec![1, 2, 3]).to_bytes(), vec![1, 2, 3]);
    assert_eq!(Value::Null.to_bytes(), Vec::<u8>::new());

    let json_val = Value::Json(serde_json::json!({"key": "value"}));
    assert!(!json_val.to_bytes().is_empty());
}

#[test]
fn test_value_to_json_string() {
    let string_val = Value::String("hello".to_string());
    let json = string_val.to_json_string().unwrap();
    assert!(json.contains("hello"));
    assert!(json.contains("String"));

    let int_val = Value::Int64(42);
    let json = int_val.to_json_string().unwrap();
    assert!(json.contains("42"));
    assert!(json.contains("Int64"));

    let bool_val = Value::Bool(true);
    let json = bool_val.to_json_string().unwrap();
    assert!(json.contains("true"));
    assert!(json.contains("Bool"));

    let null_val = Value::Null;
    let json = null_val.to_json_string().unwrap();
    // Null is a unit variant, serializes as "Null"
    assert!(json.contains("Null"));
}

#[test]
fn test_value_from_json_string() {
    // Value enum uses externally tagged format for serialization
    let json = r#"{"String":"hello"}"#;
    let val = Value::from_json_string(json).unwrap();
    assert_eq!(val, Value::String("hello".to_string()));

    let json = r#"{"Int64":42}"#;
    let val = Value::from_json_string(json).unwrap();
    assert_eq!(val, Value::Int64(42));

    let json = r#"{"Bool":true}"#;
    let val = Value::from_json_string(json).unwrap();
    assert_eq!(val, Value::Bool(true));

    // Null is a unit variant, serializes as "Null"
    let json = r#""Null""#;
    let val = Value::from_json_string(json).unwrap();
    assert_eq!(val, Value::Null);
}

#[test]
fn test_value_memory_size() {
    let string_val = Value::String("hello".to_string());
    assert!(string_val.memory_size() >= 5);

    assert_eq!(Value::Int64(42).memory_size(), 8);
    assert_eq!(Value::UInt64(100).memory_size(), 8);
    assert_eq!(Value::Float64(3.14).memory_size(), 8);
    assert_eq!(Value::Bool(true).memory_size(), 1);

    let bytes_val = Value::Bytes(vec![1, 2, 3, 4, 5]);
    assert!(bytes_val.memory_size() >= 5);

    assert_eq!(Value::Null.memory_size(), 0);

    let json_val = Value::Json(serde_json::json!({"key": "value"}));
    assert!(json_val.memory_size() > 0);
}

#[test]
fn test_value_from_string() {
    let val: Value = "hello".to_string().into();
    assert_eq!(val, Value::String("hello".to_string()));
}

#[test]
fn test_value_from_str() {
    let val: Value = "world".into();
    assert_eq!(val, Value::String("world".to_string()));
}

#[test]
fn test_value_from_int64() {
    let val: Value = 42i64.into();
    assert_eq!(val, Value::Int64(42));
}

#[test]
fn test_value_from_uint64() {
    let val: Value = 100u64.into();
    assert_eq!(val, Value::UInt64(100));
}

#[test]
fn test_value_from_float64() {
    let val: Value = 3.14f64.into();
    assert_eq!(val, Value::Float64(3.14));
}

#[test]
fn test_value_from_bool() {
    let val: Value = true.into();
    assert_eq!(val, Value::Bool(true));

    let val: Value = false.into();
    assert_eq!(val, Value::Bool(false));
}

#[test]
fn test_value_from_bytes() {
    let bytes = vec![1, 2, 3, 4, 5];
    let val: Value = bytes.clone().into();
    assert_eq!(val, Value::Bytes(bytes));
}

#[test]
fn test_value_from_json() {
    let json = serde_json::json!({"key": "value"});
    let val: Value = json.clone().into();
    assert_eq!(val, Value::Json(json));
}

#[test]
fn test_value_json_memory_size() {
    // Test JSON array memory size
    let json_array = Value::Json(serde_json::json!([1, 2, 3, 4, 5]));
    assert!(json_array.memory_size() > 0);

    // Test JSON object memory size
    let json_obj = Value::Json(serde_json::json!({
        "key1": "value1",
        "key2": "value2",
        "key3": 42
    }));
    assert!(json_obj.memory_size() > 0);

    // Test nested JSON
    let nested_json = Value::Json(serde_json::json!({
        "nested": {
            "inner": "value"
        }
    }));
    assert!(nested_json.memory_size() > 0);
}

#[test]
fn test_value_equality() {
    assert_eq!(
        Value::String("test".to_string()),
        Value::String("test".to_string())
    );
    assert_ne!(
        Value::String("test".to_string()),
        Value::String("other".to_string())
    );

    assert_eq!(Value::Int64(42), Value::Int64(42));
    assert_ne!(Value::Int64(42), Value::Int64(43));

    assert_eq!(Value::Bool(true), Value::Bool(true));
    assert_ne!(Value::Bool(true), Value::Bool(false));

    assert_eq!(Value::Null, Value::Null);
    assert_ne!(Value::Null, Value::String("test".to_string()));
}

#[test]
fn test_value_clone() {
    let original = Value::String("test".to_string());
    let cloned = original.clone();
    assert_eq!(original, cloned);

    let json_original = Value::Json(serde_json::json!({"key": "value"}));
    let json_cloned = json_original.clone();
    assert_eq!(json_original, json_cloned);
}

#[test]
fn test_value_display() {
    let val = Value::String("hello".to_string());
    let display = format!("{:?}", val);
    assert!(display.contains("String"));
    assert!(display.contains("hello"));
}

#[test]
fn test_value_large_string() {
    let large_string = "x".repeat(10000);
    let val = Value::String(large_string.clone());
    assert_eq!(val.serialized_size(), 10000);
    assert_eq!(val.to_bytes(), large_string.as_bytes().to_vec());
}

#[test]
fn test_value_large_bytes() {
    let large_bytes = vec![0u8; 10000];
    let val = Value::Bytes(large_bytes.clone());
    assert_eq!(val.serialized_size(), 10000);
    assert_eq!(val.to_bytes(), large_bytes);
}

#[test]
fn test_value_edge_cases() {
    // Empty string
    let empty = Value::String(String::new());
    assert_eq!(empty.serialized_size(), 0);
    assert_eq!(empty.to_bytes(), Vec::<u8>::new());

    // Zero values
    assert_eq!(Value::Int64(0).serialized_size(), 8);
    assert_eq!(Value::UInt64(0).serialized_size(), 8);
    assert_eq!(Value::Float64(0.0).serialized_size(), 8);

    // Negative values
    assert_eq!(Value::Int64(-42).serialized_size(), 8);
    assert_eq!(
        Value::Int64(-42).to_bytes(),
        (-42i64).to_le_bytes().to_vec()
    );

    // Maximum values
    assert_eq!(Value::Int64(i64::MAX).serialized_size(), 8);
    assert_eq!(Value::UInt64(u64::MAX).serialized_size(), 8);
}

#[test]
fn test_value_json_serialization() {
    // Test complex JSON structure
    let complex_json = serde_json::json!({
        "string": "value",
        "number": 42,
        "boolean": true,
        "array": [1, 2, 3],
        "object": {
            "nested": "value"
        },
        "null": null
    });

    let val = Value::Json(complex_json.clone());
    let json_str = val.to_json_string().unwrap();
    let parsed = Value::from_json_string(&json_str).unwrap();

    if let (Value::Json(orig), Value::Json(parsed)) = (val, parsed) {
        assert_eq!(orig, parsed);
    } else {
        panic!("JSON serialization round-trip failed");
    }
}

#[test]
fn test_value_json_string_variants() {
    // Test JSON string with escaped characters - use enum format
    let json_str = r#"{"String":"hello\nworld"}"#;
    let val = Value::from_json_string(json_str).unwrap();
    assert!(matches!(val, Value::String(_)));
    if let Value::String(s) = val {
        assert_eq!(s, "hello\nworld");
    }

    // Test JSON array - wrapped in Json variant
    let json_str = r#"{"Json":[1,2,3,4,5]}"#;
    let val = Value::from_json_string(json_str).unwrap();
    assert!(matches!(val, Value::Json(_)));
}
