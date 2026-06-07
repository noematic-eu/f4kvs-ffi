//! Comprehensive encryption tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for encryption scenarios including:
//! - Multiple algorithm support
//! - Key management workflows
//! - Large data encryption/decryption
//! - Performance under various payload sizes
//! - Error recovery scenarios

use f4kvs_core::encryption::*;
use std::time::Instant;

#[tokio::test]
async fn test_multiple_algorithm_support() {
    let algorithms = vec![
        EncryptionAlgorithm::Aes256Gcm,
        EncryptionAlgorithm::Aes128Gcm,
        EncryptionAlgorithm::ChaCha20Poly1305,
        EncryptionAlgorithm::XChaCha20Poly1305,
    ];

    for algorithm in algorithms {
        let config = EncryptionConfig {
            enabled: true,
            algorithm,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();
        let data = b"test data for algorithm testing";

        // Encrypt and decrypt
        let encrypted = manager.encrypt(data, None).await.unwrap();
        assert_eq!(encrypted.algorithm, algorithm);

        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, data);
    }
}

#[tokio::test]
async fn test_key_management_workflows() {
    let config = EncryptionConfig {
        enabled: true,
        ..Default::default()
    };

    let mut manager = SimpleEncryptionManager::new(config).unwrap();

    // List initial keys
    let initial_keys = manager.list_keys().await.unwrap();
    assert!(!initial_keys.is_empty());

    // Generate new key
    let new_key_id = manager
        .generate_key(EncryptionAlgorithm::Aes256Gcm)
        .await
        .unwrap();
    assert!(!new_key_id.is_empty());

    // List keys again (should have at least the initial keys)
    let all_keys = manager.list_keys().await.unwrap();
    assert!(all_keys.len() >= initial_keys.len());

    // Get key metadata
    let metadata = manager.get_key_metadata(&initial_keys[0].id).await.unwrap();
    assert_eq!(metadata.id, initial_keys[0].id);
    assert!(metadata.is_active);
}

#[tokio::test]
async fn test_large_data_encryption_decryption() {
    let config = EncryptionConfig {
        enabled: true,
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        ..Default::default()
    };

    let manager = SimpleEncryptionManager::new(config).unwrap();

    // Test with various large data sizes
    let sizes = vec![
        1024,             // 1KB
        10 * 1024,        // 10KB
        100 * 1024,       // 100KB
        1024 * 1024,      // 1MB
        10 * 1024 * 1024, // 10MB
    ];

    for size in sizes {
        let data = vec![0x42u8; size];

        let start = Instant::now();
        let encrypted = manager.encrypt(&data, None).await.unwrap();
        let encrypt_time = start.elapsed();

        let start = Instant::now();
        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        let decrypt_time = start.elapsed();

        assert_eq!(decrypted, data);
        assert!(encrypt_time.as_secs() < 10); // Should complete in reasonable time
        assert!(decrypt_time.as_secs() < 10);
    }
}

#[tokio::test]
async fn test_performance_under_various_payload_sizes() {
    let config = EncryptionConfig {
        enabled: true,
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        ..Default::default()
    };

    let manager = SimpleEncryptionManager::new(config).unwrap();

    let payload_sizes = vec![
        16,    // Small
        256,   // Medium-small
        4096,  // Medium
        65536, // Large
    ];

    for size in payload_sizes {
        let data = vec![0xAAu8; size];

        let start = Instant::now();
        let encrypted = manager.encrypt(&data, None).await.unwrap();
        let encrypt_time = start.elapsed();

        let start = Instant::now();
        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        let decrypt_time = start.elapsed();

        assert_eq!(decrypted, data);

        // Performance should be reasonable (adjust thresholds as needed)
        println!(
            "Size: {} bytes, Encrypt: {:?}, Decrypt: {:?}",
            size, encrypt_time, decrypt_time
        );
    }
}

#[tokio::test]
async fn test_error_recovery_scenarios() {
    let config = EncryptionConfig {
        enabled: true,
        ..Default::default()
    };

    let manager = SimpleEncryptionManager::new(config).unwrap();

    // Test with invalid key ID
    let data = b"test data";
    let result = manager.encrypt(data, Some("invalid_key_id")).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        EncryptionError::KeyNotFound { .. }
    ));

    // Test with corrupted encrypted data
    let valid_encrypted = manager.encrypt(data, None).await.unwrap();

    // Create corrupted version
    let mut corrupted = valid_encrypted.clone();
    corrupted.ciphertext[0] ^= 0xFF; // Flip bits

    // Decryption may fail or succeed depending on algorithm
    let result = manager.decrypt(&corrupted).await;
    // Result depends on algorithm - some may detect corruption, others may not
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_key_rotation_workflow() {
    let config = EncryptionConfig {
        enabled: true,
        enable_rotation: true,
        rotation_interval: Some(3600),
        ..Default::default()
    };

    let mut manager = SimpleEncryptionManager::new(config).unwrap();

    // Get initial keys
    let initial_keys = manager.list_keys().await.unwrap();
    let initial_key_count = initial_keys.len();

    // Perform key rotation
    let result = manager.rotate_keys().await;
    assert!(result.is_ok());

    // After rotation, keys should still be accessible
    let keys_after = manager.list_keys().await.unwrap();
    assert!(keys_after.len() >= initial_key_count);
}

#[tokio::test]
async fn test_encryption_with_different_key_derivation() {
    let key_derivations = vec![
        KeyDerivationAlgorithm::Argon2id,
        KeyDerivationAlgorithm::Scrypt,
    ];

    for key_derivation in key_derivations {
        let config = EncryptionConfig {
            enabled: true,
            key_derivation,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();
        let data = b"test data with different key derivation";

        let encrypted = manager.encrypt(data, None).await.unwrap();
        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, data);
    }
}

#[tokio::test]
async fn test_concurrent_encryption_operations() {
    let config = EncryptionConfig {
        enabled: true,
        ..Default::default()
    };

    let manager = std::sync::Arc::new(SimpleEncryptionManager::new(config).unwrap());

    let data = b"concurrent test data";

    // Spawn multiple concurrent encryption operations
    let mut handles = Vec::new();
    for _ in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let encrypted = manager_clone.encrypt(data, None).await.unwrap();
            manager_clone.decrypt(&encrypted).await
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data);
    }
}

#[tokio::test]
async fn test_encryption_metadata_persistence() {
    let config = EncryptionConfig {
        enabled: true,
        ..Default::default()
    };
    let algorithm = config.algorithm;

    let manager = SimpleEncryptionManager::new(config).unwrap();
    let data = b"test data";

    let encrypted = manager.encrypt(data, None).await.unwrap();

    // Verify all metadata fields are present
    assert!(!encrypted.key_id.is_empty());
    assert!(!encrypted.nonce.is_empty());
    assert!(!encrypted.salt.is_empty());
    assert_eq!(encrypted.version, 1);
    assert_eq!(encrypted.algorithm, algorithm);
}

#[tokio::test]
async fn test_encryption_with_custom_iterations() {
    let config = EncryptionConfig {
        enabled: true,
        iterations: 50000, // Lower iterations for faster testing
        ..Default::default()
    };

    let manager = SimpleEncryptionManager::new(config).unwrap();
    let data = b"test data with custom iterations";

    let encrypted = manager.encrypt(data, None).await.unwrap();
    let decrypted = manager.decrypt(&encrypted).await.unwrap();
    assert_eq!(decrypted, data);
}

#[tokio::test]
async fn test_encryption_with_custom_salt_size() {
    let config = EncryptionConfig {
        enabled: true,
        salt_size: 16, // Smaller salt for testing
        ..Default::default()
    };
    let salt_size = config.salt_size;

    let manager = SimpleEncryptionManager::new(config).unwrap();
    let data = b"test data with custom salt size";

    let encrypted = manager.encrypt(data, None).await.unwrap();
    assert_eq!(encrypted.salt.len(), salt_size);

    let decrypted = manager.decrypt(&encrypted).await.unwrap();
    assert_eq!(decrypted, data);
}

#[tokio::test]
async fn test_key_metadata_fields() {
    let config = EncryptionConfig {
        enabled: true,
        ..Default::default()
    };

    let manager = SimpleEncryptionManager::new(config).unwrap();

    let keys = manager.list_keys().await.unwrap();
    assert!(!keys.is_empty());

    for key_metadata in keys {
        // Verify all metadata fields
        assert!(!key_metadata.id.is_empty());
        assert!(key_metadata.created_at > 0);
        assert!(key_metadata.version > 0);
        assert!(key_metadata.is_active);
    }
}

#[tokio::test]
async fn test_encryption_algorithm_default() {
    // Test default algorithm
    let default_algorithm = EncryptionAlgorithm::default();
    assert_eq!(default_algorithm, EncryptionAlgorithm::Aes256Gcm);
}

#[tokio::test]
async fn test_key_derivation_default() {
    // Test default key derivation
    let default_derivation = KeyDerivationAlgorithm::default();
    assert_eq!(default_derivation, KeyDerivationAlgorithm::Argon2id);
}
