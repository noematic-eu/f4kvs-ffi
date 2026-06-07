//! Engine utility functions and helpers
//!
//! This module contains utility functions and helpers for the F4KVS core engine,
//! extracted to improve code organization and maintainability.

use crate::{Config, Result, Value};
// These imports are conditionally used in #[cfg(not(feature = "uuid"))] block
#[allow(unused_imports)]
use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique health check key to avoid conflicts with user data
pub fn generate_health_check_key() -> String {
    #[cfg(feature = "uuid")]
    {
        format!("__health_check_{}__", uuid::Uuid::new_v4())
    }

    #[cfg(not(feature = "uuid"))]
    {
        // Use a more efficient key generation to avoid format! allocation
        // Try to use system time, but fall back to a counter if system time fails
        // (e.g., if system time is before Unix epoch, which is extremely unlikely)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_else(|_| {
                // Fallback: use a static counter to ensure uniqueness
                // This counter starts at a high value to avoid collisions with timestamps
                // This should only happen if system time is before Unix epoch (extremely unlikely)
                static FALLBACK_COUNTER: AtomicU64 = AtomicU64::new(1_000_000_000_000_000_000);
                FALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed) as u128
            });
        format!("__health_check_{}__", timestamp)
    }
}

/// Create a test value for health checks
pub fn create_health_check_value() -> Value {
    Value::String("ok".to_string())
}

/// Generate engine features list
pub fn generate_engine_features() -> Vec<String> {
    vec![
        "async".to_string(),
        "memory_storage".to_string(),
        "validation".to_string(),
        "health_checks".to_string(),
    ]
}

/// Validate configuration for engine creation
pub fn validate_config(config: &Config) -> Result<()> {
    config.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_health_check_key() {
        let key1 = generate_health_check_key();
        let key2 = generate_health_check_key();

        assert!(key1.starts_with("__health_check_"));
        assert!(key2.starts_with("__health_check_"));
        assert_ne!(key1, key2); // Should be unique
    }

    #[test]
    fn test_create_health_check_value() {
        let value = create_health_check_value();
        assert_eq!(value, Value::String("ok".to_string()));
    }

    #[test]
    fn test_generate_engine_features() {
        let features = generate_engine_features();
        assert_eq!(features.len(), 4);
        assert!(features.contains(&"async".to_string()));
        assert!(features.contains(&"memory_storage".to_string()));
        assert!(features.contains(&"validation".to_string()));
        assert!(features.contains(&"health_checks".to_string()));
    }

    #[test]
    fn test_validate_config() {
        let config = Config::default();
        assert!(validate_config(&config).is_ok());
    }
}
