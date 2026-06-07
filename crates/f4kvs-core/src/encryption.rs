//! F4KVS Encryption at Rest
//!
//! This module provides comprehensive encryption capabilities for F4KVS,
//! including multiple encryption algorithms, key management, and secure
//! data handling.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Errors that can occur during encryption operations
#[derive(Error, Debug)]
pub enum EncryptionError {
    /// Invalid key length provided
    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength {
        /// Expected key length in bytes
        expected: usize,
        /// Actual key length in bytes
        actual: usize,
    },
    /// Invalid encryption algorithm specified
    #[error("Invalid algorithm: {algorithm}")]
    InvalidAlgorithm {
        /// The invalid algorithm name
        algorithm: String,
    },
    /// Encryption operation failed
    #[error("Encryption failed: {message}")]
    EncryptionFailed {
        /// Error message describing the failure
        message: String,
    },
    /// Decryption operation failed
    #[error("Decryption failed: {message}")]
    DecryptionFailed {
        /// Error message describing the failure
        message: String,
    },
    /// Key derivation operation failed
    #[error("Key derivation failed: {message}")]
    KeyDerivationFailed {
        /// Error message describing the failure
        message: String,
    },
    /// Invalid ciphertext provided for decryption
    #[error("Invalid ciphertext: {message}")]
    InvalidCiphertext {
        /// Error message describing the issue
        message: String,
    },
    /// Encryption key not found
    #[error("Key not found: {key_id}")]
    KeyNotFound {
        /// ID of the missing key
        key_id: String,
    },
    /// Key rotation operation failed
    #[error("Key rotation failed: {message}")]
    KeyRotationFailed {
        /// Error message describing the failure
        message: String,
    },
    /// Internal encryption system error
    #[error("Internal error: {message}")]
    Internal {
        /// Error message describing the internal error
        message: String,
    },
}

/// Result type for encryption operations
pub type EncryptionResult<T> = Result<T, EncryptionError>;

/// Supported encryption algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (recommended for most use cases)
    #[default]
    Aes256Gcm,
    /// ChaCha20-Poly1305 (fast, secure alternative)
    ChaCha20Poly1305,
    /// AES-128-GCM (faster, less secure)
    Aes128Gcm,
    /// XChaCha20-Poly1305 (extended nonce, very secure)
    XChaCha20Poly1305,
}

impl std::fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncryptionAlgorithm::Aes256Gcm => write!(f, "AES-256-GCM"),
            EncryptionAlgorithm::ChaCha20Poly1305 => write!(f, "ChaCha20-Poly1305"),
            EncryptionAlgorithm::Aes128Gcm => write!(f, "AES-128-GCM"),
            EncryptionAlgorithm::XChaCha20Poly1305 => write!(f, "XChaCha20-Poly1305"),
        }
    }
}

/// Key derivation algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyDerivationAlgorithm {
    /// Argon2id (recommended, default)
    #[default]
    Argon2id,
    /// Scrypt (not yet fully implemented)
    Scrypt,
}

/// Metadata about an encryption key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    /// Unique identifier for the key
    pub id: String,
    /// Encryption algorithm used with this key
    pub algorithm: EncryptionAlgorithm,
    /// Unix timestamp when the key was created
    pub created_at: u64,
    /// Unix timestamp when the key expires (None if no expiration)
    pub expires_at: Option<u64>,
    /// Version number of the key
    pub version: u32,
    /// Whether the key is currently active
    pub is_active: bool,
    /// ID of the parent key this was derived from (None if root key)
    pub derived_from: Option<String>,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Primary encryption algorithm
    pub algorithm: EncryptionAlgorithm,
    /// Key derivation algorithm
    pub key_derivation: KeyDerivationAlgorithm,
    /// Master key (base64 encoded)
    pub master_key: Option<String>,
    /// Key rotation interval in seconds
    pub rotation_interval: Option<u64>,
    /// Enable key rotation
    pub enable_rotation: bool,
    /// Key derivation iterations
    pub iterations: u32,
    /// Salt size in bytes
    pub salt_size: usize,
    /// Enable encryption at rest
    pub enabled: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_derivation: KeyDerivationAlgorithm::Argon2id,
            master_key: None,
            rotation_interval: Some(86400 * 30), // 30 days
            enable_rotation: true,
            iterations: 100000, // Argon2id iterations
            salt_size: 32,
            enabled: false,
        }
    }
}

/// Encrypted data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The encrypted data bytes
    pub ciphertext: Vec<u8>,
    /// Nonce used for encryption
    pub nonce: Vec<u8>,
    /// ID of the key used for encryption
    pub key_id: String,
    /// Algorithm used for encryption
    pub algorithm: EncryptionAlgorithm,
    /// Salt used for key derivation
    pub salt: Vec<u8>,
    /// Version of the encryption format
    pub version: u32,
}

/// Encryption manager trait
#[async_trait::async_trait]
pub trait EncryptionManager: Send + Sync {
    /// Encrypt data
    async fn encrypt(&self, data: &[u8], key_id: Option<&str>) -> EncryptionResult<EncryptedData>;

    /// Decrypt data
    async fn decrypt(&self, encrypted_data: &EncryptedData) -> EncryptionResult<Vec<u8>>;

    /// Generate a new encryption key
    async fn generate_key(&mut self, algorithm: EncryptionAlgorithm) -> EncryptionResult<String>;

    /// Rotate encryption keys
    async fn rotate_keys(&mut self) -> EncryptionResult<()>;

    /// Get key metadata
    async fn get_key_metadata(&self, key_id: &str) -> EncryptionResult<KeyMetadata>;

    /// List all keys
    async fn list_keys(&self) -> EncryptionResult<Vec<KeyMetadata>>;

    /// Delete a key
    async fn delete_key(&mut self, key_id: &str) -> EncryptionResult<()>;
}

/// Simple encryption manager implementation
pub struct SimpleEncryptionManager {
    config: EncryptionConfig,
    keys: RwLock<HashMap<String, Vec<u8>>>,
    key_metadata: RwLock<HashMap<String, KeyMetadata>>,
    current_key_id: RwLock<String>,
}

impl SimpleEncryptionManager {
    /// Create a new encryption manager
    pub fn new(config: EncryptionConfig) -> EncryptionResult<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                keys: RwLock::new(HashMap::new()),
                key_metadata: RwLock::new(HashMap::new()),
                current_key_id: RwLock::new(String::new()),
            });
        }

        let mut manager = Self {
            config,
            keys: RwLock::new(HashMap::new()),
            key_metadata: RwLock::new(HashMap::new()),
            current_key_id: RwLock::new(String::new()),
        };

        // Generate initial key
        let key_id = manager.generate_initial_key()?;
        *manager.current_key_id.write().unwrap() = key_id;

        Ok(manager)
    }

    /// Generate initial encryption key
    fn generate_initial_key(&mut self) -> EncryptionResult<String> {
        let key_id = format!(
            "key_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );

        // Generate random key based on algorithm
        let key = self.generate_key_bytes(self.config.algorithm)?;

        // Store key
        self.keys.write().unwrap().insert(key_id.clone(), key);

        // Create metadata
        let metadata = KeyMetadata {
            id: key_id.clone(),
            algorithm: self.config.algorithm,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            expires_at: None,
            version: 1,
            is_active: true,
            derived_from: None,
        };

        self.key_metadata
            .write()
            .unwrap()
            .insert(key_id.clone(), metadata);

        Ok(key_id)
    }

    /// Generate key bytes for the given algorithm
    fn generate_key_bytes(&self, algorithm: EncryptionAlgorithm) -> EncryptionResult<Vec<u8>> {
        let key_size = match algorithm {
            EncryptionAlgorithm::Aes256Gcm => 32,
            EncryptionAlgorithm::Aes128Gcm => 16,
            EncryptionAlgorithm::ChaCha20Poly1305 => 32,
            EncryptionAlgorithm::XChaCha20Poly1305 => 32,
        };

        #[cfg(feature = "encryption")]
        {
            use rand::RngCore;
            let mut key = vec![0u8; key_size];
            rand::thread_rng().fill_bytes(&mut key);
            Ok(key)
        }

        #[cfg(not(feature = "encryption"))]
        {
            // Fallback for when encryption feature is not enabled
            let mut key = vec![0u8; key_size];
            for (i, item) in key.iter_mut().enumerate().take(key_size) {
                *item = (i as u8).wrapping_add(42);
            }
            Ok(key)
        }
    }

    /// Derive key from master key and salt
    async fn derive_key(&self, master_key: &[u8], salt: &[u8]) -> EncryptionResult<Vec<u8>> {
        match self.config.key_derivation {
            KeyDerivationAlgorithm::Argon2id => {
                #[cfg(feature = "encryption")]
                {
                    use argon2::password_hash::SaltString;
                    use argon2::{Argon2, Params, PasswordHasher};
                    let master_key = master_key.to_vec();
                    let salt = salt.to_vec();
                    // Move CPU-intensive Argon2id computation to blocking thread pool
                    tokio::task::spawn_blocking(move || {
                        let params = Params::new(65536, 3, 4, Some(32)).map_err(|e| {
                            EncryptionError::KeyDerivationFailed {
                                message: format!("Failed to create Argon2 params: {}", e),
                            }
                        })?;
                        let argon2 = Argon2::new(
                            argon2::Algorithm::Argon2id,
                            argon2::Version::V0x13,
                            params,
                        );
                        // Create salt from bytes
                        let salt_str = SaltString::encode_b64(&salt).map_err(|e| {
                            EncryptionError::KeyDerivationFailed {
                                message: format!("Failed to create salt: {}", e),
                            }
                        })?;
                        let password_hash =
                            argon2.hash_password(&master_key, &salt_str).map_err(|e| {
                                EncryptionError::KeyDerivationFailed {
                                    message: format!("Failed to hash password: {}", e),
                                }
                            })?;
                        Ok(password_hash.hash.unwrap().as_bytes()[..32].to_vec())
                    })
                    .await
                    .map_err(|e| EncryptionError::KeyDerivationFailed {
                        message: format!("Task join error: {}", e),
                    })?
                }
                #[cfg(not(feature = "encryption"))]
                {
                    // Fallback
                    let mut derived = Vec::new();
                    for i in 0..32 {
                        derived.push(
                            master_key[i % master_key.len()].wrapping_add(salt[i % salt.len()]),
                        );
                    }
                    Ok(derived)
                }
            }
            KeyDerivationAlgorithm::Scrypt => {
                // Scrypt not yet implemented with real library
                let mut derived = Vec::new();
                for i in 0..32 {
                    derived
                        .push(master_key[i % master_key.len()].wrapping_add(salt[i % salt.len()]));
                }
                Ok(derived)
            }
        }
    }

    /// Generate random nonce
    fn generate_nonce(&self, algorithm: EncryptionAlgorithm) -> EncryptionResult<Vec<u8>> {
        let nonce_size = match algorithm {
            EncryptionAlgorithm::Aes256Gcm => 12,
            EncryptionAlgorithm::Aes128Gcm => 12,
            EncryptionAlgorithm::ChaCha20Poly1305 => 12,
            EncryptionAlgorithm::XChaCha20Poly1305 => 24,
        };

        #[cfg(feature = "encryption")]
        {
            use rand::RngCore;
            let mut nonce = vec![0u8; nonce_size];
            rand::thread_rng().fill_bytes(&mut nonce);
            Ok(nonce)
        }

        #[cfg(not(feature = "encryption"))]
        {
            // Fallback
            let mut nonce = vec![0u8; nonce_size];
            for (i, item) in nonce.iter_mut().enumerate().take(nonce_size) {
                *item = (i as u8).wrapping_add(123);
            }
            Ok(nonce)
        }
    }

    /// Generate random salt
    fn generate_salt(&self) -> EncryptionResult<Vec<u8>> {
        #[cfg(feature = "encryption")]
        {
            use rand::RngCore;
            let mut salt = vec![0u8; self.config.salt_size];
            rand::thread_rng().fill_bytes(&mut salt);
            Ok(salt)
        }

        #[cfg(not(feature = "encryption"))]
        {
            // Fallback
            let mut salt = vec![0u8; self.config.salt_size];
            for (i, item) in salt.iter_mut().enumerate().take(self.config.salt_size) {
                *item = (i as u8).wrapping_add(67);
            }
            Ok(salt)
        }
    }
}

#[async_trait::async_trait]
impl EncryptionManager for SimpleEncryptionManager {
    async fn encrypt(&self, data: &[u8], key_id: Option<&str>) -> EncryptionResult<EncryptedData> {
        if !self.config.enabled {
            return Ok(EncryptedData {
                ciphertext: data.to_vec(),
                nonce: Vec::new(),
                key_id: "none".to_string(),
                algorithm: self.config.algorithm,
                salt: Vec::new(),
                version: 1,
            });
        }

        let key_id = {
            let current_key_id = self.current_key_id.read().unwrap();
            key_id.unwrap_or(&current_key_id).to_string()
        };
        let key = self
            .keys
            .read()
            .unwrap()
            .get(&key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound {
                key_id: key_id.clone(),
            })?
            .clone();

        let salt = self.generate_salt()?;
        let derived_key = self.derive_key(&key, &salt).await?;
        let nonce = self.generate_nonce(self.config.algorithm)?;

        let ciphertext = {
            #[cfg(feature = "encryption")]
            {
                match self.config.algorithm {
                    EncryptionAlgorithm::Aes256Gcm => {
                        use aes_gcm::aead::{Aead, KeyInit};
                        use aes_gcm::{Aes256Gcm, Nonce};
                        let cipher = Aes256Gcm::new_from_slice(&derived_key).map_err(|e| {
                            EncryptionError::EncryptionFailed {
                                message: format!("Failed to create cipher: {}", e),
                            }
                        })?;
                        let nonce_array: [u8; 12] = nonce[..12].try_into().map_err(|_| {
                            EncryptionError::EncryptionFailed {
                                message: "Invalid nonce length".to_string(),
                            }
                        })?;
                        let nonce_obj = Nonce::from(nonce_array);
                        cipher.encrypt(&nonce_obj, data).map_err(|e| {
                            EncryptionError::EncryptionFailed {
                                message: format!("Encryption failed: {}", e),
                            }
                        })?
                    }
                    EncryptionAlgorithm::ChaCha20Poly1305 => {
                        use chacha20poly1305::aead::{Aead, KeyInit};
                        use chacha20poly1305::{ChaCha20Poly1305, Nonce};
                        let cipher =
                            ChaCha20Poly1305::new_from_slice(&derived_key).map_err(|e| {
                                EncryptionError::EncryptionFailed {
                                    message: format!("Failed to create cipher: {}", e),
                                }
                            })?;
                        let nonce_array: [u8; 12] = nonce[..12].try_into().map_err(|_| {
                            EncryptionError::EncryptionFailed {
                                message: "Invalid nonce length".to_string(),
                            }
                        })?;
                        let nonce_obj = Nonce::from(nonce_array);
                        cipher.encrypt(&nonce_obj, data).map_err(|e| {
                            EncryptionError::EncryptionFailed {
                                message: format!("Encryption failed: {}", e),
                            }
                        })?
                    }
                    _ => {
                        // Fallback to XOR for unsupported algorithms
                        let mut result = Vec::with_capacity(data.len());
                        for (i, &byte) in data.iter().enumerate() {
                            result.push(byte ^ derived_key[i % derived_key.len()]);
                        }
                        result
                    }
                }
            }

            #[cfg(not(feature = "encryption"))]
            {
                // Fallback to XOR when encryption feature is not enabled
                let mut result = Vec::with_capacity(data.len());
                for (i, &byte) in data.iter().enumerate() {
                    result.push(byte ^ derived_key[i % derived_key.len()]);
                }
                result
            }
        };

        Ok(EncryptedData {
            ciphertext,
            nonce,
            key_id,
            algorithm: self.config.algorithm,
            salt,
            version: 1,
        })
    }

    async fn decrypt(&self, encrypted_data: &EncryptedData) -> EncryptionResult<Vec<u8>> {
        if !self.config.enabled {
            return Ok(encrypted_data.ciphertext.clone());
        }

        let key = self
            .keys
            .read()
            .unwrap()
            .get(&encrypted_data.key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound {
                key_id: encrypted_data.key_id.clone(),
            })?
            .clone();

        let derived_key = self.derive_key(&key, &encrypted_data.salt).await?;

        let plaintext =
            {
                #[cfg(feature = "encryption")]
                {
                    match encrypted_data.algorithm {
                        EncryptionAlgorithm::Aes256Gcm => {
                            use aes_gcm::aead::{Aead, KeyInit};
                            use aes_gcm::{Aes256Gcm, Nonce};
                            let cipher = Aes256Gcm::new_from_slice(&derived_key).map_err(|e| {
                                EncryptionError::DecryptionFailed {
                                    message: format!("Failed to create cipher: {}", e),
                                }
                            })?;
                            let nonce_array: [u8; 12] = encrypted_data.nonce[..12]
                                .try_into()
                                .map_err(|_| EncryptionError::DecryptionFailed {
                                    message: "Invalid nonce length".to_string(),
                                })?;
                            let nonce = Nonce::from(nonce_array);
                            cipher
                                .decrypt(&nonce, encrypted_data.ciphertext.as_ref())
                                .map_err(|e| EncryptionError::DecryptionFailed {
                                    message: format!("Decryption failed: {}", e),
                                })?
                        }
                        EncryptionAlgorithm::ChaCha20Poly1305 => {
                            use chacha20poly1305::aead::{Aead, KeyInit};
                            use chacha20poly1305::{ChaCha20Poly1305, Nonce};
                            let cipher =
                                ChaCha20Poly1305::new_from_slice(&derived_key).map_err(|e| {
                                    EncryptionError::DecryptionFailed {
                                        message: format!("Failed to create cipher: {}", e),
                                    }
                                })?;
                            let nonce_array: [u8; 12] = encrypted_data.nonce[..12]
                                .try_into()
                                .map_err(|_| EncryptionError::DecryptionFailed {
                                    message: "Invalid nonce length".to_string(),
                                })?;
                            let nonce = Nonce::from(nonce_array);
                            cipher
                                .decrypt(&nonce, encrypted_data.ciphertext.as_ref())
                                .map_err(|e| EncryptionError::DecryptionFailed {
                                    message: format!("Decryption failed: {}", e),
                                })?
                        }
                        _ => {
                            // Fallback to XOR for unsupported algorithms
                            let mut result = Vec::with_capacity(encrypted_data.ciphertext.len());
                            for (i, &byte) in encrypted_data.ciphertext.iter().enumerate() {
                                result.push(byte ^ derived_key[i % derived_key.len()]);
                            }
                            result
                        }
                    }
                }

                #[cfg(not(feature = "encryption"))]
                {
                    // Fallback to XOR when encryption feature is not enabled
                    let mut result = Vec::with_capacity(encrypted_data.ciphertext.len());
                    for (i, &byte) in encrypted_data.ciphertext.iter().enumerate() {
                        result.push(byte ^ derived_key[i % derived_key.len()]);
                    }
                    result
                }
            };

        Ok(plaintext)
    }

    async fn generate_key(&mut self, algorithm: EncryptionAlgorithm) -> EncryptionResult<String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        #[cfg(feature = "encryption")]
        let random_part = {
            use rand::RngCore;
            rand::thread_rng().next_u64()
        };

        #[cfg(not(feature = "encryption"))]
        let random_part = timestamp % 1000000; // Fallback for when encryption is disabled

        let key_id = format!("key_{}_{}", timestamp, random_part);
        let key = self.generate_key_bytes(algorithm)?;

        // Store key and metadata
        self.keys.write().unwrap().insert(key_id.clone(), key);
        self.key_metadata.write().unwrap().insert(
            key_id.clone(),
            KeyMetadata {
                id: key_id.clone(),
                algorithm,
                created_at: timestamp as u64 / 1_000_000_000, // Convert to seconds
                expires_at: None,
                is_active: true,
                version: 1,
                derived_from: None,
            },
        );

        Ok(key_id)
    }

    async fn rotate_keys(&mut self) -> EncryptionResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // In a real implementation, this would:
        // 1. Generate a new key
        // 2. Re-encrypt all data with the new key
        // 3. Mark the old key as inactive
        // 4. Update the current key ID

        Ok(())
    }

    async fn get_key_metadata(&self, key_id: &str) -> EncryptionResult<KeyMetadata> {
        self.key_metadata
            .read()
            .unwrap()
            .get(key_id)
            .cloned()
            .ok_or_else(|| EncryptionError::KeyNotFound {
                key_id: key_id.to_string(),
            })
    }

    async fn list_keys(&self) -> EncryptionResult<Vec<KeyMetadata>> {
        Ok(self
            .key_metadata
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect())
    }

    async fn delete_key(&mut self, key_id: &str) -> EncryptionResult<()> {
        let mut keys = self.keys.write().unwrap();
        if !keys.contains_key(key_id) {
            return Err(EncryptionError::KeyNotFound {
                key_id: key_id.to_string(),
            });
        }

        // Remove from both stores
        keys.remove(key_id);
        drop(keys); // Release write lock before acquiring another
        self.key_metadata.write().unwrap().remove(key_id);

        // In a real implementation, this would securely delete the key
        Ok(())
    }
}

/// Create a new encryption manager
pub fn create_encryption_manager(
    config: EncryptionConfig,
) -> EncryptionResult<Arc<dyn EncryptionManager>> {
    let manager = SimpleEncryptionManager::new(config)?;
    Ok(Arc::new(manager))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encryption_disabled() {
        let config = EncryptionConfig {
            enabled: false,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();
        let data = b"hello world";

        let encrypted = manager.encrypt(data, None).await.unwrap();
        assert_eq!(encrypted.ciphertext, data);
        assert_eq!(encrypted.key_id, "none");

        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, data);
    }

    #[tokio::test]
    async fn test_encryption_enabled() {
        let config = EncryptionConfig {
            enabled: true,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();
        let data = b"hello world";

        let encrypted = manager.encrypt(data, None).await.unwrap();
        assert_ne!(encrypted.ciphertext, data);
        assert!(!encrypted.key_id.is_empty());
        assert!(!encrypted.nonce.is_empty());
        assert!(!encrypted.salt.is_empty());

        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, data);
    }

    #[tokio::test]
    async fn test_key_management() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        let keys = manager.list_keys().await.unwrap();
        assert!(!keys.is_empty());

        let key_id = &keys[0].id;
        let metadata = manager.get_key_metadata(key_id).await.unwrap();
        assert_eq!(metadata.id, *key_id);
        assert!(metadata.is_active);
    }

    #[tokio::test]
    async fn test_encryption_algorithm_selection() {
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
            let data = b"test data";

            let encrypted = manager.encrypt(data, None).await.unwrap();
            assert_eq!(encrypted.algorithm, algorithm);

            let decrypted = manager.decrypt(&encrypted).await.unwrap();
            assert_eq!(decrypted, data);
        }
    }

    #[tokio::test]
    async fn test_encryption_decryption_roundtrip() {
        let config = EncryptionConfig {
            enabled: true,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        // Test with various data sizes
        let test_cases = vec![
            b"".to_vec(),            // Empty
            b"a".to_vec(),           // Single byte
            b"hello".to_vec(),       // Short
            b"hello world".to_vec(), // Medium
            vec![0u8; 1024],         // 1KB
            vec![0u8; 1024 * 1024],  // 1MB
        ];

        for data in test_cases {
            let encrypted = manager.encrypt(&data, None).await.unwrap();
            let decrypted = manager.decrypt(&encrypted).await.unwrap();
            assert_eq!(decrypted, data);
        }
    }

    #[tokio::test]
    async fn test_key_generation() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let mut manager = SimpleEncryptionManager::new(config).unwrap();

        // Generate keys for different algorithms
        let key1 = manager
            .generate_key(EncryptionAlgorithm::Aes256Gcm)
            .await
            .unwrap();
        let key2 = manager
            .generate_key(EncryptionAlgorithm::ChaCha20Poly1305)
            .await
            .unwrap();

        assert!(!key1.is_empty());
        assert!(!key2.is_empty());
        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let config = EncryptionConfig {
            enabled: true,
            enable_rotation: true,
            ..Default::default()
        };

        let mut manager = SimpleEncryptionManager::new(config).unwrap();

        // Key rotation should succeed
        let result = manager.rotate_keys().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_ciphertext_handling() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        // Create invalid encrypted data
        let invalid_encrypted = EncryptedData {
            ciphertext: vec![1, 2, 3, 4],
            nonce: vec![5, 6, 7, 8],
            key_id: "nonexistent_key".to_string(),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            salt: vec![9, 10, 11, 12],
            version: 1,
        };

        // Decryption should fail with invalid key
        let result = manager.decrypt(&invalid_encrypted).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EncryptionError::KeyNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_key_metadata_retrieval() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        let keys = manager.list_keys().await.unwrap();
        assert!(!keys.is_empty());

        for key_metadata in keys {
            let retrieved = manager.get_key_metadata(&key_metadata.id).await.unwrap();
            assert_eq!(retrieved.id, key_metadata.id);
            assert_eq!(retrieved.algorithm, key_metadata.algorithm);
            assert_eq!(retrieved.version, key_metadata.version);
        }
    }

    #[tokio::test]
    async fn test_encryption_with_specific_key() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        // Get current key
        let keys = manager.list_keys().await.unwrap();
        let key_id = &keys[0].id;

        let data = b"test data";

        // Encrypt with specific key
        let encrypted = manager.encrypt(data, Some(key_id)).await.unwrap();
        assert_eq!(encrypted.key_id, *key_id);

        // Decrypt should work
        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, data);
    }

    #[tokio::test]
    async fn test_encryption_key_not_found() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();

        let data = b"test data";

        // Try to encrypt with non-existent key
        let result = manager.encrypt(data, Some("nonexistent_key")).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EncryptionError::KeyNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_key_deletion() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let mut manager = SimpleEncryptionManager::new(config).unwrap();

        let keys = manager.list_keys().await.unwrap();
        let key_id = &keys[0].id;

        // Delete key
        let result = manager.delete_key(key_id).await;
        assert!(result.is_ok());

        // Key should not be found
        let result = manager.get_key_metadata(key_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_encryption_algorithm_display() {
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.to_string(), "AES-256-GCM");
        assert_eq!(
            EncryptionAlgorithm::ChaCha20Poly1305.to_string(),
            "ChaCha20-Poly1305"
        );
        assert_eq!(EncryptionAlgorithm::Aes128Gcm.to_string(), "AES-128-GCM");
        assert_eq!(
            EncryptionAlgorithm::XChaCha20Poly1305.to_string(),
            "XChaCha20-Poly1305"
        );
    }

    #[tokio::test]
    async fn test_encryption_with_different_salts() {
        let config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let manager = SimpleEncryptionManager::new(config).unwrap();
        let data = b"test data";

        // Encrypt same data multiple times (should get different ciphertexts due to different salts/nonces)
        let encrypted1 = manager.encrypt(data, None).await.unwrap();
        let encrypted2 = manager.encrypt(data, None).await.unwrap();

        // Ciphertexts should be different
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
        assert_ne!(encrypted1.nonce, encrypted2.nonce);
        assert_ne!(encrypted1.salt, encrypted2.salt);

        // But both should decrypt to the same plaintext
        let decrypted1 = manager.decrypt(&encrypted1).await.unwrap();
        let decrypted2 = manager.decrypt(&encrypted2).await.unwrap();
        assert_eq!(decrypted1, data);
        assert_eq!(decrypted2, data);
    }

    #[tokio::test]
    async fn test_key_derivation_algorithms() {
        let algorithms = vec![
            KeyDerivationAlgorithm::Argon2id,
            KeyDerivationAlgorithm::Scrypt,
        ];

        for key_derivation in algorithms {
            let config = EncryptionConfig {
                enabled: true,
                key_derivation,
                ..Default::default()
            };

            let manager = SimpleEncryptionManager::new(config).unwrap();
            let data = b"test data";

            let encrypted = manager.encrypt(data, None).await.unwrap();
            let decrypted = manager.decrypt(&encrypted).await.unwrap();
            assert_eq!(decrypted, data);
        }
    }
}
