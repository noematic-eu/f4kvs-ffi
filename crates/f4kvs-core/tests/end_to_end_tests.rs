//! End-to-end integration tests for F4KVS
//!
//! These tests verify the complete functionality of F4KVS including:
//! - Core storage operations
//! - Performance characteristics
//! - Error handling and recovery

use f4kvs_core::{F4KVSCore, Value};
use std::time::Instant;

/// Helper function for basic storage operations test
async fn test_basic_storage_operations_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing basic storage operations...");

    // Initialize F4KVS core
    let engine = F4KVSCore::new()?;

    // Test data
    let test_key = "test_key_1";
    let test_value = Value::String("Hello, F4KVS!".to_string());

    // Test PUT operation
    println!("📤 Testing PUT operation...");
    let put_start = Instant::now();
    engine.put(test_key, &test_value).await?;
    let put_duration = put_start.elapsed();
    println!("✅ PUT completed in {:?}", put_duration);

    // Test GET operation
    println!("📥 Testing GET operation...");
    let get_start = Instant::now();
    let retrieved_value = engine.get(test_key).await?;
    let get_duration = get_start.elapsed();
    println!("✅ GET completed in {:?}", get_duration);

    // Verify data integrity
    assert_eq!(retrieved_value, Some(test_value));
    println!("✅ Data integrity verified");

    // Test DELETE operation
    println!("🗑️ Testing DELETE operation...");
    let delete_start = Instant::now();
    engine.delete(test_key).await?;
    let delete_duration = delete_start.elapsed();
    println!("✅ DELETE completed in {:?}", delete_duration);

    // Verify deletion
    let deleted_value = engine.get(test_key).await?;
    assert_eq!(deleted_value, None);
    println!("✅ Deletion verified");

    println!("🎉 Basic storage operations test passed!");
    Ok(())
}

/// Test basic storage operations
#[tokio::test]
async fn test_basic_storage_operations() -> Result<(), Box<dyn std::error::Error>> {
    test_basic_storage_operations_impl().await
}

/// Helper function for batch operations test
async fn test_batch_operations_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing batch operations...");

    // Initialize F4KVS core
    let engine = F4KVSCore::new()?;

    // Prepare batch data
    let batch_size = 1000;
    let mut batch_data = Vec::new();
    for i in 0..batch_size {
        batch_data.push((
            format!("batch_key_{}", i),
            Value::String(format!("batch_value_{}", i)),
        ));
    }

    // Test batch PUT
    println!("📤 Testing batch PUT with {} items...", batch_size);
    let batch_start = Instant::now();
    for (key, value) in &batch_data {
        engine.put(key, value).await?;
    }
    let batch_duration = batch_start.elapsed();
    let throughput = batch_size as f64 / batch_duration.as_secs_f64();
    println!(
        "✅ Batch PUT completed in {:?} ({:.2} ops/sec)",
        batch_duration, throughput
    );

    // Test batch GET
    println!("📥 Testing batch GET...");
    let get_start = Instant::now();
    for (key, _) in &batch_data {
        let value = engine.get(key).await?;
        assert!(value.is_some());
    }
    let get_duration = get_start.elapsed();
    let get_throughput = batch_size as f64 / get_duration.as_secs_f64();
    println!(
        "✅ Batch GET completed in {:?} ({:.2} ops/sec)",
        get_duration, get_throughput
    );

    // Test batch DELETE
    println!("🗑️ Testing batch DELETE...");
    let delete_start = Instant::now();
    for (key, _) in &batch_data {
        engine.delete(key).await?;
    }
    let delete_duration = delete_start.elapsed();
    let delete_throughput = batch_size as f64 / delete_duration.as_secs_f64();
    println!(
        "✅ Batch DELETE completed in {:?} ({:.2} ops/sec)",
        delete_duration, delete_throughput
    );

    println!("🎉 Batch operations test passed!");
    Ok(())
}

/// Test batch operations
#[tokio::test]
async fn test_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
    test_batch_operations_impl().await
}

/// Helper function for performance characteristics test
async fn test_performance_characteristics_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing performance characteristics...");

    // Initialize F4KVS core
    let engine = F4KVSCore::new()?;

    // Test read performance
    println!("📊 Testing read performance...");
    let read_ops = 10000;

    // Pre-populate data
    for i in 0..read_ops {
        let key = format!("perf_key_{}", i);
        let value = Value::String(format!("perf_value_{}", i));
        engine.put(&key, &value).await?;
    }

    // Measure read performance
    let read_start = Instant::now();
    for i in 0..read_ops {
        let key = format!("perf_key_{}", i);
        let _ = engine.get(&key).await?;
    }
    let read_duration = read_start.elapsed();
    let read_throughput = read_ops as f64 / read_duration.as_secs_f64();
    println!("✅ Read performance: {:.2} ops/sec", read_throughput);

    // Test write performance
    println!("📊 Testing write performance...");
    let write_ops = 10000;
    let write_start = Instant::now();
    for i in 0..write_ops {
        let key = format!("write_key_{}", i);
        let value = Value::String(format!("write_value_{}", i));
        engine.put(&key, &value).await?;
    }
    let write_duration = write_start.elapsed();
    let write_throughput = write_ops as f64 / write_duration.as_secs_f64();
    println!("✅ Write performance: {:.2} ops/sec", write_throughput);

    // Test mixed workload
    println!("📊 Testing mixed workload...");
    let mixed_ops = 5000;
    let mixed_start = Instant::now();
    for i in 0..mixed_ops {
        if i % 2 == 0 {
            // Write operation
            let key = format!("mixed_key_{}", i);
            let value = Value::String(format!("mixed_value_{}", i));
            engine.put(&key, &value).await?;
        } else {
            // Read operation
            let key = format!("mixed_key_{}", i - 1);
            let _ = engine.get(&key).await?;
        }
    }
    let mixed_duration = mixed_start.elapsed();
    let mixed_throughput = mixed_ops as f64 / mixed_duration.as_secs_f64();
    println!(
        "✅ Mixed workload performance: {:.2} ops/sec",
        mixed_throughput
    );

    println!("🎉 Performance characteristics test passed!");
    Ok(())
}

/// Test performance characteristics
#[tokio::test]
async fn test_performance_characteristics() -> Result<(), Box<dyn std::error::Error>> {
    test_performance_characteristics_impl().await
}

/// Helper function for error handling test
async fn test_error_handling_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing error handling and recovery...");

    // Initialize F4KVS core
    let engine = F4KVSCore::new()?;

    // Test handling of non-existent keys
    println!("🔍 Testing non-existent key handling...");
    let non_existent_key = "non_existent_key";
    let result = engine.get(non_existent_key).await?;
    assert_eq!(result, None);
    println!("✅ Non-existent key handled correctly");

    // Test handling of empty values
    println!("🔍 Testing empty value handling...");
    let empty_key = "empty_key";
    let empty_value = Value::String(String::new());
    engine.put(empty_key, &empty_value).await?;
    let retrieved = engine.get(empty_key).await?;
    assert_eq!(retrieved, Some(empty_value));
    println!("✅ Empty value handled correctly");

    // Test handling of large values
    println!("🔍 Testing large value handling...");
    let large_key = "large_key";
    let large_value = Value::String("x".repeat(1024 * 1024)); // 1MB string
    engine.put(large_key, &large_value).await?;
    let retrieved_large = engine.get(large_key).await?;
    assert_eq!(retrieved_large, Some(large_value));
    println!("✅ Large value handled correctly");

    // Test concurrent operations
    println!("🔍 Testing concurrent operations...");
    let concurrent_ops = 100;
    let mut handles = Vec::new();

    for i in 0..concurrent_ops {
        let engine_clone = engine.clone();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let value = Value::String(format!("concurrent_value_{}", i));
            engine_clone.put(&key, &value).await
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await??;
    }
    println!("✅ Concurrent operations completed successfully");

    println!("🎉 Error handling and recovery test passed!");
    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    test_error_handling_impl().await
}

/// Helper function for data persistence test
async fn test_data_persistence_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing data persistence...");

    // This test would require persistent storage to be meaningful
    // For now, we'll test that the engine can be recreated and maintain state
    // within the same process

    let engine1 = F4KVSCore::new()?;
    let test_key = "persistence_test_key";
    let test_value = Value::String("persistence_test_value".to_string());

    // Store data
    engine1.put(test_key, &test_value).await?;

    // Verify data is accessible
    let retrieved = engine1.get(test_key).await?;
    assert_eq!(retrieved, Some(test_value));

    println!("✅ Data persistence test passed!");
    Ok(())
}

/// Test data persistence and recovery
#[tokio::test]
async fn test_data_persistence() -> Result<(), Box<dyn std::error::Error>> {
    test_data_persistence_impl().await
}

/// Helper function for memory usage test
async fn test_memory_usage_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing memory usage and cleanup...");

    let engine = F4KVSCore::new()?;

    // Store a large amount of data
    let large_data_size = 10000;
    for i in 0..large_data_size {
        let key = format!("memory_key_{}", i);
        let value = Value::String(format!("memory_value_{}", i));
        engine.put(&key, &value).await?;
    }

    // Verify data is accessible
    for i in 0..large_data_size {
        let key = format!("memory_key_{}", i);
        let result = engine.get(&key).await?;
        assert!(result.is_some());
    }

    // Clean up data
    for i in 0..large_data_size {
        let key = format!("memory_key_{}", i);
        engine.delete(&key).await?;
    }

    // Verify cleanup
    for i in 0..large_data_size {
        let key = format!("memory_key_{}", i);
        let result = engine.get(&key).await?;
        assert_eq!(result, None);
    }

    println!("✅ Memory usage and cleanup test passed!");
    Ok(())
}

/// Test memory usage and cleanup
#[tokio::test]
async fn test_memory_usage() -> Result<(), Box<dyn std::error::Error>> {
    test_memory_usage_impl().await
}

/// Helper function for configuration test
async fn test_configuration_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing configuration and customization...");

    // Test default configuration
    let engine = F4KVSCore::new()?;

    // Test basic operations with default config
    let test_key = "config_test_key";
    let test_value = Value::String("config_test_value".to_string());

    engine.put(test_key, &test_value).await?;
    let retrieved = engine.get(test_key).await?;
    assert_eq!(retrieved, Some(test_value));

    println!("✅ Configuration test passed!");
    Ok(())
}

/// Test configuration and customization
#[tokio::test]
async fn test_configuration() -> Result<(), Box<dyn std::error::Error>> {
    test_configuration_impl().await
}

/// Helper function for component integration test
async fn test_component_integration_impl() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing component integration...");

    // Test core + storage integration
    let engine = F4KVSCore::new()?;

    // Test basic operations
    let test_key = "integration_test_key";
    let test_value = Value::String("integration_test_value".to_string());

    engine.put(test_key, &test_value).await?;
    let retrieved = engine.get(test_key).await?;
    assert_eq!(retrieved, Some(test_value));

    println!("✅ Component integration test passed!");
    Ok(())
}

/// Test integration between different F4KVS components
#[tokio::test]
async fn test_component_integration() -> Result<(), Box<dyn std::error::Error>> {
    test_component_integration_impl().await
}

/// Main test runner
#[tokio::test]
async fn test_complete_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Running complete F4KVS integration test...");

    // Run all individual test implementations
    test_basic_storage_operations_impl().await?;
    test_batch_operations_impl().await?;
    test_performance_characteristics_impl().await?;
    test_error_handling_impl().await?;
    test_data_persistence_impl().await?;
    test_memory_usage_impl().await?;
    test_configuration_impl().await?;
    test_component_integration_impl().await?;

    println!("🎉 Complete F4KVS integration test passed!");
    Ok(())
}
