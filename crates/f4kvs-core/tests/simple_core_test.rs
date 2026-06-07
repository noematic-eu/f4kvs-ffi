use f4kvs_core::{F4KVSCore, Value};

#[tokio::test]
async fn test_simple_core() {
    println!("Creating F4KVSCore...");
    let core = F4KVSCore::new().unwrap();
    println!("F4KVSCore created successfully");

    println!("Testing put operation...");
    let value = Value::String("test_value".to_string());
    core.put("test_key", &value).await.unwrap();
    println!("Put operation completed");

    println!("Testing get operation...");
    let retrieved = core.get("test_key").await.unwrap();
    println!("Get operation completed: {:?}", retrieved);
}
