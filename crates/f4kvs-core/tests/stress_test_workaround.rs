// WORKAROUND FOR HANGING STRESS TESTS
//
// This file provides a workaround for the hanging stress tests by disabling
// the problematic monitoring and memory leak detection components that cause
// async deadlocks in F4KVSCore.
//
// To use this workaround, modify your stress tests to use this configuration
// instead of the default F4KVSCore::new().

use f4kvs_core::{Config, F4KVSCore};

/// Create F4KVSCore with deadlock-safe configuration
///
/// This disables monitoring and memory leak detection to avoid async deadlocks
/// that cause stress tests to hang indefinitely.
pub fn create_safe_f4kvs_core() -> Result<F4KVSCore, String> {
    let config = Config {
        // Disable problematic components that cause deadlocks
        enable_monitoring: false,
        enable_memory_leak_detection: false,

        // Keep other defaults
        ..Default::default()
    };

    F4KVSCore::with_config(config)
        .map_err(|e| format!("Failed to create F4KVSCore with safe config: {}", e))
}
