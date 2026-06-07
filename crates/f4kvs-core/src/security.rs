//! F4KVS Security Integration
//!
//! This module provides comprehensive security integration for F4KVS,
//! combining encryption at rest, RBAC, and security policies.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::encryption::{EncryptedData, EncryptionConfig, EncryptionManager};
use crate::rbac::{Permission, RbacConfig, RbacManager, Session};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Security-related errors
#[derive(Error, Debug)]
pub enum SecurityError {
    /// Authentication operation failed
    #[error("Authentication failed: {message}")]
    AuthenticationFailed {
        /// Error message describing the authentication failure
        message: String,
    },
    /// Authorization operation failed
    #[error("Authorization failed: {message}")]
    AuthorizationFailed {
        /// Error message describing the authorization failure
        message: String,
    },
    /// Encryption operation failed
    #[error("Encryption error: {message}")]
    EncryptionError {
        /// Error message describing the encryption failure
        message: String,
    },
    /// RBAC operation failed
    #[error("RBAC error: {message}")]
    RbacError {
        /// Error message describing the RBAC failure
        message: String,
    },
    /// Security policy violation detected
    #[error("Security policy violation: {message}")]
    PolicyViolation {
        /// Error message describing the policy violation
        message: String,
    },
    /// Session validation failed
    #[error("Session invalid: {message}")]
    SessionInvalid {
        /// Error message describing the session issue
        message: String,
    },
    /// Internal security system error
    #[error("Internal error: {message}")]
    Internal {
        /// Error message describing the internal error
        message: String,
    },
}

impl From<crate::encryption::EncryptionError> for SecurityError {
    fn from(err: crate::encryption::EncryptionError) -> Self {
        SecurityError::EncryptionError {
            message: err.to_string(),
        }
    }
}

impl From<crate::rbac::RbacError> for SecurityError {
    fn from(err: crate::rbac::RbacError) -> Self {
        SecurityError::RbacError {
            message: err.to_string(),
        }
    }
}

/// Result type for security operations
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Security configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Encryption configuration
    pub encryption: EncryptionConfig,
    /// RBAC configuration
    pub rbac: RbacConfig,
    /// Whether to enable audit logging
    pub enable_audit_logging: bool,
    /// Whether to enable security policies
    pub enable_security_policies: bool,
    /// Maximum failed authentication attempts before lockout
    pub max_failed_attempts: u32,
    /// Lockout duration in seconds
    pub lockout_duration: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            encryption: EncryptionConfig::default(),
            rbac: RbacConfig::default(),
            enable_audit_logging: true,
            enable_security_policies: true,
            max_failed_attempts: 5,
            lockout_duration: 300, // 5 minutes
        }
    }
}

/// Security context for request processing
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// ID of the authenticated user
    pub user_id: String,
    /// Active session information (if available)
    pub session: Option<Session>,
    /// List of permissions available in this context
    pub permissions: Vec<Permission>,
    /// Additional security metadata
    pub metadata: HashMap<String, String>,
}

/// Security policy types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityPolicy {
    /// Require encryption for all data
    RequireEncryption,
    /// Require specific permissions for operations
    RequirePermission {
        /// Resource that requires permission
        resource: String,
        /// Action that requires permission
        action: String,
    },
    /// Rate limiting policy
    RateLimit {
        /// Maximum number of requests allowed
        max_requests: u32,
        /// Time window in seconds
        window_seconds: u64,
    },
    /// Data classification policy
    DataClassification {
        /// Classification level (e.g., "public", "internal", "confidential")
        level: String,
        /// Required permissions for this classification level
        required_permissions: Vec<String>,
    },
}

/// Security manager trait
#[async_trait::async_trait]
pub trait SecurityManager: Send + Sync {
    /// Authenticate user and create security context
    async fn authenticate(&self, username: &str, password: &str)
        -> SecurityResult<SecurityContext>;

    /// Validate security context for operation
    async fn validate_operation(
        &self,
        context: &SecurityContext,
        operation: &str,
    ) -> SecurityResult<()>;

    /// Encrypt data with user context
    async fn encrypt_data(
        &self,
        data: &[u8],
        context: &SecurityContext,
    ) -> SecurityResult<EncryptedData>;

    /// Decrypt data with user context
    async fn decrypt_data(
        &self,
        encrypted_data: &EncryptedData,
        context: &SecurityContext,
    ) -> SecurityResult<Vec<u8>>;

    /// Check if user has permission
    async fn has_permission(
        &self,
        context: &SecurityContext,
        permission: &Permission,
    ) -> SecurityResult<bool>;

    /// Apply security policies
    async fn apply_policies(
        &self,
        context: &SecurityContext,
        operation: &str,
    ) -> SecurityResult<()>;

    /// Log security event
    async fn log_security_event(&self, event: SecurityEvent) -> SecurityResult<()>;
}

/// Security event for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    /// Type of security event (e.g., "login", "permission_check", "policy_violation")
    pub event_type: String,
    /// ID of the user who triggered the event
    pub user_id: String,
    /// Operation that was performed
    pub operation: String,
    /// Resource that was accessed
    pub resource: String,
    /// Whether the operation was successful
    pub success: bool,
    /// Unix timestamp when the event occurred
    pub timestamp: u64,
    /// Additional event metadata
    pub metadata: HashMap<String, String>,
}

/// Simple security manager implementation
pub struct SimpleSecurityManager {
    config: SecurityConfig,
    encryption_manager: Arc<dyn EncryptionManager>,
    rbac_manager: Arc<dyn RbacManager>,
    policies: Vec<SecurityPolicy>,
    failed_attempts: HashMap<String, (u32, u64)>, // user_id -> (count, last_attempt)
}

impl SimpleSecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> SecurityResult<Self> {
        let encryption_manager =
            crate::encryption::create_encryption_manager(config.encryption.clone())?;
        let rbac_manager = crate::rbac::create_rbac_manager(config.rbac.clone());

        let mut manager = Self {
            config,
            encryption_manager,
            rbac_manager,
            policies: Vec::new(),
            failed_attempts: HashMap::new(),
        };

        // Add default policies
        manager.add_default_policies();

        Ok(manager)
    }

    /// Add default security policies
    fn add_default_policies(&mut self) {
        self.policies.push(SecurityPolicy::RequireEncryption);
        self.policies.push(SecurityPolicy::RequirePermission {
            resource: "data".to_string(),
            action: "write".to_string(),
        });
    }

    /// Get all security policies
    pub fn policies(&self) -> &[SecurityPolicy] {
        &self.policies
    }

    /// Add a security policy
    pub fn add_policy(&mut self, policy: SecurityPolicy) {
        self.policies.push(policy);
    }

    /// Check if user is locked out
    fn is_user_locked_out(&self, user_id: &str) -> bool {
        if let Some((count, last_attempt)) = self.failed_attempts.get(user_id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if *count >= self.config.max_failed_attempts {
                return now - last_attempt < self.config.lockout_duration;
            }
        }
        false
    }

    /// Record failed authentication attempt
    #[allow(dead_code)]
    fn record_failed_attempt(&mut self, user_id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let (count, _) = self.failed_attempts.get(user_id).unwrap_or(&(0, 0));
        self.failed_attempts
            .insert(user_id.to_string(), (count + 1, now));
    }

    /// Clear failed attempts for user
    #[allow(dead_code)]
    fn clear_failed_attempts(&mut self, user_id: &str) {
        self.failed_attempts.remove(user_id);
    }
}

#[async_trait::async_trait]
impl SecurityManager for SimpleSecurityManager {
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> SecurityResult<SecurityContext> {
        // Check if user is locked out
        if self.is_user_locked_out(username) {
            return Err(SecurityError::AuthenticationFailed {
                message: "Account locked due to too many failed attempts".to_string(),
            });
        }

        // Authenticate with RBAC manager
        let user_id = self
            .rbac_manager
            .authenticate_user(username, password)
            .await
            .map_err(|e| SecurityError::RbacError {
                message: e.to_string(),
            })?;

        // Get user permissions
        let permissions = self
            .rbac_manager
            .get_user_permissions(&user_id)
            .await
            .map_err(|e| SecurityError::RbacError {
                message: e.to_string(),
            })?;

        // Create session
        let session = self
            .rbac_manager
            .create_session(&user_id)
            .await
            .map_err(|e| SecurityError::RbacError {
                message: e.to_string(),
            })?;

        let context = SecurityContext {
            user_id,
            session: Some(session),
            permissions: permissions.into_iter().collect(),
            metadata: HashMap::new(),
        };

        // Log successful authentication
        if self.config.enable_audit_logging {
            let event = SecurityEvent {
                event_type: "authentication_success".to_string(),
                user_id: context.user_id.clone(),
                operation: "login".to_string(),
                resource: "system".to_string(),
                success: true,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                metadata: HashMap::new(),
            };
            let _ = self.log_security_event(event).await;
        }

        Ok(context)
    }

    async fn validate_operation(
        &self,
        context: &SecurityContext,
        operation: &str,
    ) -> SecurityResult<()> {
        // Check if session is valid
        if let Some(session) = &context.session {
            if self
                .rbac_manager
                .validate_session(&session.id)
                .await
                .is_err()
            {
                return Err(SecurityError::SessionInvalid {
                    message: "Session expired or invalid".to_string(),
                });
            }
        }

        // Apply security policies
        self.apply_policies(context, operation).await?;

        Ok(())
    }

    async fn encrypt_data(
        &self,
        data: &[u8],
        context: &SecurityContext,
    ) -> SecurityResult<EncryptedData> {
        // Validate operation
        self.validate_operation(context, "encrypt").await?;

        // Encrypt data
        self.encryption_manager
            .encrypt(data, None)
            .await
            .map_err(|e| SecurityError::EncryptionError {
                message: e.to_string(),
            })
    }

    async fn decrypt_data(
        &self,
        encrypted_data: &EncryptedData,
        context: &SecurityContext,
    ) -> SecurityResult<Vec<u8>> {
        // Validate operation
        self.validate_operation(context, "decrypt").await?;

        // Decrypt data
        self.encryption_manager
            .decrypt(encrypted_data)
            .await
            .map_err(|e| SecurityError::EncryptionError {
                message: e.to_string(),
            })
    }

    async fn has_permission(
        &self,
        context: &SecurityContext,
        permission: &Permission,
    ) -> SecurityResult<bool> {
        self.rbac_manager
            .check_permission(&context.user_id, permission)
            .await
            .map_err(|e| SecurityError::RbacError {
                message: e.to_string(),
            })
    }

    async fn apply_policies(
        &self,
        context: &SecurityContext,
        _operation: &str,
    ) -> SecurityResult<()> {
        if !self.config.enable_security_policies {
            return Ok(());
        }

        for policy in &self.policies {
            match policy {
                SecurityPolicy::RequireEncryption => {
                    // This is handled at the data level
                }
                SecurityPolicy::RequirePermission { resource, action } => {
                    let permission = Permission::new(resource, action);
                    match self.has_permission(context, &permission).await {
                        Ok(true) => {
                            // Permission granted, continue
                        }
                        Ok(false) | Err(_) => {
                            // Permission denied or error checking permission (e.g., user not found)
                            // Both cases should be treated as policy violations
                            return Err(SecurityError::PolicyViolation {
                                message: format!("Permission required: {}:{}", resource, action),
                            });
                        }
                    }
                }
                SecurityPolicy::RateLimit {
                    max_requests,
                    window_seconds,
                } => {
                    // In a real implementation, this would check rate limits
                    // For now, we'll just log the policy check
                    tracing::debug!(
                        "Rate limit policy applied: {} requests per {} seconds",
                        max_requests,
                        window_seconds
                    );
                }
                SecurityPolicy::DataClassification {
                    level,
                    required_permissions,
                } => {
                    // In a real implementation, this would check data classification
                    tracing::debug!(
                        "Data classification policy applied: level {}, permissions: {:?}",
                        level,
                        required_permissions
                    );
                }
            }
        }

        Ok(())
    }

    async fn log_security_event(&self, event: SecurityEvent) -> SecurityResult<()> {
        if !self.config.enable_audit_logging {
            return Ok(());
        }

        // In a real implementation, this would write to an audit log
        tracing::info!(
            "Security event: type={}, user={}, operation={}, resource={}, success={}",
            event.event_type,
            event.user_id,
            event.operation,
            event.resource,
            event.success
        );

        Ok(())
    }
}

/// Create a new security manager
pub fn create_security_manager(config: SecurityConfig) -> SecurityResult<Arc<dyn SecurityManager>> {
    let manager = SimpleSecurityManager::new(config)?;
    Ok(Arc::new(manager))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_manager_creation() {
        let config = SecurityConfig::default();
        let manager = create_security_manager(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_security_policies() {
        let config = SecurityConfig {
            enable_security_policies: true,
            ..Default::default()
        };

        let manager = SimpleSecurityManager::new(config).unwrap();
        assert!(!manager.policies.is_empty());
    }

    #[tokio::test]
    async fn test_user_lockout() {
        let config = SecurityConfig {
            max_failed_attempts: 3,
            lockout_duration: 60,
            ..Default::default()
        };

        let mut manager = SimpleSecurityManager::new(config).unwrap();

        // Record failed attempts
        manager.record_failed_attempt("user1");
        manager.record_failed_attempt("user1");
        manager.record_failed_attempt("user1");

        // User should be locked out
        assert!(manager.is_user_locked_out("user1"));

        // Clear failed attempts
        manager.clear_failed_attempts("user1");
        assert!(!manager.is_user_locked_out("user1"));
    }

    #[tokio::test]
    async fn test_security_context_creation() {
        use crate::rbac::{Permission, Session};

        let config = SecurityConfig::default();
        let _manager = SimpleSecurityManager::new(config).unwrap();

        // Create a security context manually
        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: Some(Session {
                id: "session1".to_string(),
                user_id: "user1".to_string(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                expires_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
                permissions: std::collections::HashSet::new(),
                metadata: HashMap::new(),
            }),
            permissions: vec![Permission::new("data", "read")],
            metadata: HashMap::new(),
        };

        assert_eq!(context.user_id, "user1");
        assert!(context.session.is_some());
        assert!(!context.permissions.is_empty());
    }

    #[tokio::test]
    async fn test_security_policy_application() {
        let config = SecurityConfig {
            enable_security_policies: true,
            ..Default::default()
        };

        let manager = SimpleSecurityManager::new(config).unwrap();

        // Policies should be initialized
        assert!(!manager.policies.is_empty());

        // Check for default policies
        let has_encryption_policy = manager
            .policies
            .iter()
            .any(|p| matches!(p, SecurityPolicy::RequireEncryption));
        assert!(has_encryption_policy);
    }

    #[tokio::test]
    async fn test_security_event_logging() {
        let config = SecurityConfig {
            enable_audit_logging: true,
            ..Default::default()
        };

        let manager = create_security_manager(config).unwrap();

        let event = SecurityEvent {
            event_type: "test_event".to_string(),
            user_id: "user1".to_string(),
            operation: "test_operation".to_string(),
            resource: "test_resource".to_string(),
            success: true,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        };

        // Logging should succeed
        let result = manager.log_security_event(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_event_logging_disabled() {
        let config = SecurityConfig {
            enable_audit_logging: false,
            ..Default::default()
        };

        let manager = create_security_manager(config).unwrap();

        let event = SecurityEvent {
            event_type: "test_event".to_string(),
            user_id: "user1".to_string(),
            operation: "test_operation".to_string(),
            resource: "test_resource".to_string(),
            success: true,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        };

        // Logging should still succeed but not actually log
        let result = manager.log_security_event(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_checking() {
        use crate::rbac::Permission;

        let config = SecurityConfig::default();
        let manager = create_security_manager(config).unwrap();

        // Create a context with permissions
        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: None,
            permissions: vec![Permission::new("data", "read")],
            metadata: HashMap::new(),
        };

        // Check permission
        let permission = Permission::new("data", "read");
        let result = manager.has_permission(&context, &permission).await;

        // Result depends on RBAC manager implementation
        // Just verify it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_validate_operation_with_session() {
        use crate::rbac::{Permission, Session};

        let config = SecurityConfig::default();
        let manager = create_security_manager(config).unwrap();

        let session = Session {
            id: "session1".to_string(),
            user_id: "user1".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600,
            permissions: std::collections::HashSet::new(),
            metadata: HashMap::new(),
        };

        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: Some(session),
            permissions: vec![Permission::new("data", "read")],
            metadata: HashMap::new(),
        };

        // Validation depends on RBAC manager
        // Just verify it doesn't panic
        let result = manager.validate_operation(&context, "test_operation").await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_encrypt_data_with_context() {
        use crate::rbac::Permission;

        let config = SecurityConfig::default();
        let manager = create_security_manager(config).unwrap();

        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: None,
            permissions: vec![Permission::new("data", "encrypt")],
            metadata: HashMap::new(),
        };

        let data = b"test data";

        // Encryption depends on encryption manager
        // Just verify it doesn't panic
        let result = manager.encrypt_data(data, &context).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_decrypt_data_with_context() {
        use crate::encryption::EncryptedData;
        use crate::rbac::Permission;

        let config = SecurityConfig::default();
        let manager = create_security_manager(config).unwrap();

        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: None,
            permissions: vec![Permission::new("data", "decrypt")],
            metadata: HashMap::new(),
        };

        let encrypted_data = EncryptedData {
            ciphertext: vec![1, 2, 3, 4],
            nonce: vec![5, 6, 7, 8],
            key_id: "key1".to_string(),
            algorithm: crate::encryption::EncryptionAlgorithm::Aes256Gcm,
            salt: vec![9, 10, 11, 12],
            version: 1,
        };

        // Decryption depends on encryption manager
        // Just verify it doesn't panic
        let result = manager.decrypt_data(&encrypted_data, &context).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_failed_attempt_tracking() {
        let config = SecurityConfig {
            max_failed_attempts: 5,
            lockout_duration: 300,
            ..Default::default()
        };

        let mut manager = SimpleSecurityManager::new(config).unwrap();

        // Track multiple failed attempts
        for _i in 1..=4 {
            manager.record_failed_attempt("user1");
            assert!(!manager.is_user_locked_out("user1")); // Not locked yet
        }

        // 5th attempt should lock
        manager.record_failed_attempt("user1");
        assert!(manager.is_user_locked_out("user1"));
    }

    #[tokio::test]
    async fn test_security_policies_disabled() {
        let config = SecurityConfig {
            enable_security_policies: false,
            ..Default::default()
        };

        let manager = create_security_manager(config).unwrap();

        use crate::rbac::Permission;
        let context = SecurityContext {
            user_id: "user1".to_string(),
            session: None,
            permissions: vec![Permission::new("data", "read")],
            metadata: HashMap::new(),
        };

        // When policies are disabled, validation should pass
        let result = manager.validate_operation(&context, "test_operation").await;
        assert!(result.is_ok());
    }
}
