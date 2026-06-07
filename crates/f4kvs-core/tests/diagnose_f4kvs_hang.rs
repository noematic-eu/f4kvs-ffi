use f4kvs_core::{Config, F4KVSCore, F4KvsError, Value};
use std::time::Duration;

#[tokio::test]
async fn diagnose_f4kvs_async_hang() {
    println!("🧪 Testing F4KVSCore for async hangs...");

    // Test 1: Default configuration (may hang due to monitoring)
    println!("Testing with default configuration (monitoring enabled)...");
    let default_result = tokio::time::timeout(Duration::from_secs(5), async {
        let core = F4KVSCore::new()?;
        let value = Value::String("test_value".to_string());
        core.put("test_key", &value).await?;
        println!("✅ Basic put operation succeeded with default config");
        core.get("test_key").await?;
        println!("✅ Basic get operation succeeded with default config");
        Ok::<(), F4KvsError>(())
    })
    .await;

    match default_result {
        Ok(Ok(())) => println!("✅ Default config works - no monitoring deadlock"),
        Ok(Err(e)) => println!("❌ Core operation failed: {:?}", e),
        Err(_) => {
            println!("🚨 HANG DETECTED with default config - monitoring deadlock confirmed!");
            println!("This indicates monitoring hooks are causing deadlocks");
        }
    }

    // Test 2: Disabled monitoring configuration
    println!("Testing with monitoring disabled...");
    let disabled_result = tokio::time::timeout(Duration::from_secs(5), async {
        let config = Config {
            enable_monitoring: false,
            enable_memory_leak_detection: false,
            ..Default::default()
        };
        let core = F4KVSCore::with_config(config)?;
        let value = Value::String("test_value".to_string());
        core.put("test_key", &value).await?;
        println!("✅ Basic put operation succeeded with monitoring disabled");
        core.get("test_key").await?;
        println!("✅ Basic get operation succeeded with monitoring disabled");
        Ok::<(), F4KvsError>(())
    })
    .await;

    match disabled_result {
        Ok(Ok(())) => println!("✅ Monitoring disabled config works - deadlock avoided!"),
        Ok(Err(e)) => println!(
            "❌ Core operation failed even with monitoring disabled: {:?}",
            e
        ),
        Err(_) => {
            println!("🚨 STILL HANGING even with monitoring disabled!");
            println!("This indicates a different issue beyond monitoring");
            panic!("F4KVSCore operations still hang even with monitoring disabled");
        }
    }

    // Test 3: Memory operations with monitoring disabled
    println!("Testing memory operations with monitoring disabled...");
    let memory_result = tokio::time::timeout(Duration::from_secs(10), async {
        let config = Config {
            enable_monitoring: false,
            enable_memory_leak_detection: false,
            ..Default::default()
        };
        let core = F4KVSCore::with_config(config)?;
        for i in 0..100 {
            let key = format!("memory_test_key_{}", i);
            let value_data = vec![i as u8; 1024]; // 1KB values
            let value = Value::Bytes(value_data);
            core.put(&key, &value).await?;
        }
        println!("✅ Memory operations completed successfully");
        Ok::<(), F4KvsError>(())
    })
    .await;

    match memory_result {
        Ok(Ok(())) => println!("✅ Memory operations work with monitoring disabled"),
        Ok(Err(e)) => println!("❌ Memory operation failed: {:?}", e),
        Err(_) => {
            println!("🚨 HANG DETECTED in memory operations!");
            println!("This indicates the storage layer itself has issues");
            panic!("Memory operations hang - storage layer deadlock");
        }
    }
}

#[tokio::test]
async fn test_minimal_core_config() {
    println!("Testing F4KVSCore with explicit minimal configuration...");

    let config = Config {
        enable_monitoring: false,
        enable_memory_leak_detection: false,
        ..Default::default()
    };

    let core_result = F4KVSCore::with_config(config);
    match core_result {
        Ok(core) => {
            println!("✅ F4KVSCore created successfully with minimal config");
            // Test a simple synchronous operation
            let stats = core.stats().await;
            match stats {
                Ok(_) => println!("✅ Stats operation works"),
                Err(e) => println!("❌ Stats operation failed: {:?}", e),
            }
        }
        Err(e) => {
            println!("❌ Failed to create F4KVSCore with minimal config: {:?}", e);
            panic!("Cannot create core even with minimal config");
        }
    }
}
