//! Utilities for F4KVS Core

use crate::{Config, F4KvsError, Result};

/// Validate a key according to F4KVS rules
pub fn validate_key(key: &str, config: &Config) -> Result<()> {
    // Check length
    if key.is_empty() {
        return Err(F4KvsError::invalid_key("key cannot be empty"));
    }

    if key.len() > config.max_key_size {
        return Err(F4KvsError::invalid_key(format!(
            "key length {} exceeds maximum {}",
            key.len(),
            config.max_key_size
        )));
    }

    // Check for invalid characters if strict validation is enabled
    if config.strict_key_validation && !key.is_ascii() {
        // For now, we're strict and only allow ASCII keys for simplicity
        return Err(F4KvsError::invalid_key("key must be ASCII"));
    }

    // Check for invalid characters (control characters)
    if key.chars().any(|c| c.is_control()) {
        return Err(F4KvsError::invalid_key("key contains control characters"));
    }

    Ok(())
}

/// Simple value size check (validation is done in the engine)
pub fn get_value_size(value: &crate::Value) -> usize {
    value.memory_size()
}

/// Generate a simple hash for a key (for internal use)
pub fn hash_key(key: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Format bytes as human readable string
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Get current timestamp as milliseconds since Unix epoch
pub fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key() {
        let config = Config::default();

        // Valid keys
        assert!(validate_key("test", &config).is_ok());
        assert!(validate_key("user:123", &config).is_ok());
        assert!(validate_key("a".repeat(1024).as_str(), &config).is_ok());

        // Invalid keys
        assert!(validate_key("", &config).is_err()); // empty
        assert!(validate_key(&"a".repeat(1025), &config).is_err()); // too long
        assert!(validate_key("test\0null", &config).is_err()); // control character
    }

    #[test]
    fn test_get_value_size() {
        assert!(get_value_size(&crate::Value::String("test".to_string())) > 0);
        assert!(get_value_size(&crate::Value::Int64(42)) > 0);
        assert_eq!(get_value_size(&crate::Value::Null), 0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1536), "1.50 KB");
    }

    #[test]
    fn test_hash_key() {
        let hash1 = hash_key("test");
        let hash2 = hash_key("test");
        let hash3 = hash_key("other");

        assert_eq!(hash1, hash2); // same key = same hash
        assert_ne!(hash1, hash3); // different keys = different hash (usually)
    }
}
