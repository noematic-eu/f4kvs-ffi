//! F4KVS Authentication and Authorization System
//!
//! This module provides comprehensive authentication and authorization capabilities
//! for F4KVS, including JWT tokens, RBAC, user management, and security middleware.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
// Note: uuid is optional, using a simple ID generator for now

/// Authentication and authorization errors
#[derive(Error, Debug)]
pub enum AuthError {
    /// Invalid username or password provided
    #[error("Invalid credentials")]
    InvalidCredentials,
    /// JWT token has expired and needs to be refreshed
    #[error("Token expired")]
    TokenExpired,
    /// JWT token is malformed or invalid
    #[error("Token invalid")]
    TokenInvalid,
    /// User lacks required permissions for the operation
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    /// Requested user does not exist in the system
    #[error("User not found")]
    UserNotFound,
    /// Attempted to create a user that already exists
    #[error("User already exists")]
    UserAlreadyExists,
    /// Requested role does not exist in the system
    #[error("Role not found")]
    RoleNotFound,
    /// Operation denied due to access control restrictions
    #[error("Permission denied")]
    PermissionDenied,
    /// JWT token format is invalid or corrupted
    #[error("Invalid token format")]
    InvalidTokenFormat,
    /// Authentication is required but not provided
    #[error("Authentication required")]
    AuthenticationRequired,
    /// Internal system error with additional context
    #[error("Internal error: {0}")]
    Internal(String),
    /// Requested feature is not enabled in the current configuration
    #[error("Feature not enabled")]
    FeatureNotEnabled,
}

/// Result type for authentication operations
pub type AuthResult<T> = Result<T, AuthError>;

/// User entity for RBAC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique identifier for the user
    pub id: String,
    /// Username used for authentication
    pub username: String,
    /// Email address for notifications and recovery
    pub email: String,
    /// Hashed password for secure authentication
    pub password_hash: String,
    /// List of roles assigned to the user
    pub roles: Vec<String>,
    /// Whether the user account is active
    pub is_active: bool,
    /// Unix timestamp when the user was created
    pub created_at: u64,
    /// Unix timestamp of the last successful login
    pub last_login: Option<u64>,
    /// Additional metadata associated with the user
    pub metadata: HashMap<String, String>,
}

/// Role entity for RBAC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Unique identifier for the role
    pub id: String,
    /// Human-readable name of the role
    pub name: String,
    /// Description of the role's purpose and scope
    pub description: String,
    /// List of permissions granted to this role
    pub permissions: Vec<Permission>,
    /// Whether the role is currently active
    pub is_active: bool,
    /// Unix timestamp when the role was created
    pub created_at: u64,
}

/// Permission entity for RBAC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permission {
    /// Resource identifier (e.g., "namespace:users", "namespace:*", "*")
    pub resource: String,
    /// Action allowed on the resource (e.g., "read", "write", "delete", "admin", "*")
    pub action: String,
    /// Additional conditions for permission evaluation
    pub conditions: HashMap<String, String>,
}

/// JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Unique identifier of the authenticated user
    pub user_id: String,
    /// Username of the authenticated user
    pub username: String,
    /// List of roles assigned to the user
    pub roles: Vec<String>,
    /// List of permissions granted to the user
    pub permissions: Vec<Permission>,
    /// Unique identifier for this JWT token (optional for backward compatibility)
    pub jti: Option<String>,
    /// Unix timestamp when the token was issued
    pub iat: u64,
    /// Unix timestamp when the token expires
    pub exp: u64,
}

/// Authentication context for request processing
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Unique identifier of the authenticated user
    pub user_id: String,
    /// Username of the authenticated user
    pub username: String,
    /// List of roles assigned to the user
    pub roles: Vec<String>,
    /// List of permissions granted to the user
    pub permissions: Vec<Permission>,
    /// JWT token used for authentication
    pub token: String,
    /// Unix timestamp when the token expires
    pub expires_at: u64,
}

/// Authentication methods supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// JSON Web Token authentication
    JWT,
    /// HTTP Basic authentication
    Basic,
    /// API Key authentication
    ApiKey,
}

/// Request to create a new user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    /// Username for the new user
    pub username: String,
    /// Email address for the new user
    pub email: String,
    /// Plain text password (will be hashed)
    pub password: String,
    /// Optional list of roles to assign to the user
    pub roles: Option<Vec<String>>,
    /// Optional metadata to associate with the user
    pub metadata: Option<HashMap<String, String>>,
}

/// Request to update an existing user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    /// New email address for the user
    pub email: Option<String>,
    /// New list of roles for the user
    pub roles: Option<Vec<String>>,
    /// Whether the user account should be active
    pub is_active: Option<bool>,
    /// New metadata for the user
    pub metadata: Option<HashMap<String, String>>,
}

/// Request to create a new role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    /// Name of the new role
    pub name: String,
    /// Description of the role's purpose
    pub description: String,
    /// List of permissions to assign to the role
    pub permissions: Vec<Permission>,
}

/// Authentication configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Secret key for JWT token signing
    pub jwt_secret: String,
    /// JWT token expiration time in seconds
    pub jwt_expiry_seconds: u64,
    /// Minimum password length requirement
    pub password_min_length: usize,
    /// Maximum login attempts before lockout
    pub max_login_attempts: u32,
    /// Lockout duration in seconds after max attempts
    pub lockout_duration_seconds: u64,
    /// Whether to enable audit logging for auth events
    pub enable_audit_logging: bool,
    /// Default roles assigned to new users
    pub default_roles: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "f4kvs-default-secret-change-in-production".to_string(),
            jwt_expiry_seconds: 3600, // 1 hour
            password_min_length: 8,
            max_login_attempts: 5,
            lockout_duration_seconds: 900, // 15 minutes
            enable_audit_logging: true,
            default_roles: vec!["user".to_string()],
        }
    }
}

/// Main authentication manager
pub struct AuthManager {
    config: AuthConfig,
    users: HashMap<String, User>,
    roles: HashMap<String, Role>,
    user_roles: HashMap<String, Vec<String>>, // user_id -> role_ids
    role_permissions: HashMap<String, Vec<Permission>>, // role_id -> permissions
    login_attempts: HashMap<String, (u32, u64)>, // username -> (attempts, last_attempt)
    audit_logs: Vec<AuditLog>,
    user_counter: u64,
    role_counter: u64,
}

/// Audit log entry for security and compliance tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    /// Unique identifier for the audit log entry
    pub id: String,
    /// ID of the user who performed the action
    pub user_id: String,
    /// Action that was performed
    pub action: String,
    /// Resource that was accessed or modified
    pub resource: String,
    /// Unix timestamp when the action occurred
    pub timestamp: u64,
    /// IP address of the client
    pub ip_address: String,
    /// User agent string from the client
    pub user_agent: String,
    /// Result of the action (success/failure)
    pub result: AuditResult,
    /// Additional details and context about the action
    pub details: HashMap<String, String>,
}

/// Audit event data for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// ID of the user who performed the action
    pub user_id: String,
    /// Action that was performed
    pub action: String,
    /// Resource that was accessed or modified
    pub resource: String,
    /// IP address of the client
    pub ip_address: String,
    /// User agent string from the client
    pub user_agent: String,
    /// Result of the action (success/failure)
    pub result: AuditResult,
    /// Additional details and context about the action
    pub details: HashMap<String, String>,
}

/// Result of an audited action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    /// Action completed successfully
    Success,
    /// Action failed due to an error
    Failure,
    /// Action was denied due to insufficient permissions
    Denied,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(config: AuthConfig) -> Self {
        let mut manager = Self {
            config,
            users: HashMap::new(),
            roles: HashMap::new(),
            user_roles: HashMap::new(),
            role_permissions: HashMap::new(),
            login_attempts: HashMap::new(),
            audit_logs: Vec::new(),
            user_counter: 0,
            role_counter: 0,
        };

        // Initialize default roles
        manager.initialize_default_roles();
        manager
    }

    /// Initialize default roles and permissions
    fn initialize_default_roles(&mut self) {
        // Admin role
        let admin_role = Role {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            description: "Full system access".to_string(),
            permissions: vec![Permission {
                resource: "*".to_string(),
                action: "*".to_string(),
                conditions: HashMap::new(),
            }],
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // User role
        let user_role = Role {
            id: "user".to_string(),
            name: "User".to_string(),
            description: "Basic user access".to_string(),
            permissions: vec![
                Permission {
                    resource: "namespace:user:*".to_string(),
                    action: "read".to_string(),
                    conditions: HashMap::new(),
                },
                Permission {
                    resource: "namespace:user:*".to_string(),
                    action: "write".to_string(),
                    conditions: HashMap::new(),
                },
            ],
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Read-only role
        let read_only_role = Role {
            id: "readonly".to_string(),
            name: "Read Only".to_string(),
            description: "Read-only access".to_string(),
            permissions: vec![Permission {
                resource: "namespace:readonly:*".to_string(),
                action: "read".to_string(),
                conditions: HashMap::new(),
            }],
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Store roles
        self.roles.insert(admin_role.id.clone(), admin_role.clone());
        self.roles.insert(user_role.id.clone(), user_role.clone());
        self.roles
            .insert(read_only_role.id.clone(), read_only_role.clone());

        // Store role permissions for quick lookup
        self.role_permissions
            .insert(admin_role.id, admin_role.permissions);
        self.role_permissions
            .insert(user_role.id, user_role.permissions);
        self.role_permissions
            .insert(read_only_role.id, read_only_role.permissions);
    }

    /// Create a new user
    pub async fn create_user(&mut self, request: CreateUserRequest) -> AuthResult<User> {
        // Check if user already exists
        if self.users.values().any(|u| u.username == request.username) {
            return Err(AuthError::UserAlreadyExists);
        }

        // Validate password
        if request.password.len() < self.config.password_min_length {
            return Err(AuthError::Internal("Password too short".to_string()));
        }

        // Hash password (in real implementation, use proper hashing like bcrypt)
        let password_hash = self.hash_password(&request.password)?;

        // Create user
        self.user_counter += 1;
        let user = User {
            id: format!("user_{}", self.user_counter),
            username: request.username,
            email: request.email,
            password_hash,
            roles: request
                .roles
                .unwrap_or_else(|| self.config.default_roles.clone()),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_login: None,
            metadata: request.metadata.unwrap_or_default(),
        };

        // Store user
        self.users.insert(user.id.clone(), user.clone());
        self.user_roles.insert(user.id.clone(), user.roles.clone());

        // Log audit event
        if self.config.enable_audit_logging {
            self.log_audit_event(AuditEvent {
                user_id: user.id.clone(),
                action: "CREATE_USER".to_string(),
                resource: "user".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "system".to_string(),
                result: AuditResult::Success,
                details: HashMap::new(),
            });
        }

        Ok(user)
    }

    /// Authenticate user with username and password
    pub async fn authenticate(
        &mut self,
        username: &str,
        password: &str,
    ) -> AuthResult<AuthContext> {
        // Check login attempts
        if let Some((attempts, last_attempt)) = self.login_attempts.get(username) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if *attempts >= self.config.max_login_attempts {
                if now - last_attempt < self.config.lockout_duration_seconds {
                    return Err(AuthError::InvalidCredentials);
                } else {
                    // Reset attempts after lockout period
                    self.login_attempts.remove(username);
                }
            }
        }

        // Find user
        let user_id = self
            .users
            .values()
            .find(|u| u.username == username && u.is_active)
            .map(|u| u.id.clone())
            .ok_or(AuthError::InvalidCredentials)?;

        // Get user for password verification
        let user = self.users.get(&user_id).unwrap().clone();

        // Verify password
        if !self.verify_password(password, &user.password_hash)? {
            // Increment login attempts
            let attempts = self
                .login_attempts
                .get(username)
                .map(|(a, _)| *a + 1)
                .unwrap_or(1);
            self.login_attempts.insert(
                username.to_string(),
                (
                    attempts,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                ),
            );

            return Err(AuthError::InvalidCredentials);
        }

        // Reset login attempts on successful login
        self.login_attempts.remove(username);

        // Update last login
        if let Some(user) = self.users.get_mut(&user_id) {
            user.last_login = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
        }

        // Get user permissions
        let permissions = self.get_user_permissions(&user_id)?;

        // Generate JWT token
        let token = self.generate_jwt_token(&user)?;

        // Create auth context
        let context = AuthContext {
            user_id: user_id.clone(),
            username: user.username.clone(),
            roles: user.roles.clone(),
            permissions,
            token: token.clone(),
            expires_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + self.config.jwt_expiry_seconds,
        };

        // Log audit event
        if self.config.enable_audit_logging {
            self.log_audit_event(AuditEvent {
                user_id: user_id.clone(),
                action: "AUTHENTICATE".to_string(),
                resource: "user".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "system".to_string(),
                result: AuditResult::Success,
                details: HashMap::new(),
            });
        }

        Ok(context)
    }

    /// Verify JWT token and return auth context
    pub async fn verify_token(&self, token: &str) -> AuthResult<AuthContext> {
        // In real implementation, verify JWT signature and expiration
        // For now, we'll do a simplified verification
        if !token.starts_with("f4kvs.") {
            return Err(AuthError::InvalidTokenFormat);
        }

        let encoded_claims = &token[6..]; // Remove "f4kvs." prefix
        #[cfg(feature = "auth")]
        {
            use base64::{engine::general_purpose, Engine as _};
            let decoded_claims = general_purpose::STANDARD
                .decode(encoded_claims)
                .map_err(|_| AuthError::InvalidTokenFormat)?;
            let claims: TokenClaims = serde_json::from_slice(&decoded_claims)
                .map_err(|_| AuthError::InvalidTokenFormat)?;

            // Check expiration
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if claims.exp < now {
                return Err(AuthError::TokenExpired);
            }

            // Get user
            let user = self
                .users
                .get(&claims.user_id)
                .ok_or(AuthError::UserNotFound)?;

            if !user.is_active {
                return Err(AuthError::UserNotFound);
            }

            // Get user permissions
            let permissions = self.get_user_permissions(&user.id)?;

            Ok(AuthContext {
                user_id: user.id.clone(),
                username: user.username.clone(),
                roles: user.roles.clone(),
                permissions,
                token: token.to_string(),
                expires_at: claims.exp,
            })
        }
        #[cfg(not(feature = "auth"))]
        {
            Err(AuthError::FeatureNotEnabled)
        }
    }

    /// Check if user has permission for resource and action
    pub fn has_permission(&self, context: &AuthContext, resource: &str, action: &str) -> bool {
        for permission in &context.permissions {
            if self.matches_permission(permission, resource, action) {
                return true;
            }
        }
        false
    }

    /// Check if permission matches resource and action
    fn matches_permission(&self, permission: &Permission, resource: &str, action: &str) -> bool {
        // Check action match
        if permission.action != "*" && permission.action != action {
            return false;
        }

        // Check resource match
        if permission.resource == "*" {
            return true;
        }

        // Pattern matching for resources
        if permission.resource.ends_with("*") {
            let prefix = permission.resource.strip_suffix("*").unwrap();
            resource.starts_with(prefix)
        } else if permission.resource.contains("*") {
            // Handle patterns like "namespace:user:*"
            let parts: Vec<&str> = permission.resource.split(':').collect();
            let resource_parts: Vec<&str> = resource.split(':').collect();

            if parts.len() != resource_parts.len() {
                return false;
            }

            for (pattern_part, resource_part) in parts.iter().zip(resource_parts.iter()) {
                if *pattern_part != "*" && pattern_part != resource_part {
                    return false;
                }
            }
            true
        } else {
            permission.resource == resource
        }
    }

    /// Get user permissions
    fn get_user_permissions(&self, user_id: &str) -> AuthResult<Vec<Permission>> {
        let mut permissions = Vec::new();

        if let Some(role_ids) = self.user_roles.get(user_id) {
            for role_id in role_ids {
                if let Some(role_permissions) = self.role_permissions.get(role_id) {
                    permissions.extend(role_permissions.clone());
                }
            }
        }

        Ok(permissions)
    }

    /// Generate JWT token (simplified implementation)
    fn generate_jwt_token(&mut self, user: &User) -> AuthResult<String> {
        #[cfg(feature = "auth")]
        {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Generate unique JWT ID using timestamp and user ID
            let jti = format!("{}_{}_{}", user.id, now, self.user_counter);

            let claims = TokenClaims {
                user_id: user.id.clone(),
                username: user.username.clone(),
                roles: user.roles.clone(),
                permissions: self.get_user_permissions(&user.id)?,
                jti: Some(jti),
                iat: now,
                exp: now + self.config.jwt_expiry_seconds,
            };

            // Increment counter for next token to ensure uniqueness
            self.user_counter += 1;

            // In real implementation, use proper JWT library with HMAC signing
            let token = serde_json::to_string(&claims)
                .map_err(|e| AuthError::Internal(format!("Failed to serialize claims: {}", e)))?;

            use base64::{engine::general_purpose, Engine as _};
            Ok(format!("f4kvs.{}", general_purpose::STANDARD.encode(token)))
        }
        #[cfg(not(feature = "auth"))]
        {
            Err(AuthError::FeatureNotEnabled)
        }
    }

    /// Hash password (simplified implementation)
    fn hash_password(&self, password: &str) -> AuthResult<String> {
        // In real implementation, use proper password hashing like bcrypt
        Ok(format!("hashed_{}", password))
    }

    /// Verify password (simplified implementation)
    fn verify_password(&self, password: &str, hash: &str) -> AuthResult<bool> {
        // In real implementation, use proper password verification
        let expected = format!("hashed_{}", password);
        Ok(hash == expected)
    }

    /// Log audit event
    fn log_audit_event(&mut self, event: AuditEvent) {
        let audit_log = AuditLog {
            id: format!(
                "audit_{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ),
            user_id: event.user_id,
            action: event.action,
            resource: event.resource,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ip_address: event.ip_address,
            user_agent: event.user_agent,
            result: event.result,
            details: event.details,
        };

        self.audit_logs.push(audit_log);
    }

    /// Get audit logs
    pub fn get_audit_logs(&self, user_id: Option<&str>, limit: usize) -> Vec<&AuditLog> {
        self.audit_logs
            .iter()
            .filter(|log| user_id.is_none_or(|uid| log.user_id == uid))
            .rev()
            .take(limit)
            .collect()
    }

    /// Create a role
    pub async fn create_role(&mut self, request: CreateRoleRequest) -> AuthResult<Role> {
        self.role_counter += 1;
        let role = Role {
            id: format!("role_{}", self.role_counter),
            name: request.name,
            description: request.description,
            permissions: request.permissions,
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.roles.insert(role.id.clone(), role.clone());
        self.role_permissions
            .insert(role.id.clone(), role.permissions.clone());

        Ok(role)
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: &str) -> AuthResult<&User> {
        self.users.get(user_id).ok_or(AuthError::UserNotFound)
    }

    /// Get all users
    pub fn get_users(&self) -> Vec<&User> {
        self.users.values().collect()
    }

    /// Update user
    pub async fn update_user(
        &mut self,
        user_id: &str,
        request: UpdateUserRequest,
    ) -> AuthResult<User> {
        let user = self.users.get_mut(user_id).ok_or(AuthError::UserNotFound)?;

        if let Some(email) = request.email {
            user.email = email;
        }

        if let Some(roles) = request.roles {
            user.roles = roles.clone();
            self.user_roles.insert(user_id.to_string(), roles);
        }

        if let Some(is_active) = request.is_active {
            user.is_active = is_active;
        }

        if let Some(metadata) = request.metadata {
            user.metadata = metadata;
        }

        Ok(user.clone())
    }

    /// Delete user
    pub async fn delete_user(&mut self, user_id: &str) -> AuthResult<()> {
        if self.users.remove(user_id).is_some() {
            self.user_roles.remove(user_id);
            Ok(())
        } else {
            Err(AuthError::UserNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_user() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_authenticate_user() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        auth.create_user(request).await.unwrap();

        let context = auth.authenticate("testuser", "password123").await.unwrap();
        assert_eq!(context.username, "testuser");
    }

    #[tokio::test]
    async fn test_permission_check() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        auth.create_user(request).await.unwrap();

        let context = auth.authenticate("testuser", "password123").await.unwrap();

        // User should have read permission for their namespace
        assert!(auth.has_permission(&context, "namespace:user:data", "read"));
        assert!(auth.has_permission(&context, "namespace:user:data", "write"));

        // User should not have admin permission
        assert!(!auth.has_permission(&context, "namespace:admin:data", "admin"));
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        let token = auth.generate_jwt_token(&user).unwrap();

        // Token should start with "f4kvs."
        assert!(token.starts_with("f4kvs."));
        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn test_jwt_token_verification() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        auth.create_user(request).await.unwrap();
        let context = auth.authenticate("testuser", "password123").await.unwrap();

        // Verify token
        let verified_context = auth.verify_token(&context.token).await.unwrap();
        assert_eq!(verified_context.user_id, context.user_id);
        assert_eq!(verified_context.username, context.username);
    }

    #[tokio::test]
    async fn test_invalid_token_format() {
        let auth = AuthManager::new(AuthConfig::default());

        // Test invalid token format
        let result = auth.verify_token("invalid_token").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidTokenFormat));
    }

    #[tokio::test]
    async fn test_user_creation_duplicate_username() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        auth.create_user(request.clone()).await.unwrap();

        // Try to create user with same username
        let result = auth.create_user(request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::UserAlreadyExists));
    }

    #[tokio::test]
    async fn test_user_creation_short_password() {
        let mut auth = AuthManager::new(AuthConfig {
            password_min_length: 10,
            ..Default::default()
        });

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "short".to_string(), // Too short
            roles: None,
            metadata: None,
        };

        let result = auth.create_user(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_update() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        let user_id = user.id.clone();

        let update_request = UpdateUserRequest {
            email: Some("newemail@example.com".to_string()),
            roles: Some(vec!["admin".to_string()]),
            is_active: Some(true),
            metadata: Some(HashMap::from([("key".to_string(), "value".to_string())])),
        };

        let updated_user = auth.update_user(&user_id, update_request).await.unwrap();
        assert_eq!(updated_user.email, "newemail@example.com");
        assert_eq!(updated_user.roles, vec!["admin".to_string()]);
        assert!(updated_user.is_active);
    }

    #[tokio::test]
    async fn test_user_delete() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        let user_id = user.id.clone();

        // Delete user
        auth.delete_user(&user_id).await.unwrap();

        // Try to get deleted user
        let result = auth.get_user(&user_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::UserNotFound));
    }

    #[tokio::test]
    async fn test_authentication_invalid_credentials() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        auth.create_user(request).await.unwrap();

        // Try to authenticate with wrong password
        let result = auth.authenticate("testuser", "wrongpassword").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn test_authentication_nonexistent_user() {
        let mut auth = AuthManager::new(AuthConfig::default());

        // Try to authenticate non-existent user
        let result = auth.authenticate("nonexistent", "password").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn test_login_attempts_lockout() {
        let mut auth = AuthManager::new(AuthConfig {
            max_login_attempts: 3,
            lockout_duration_seconds: 60,
            ..Default::default()
        });

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        auth.create_user(request).await.unwrap();

        // Make 3 failed attempts
        for _ in 0..3 {
            let _ = auth.authenticate("testuser", "wrongpassword").await;
        }

        // Next attempt should be locked out
        let result = auth.authenticate("testuser", "password123").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn test_role_creation() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateRoleRequest {
            name: "custom_role".to_string(),
            description: "A custom role".to_string(),
            permissions: vec![Permission {
                resource: "data".to_string(),
                action: "read".to_string(),
                conditions: HashMap::new(),
            }],
        };

        let role = auth.create_role(request).await.unwrap();
        assert_eq!(role.name, "custom_role");
        assert_eq!(role.description, "A custom role");
        assert!(!role.permissions.is_empty());
    }

    #[tokio::test]
    async fn test_permission_wildcard_matching() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "adminuser".to_string(),
            email: "admin@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["admin".to_string()]),
            metadata: None,
        };

        auth.create_user(request).await.unwrap();
        let context = auth.authenticate("adminuser", "password123").await.unwrap();

        // Admin should have all permissions
        assert!(auth.has_permission(&context, "any:resource", "any_action"));
        assert!(auth.has_permission(&context, "namespace:user:data", "read"));
        assert!(auth.has_permission(&context, "namespace:user:data", "write"));
    }

    #[tokio::test]
    async fn test_permission_pattern_matching() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: Some(vec!["user".to_string()]),
            metadata: None,
        };

        auth.create_user(request).await.unwrap();
        let context = auth.authenticate("testuser", "password123").await.unwrap();

        // Test pattern matching with wildcards
        assert!(auth.has_permission(&context, "namespace:user:data", "read"));
        assert!(auth.has_permission(&context, "namespace:user:other", "read"));
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let mut auth = AuthManager::new(AuthConfig {
            enable_audit_logging: true,
            ..Default::default()
        });

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        let _ = auth.authenticate("testuser", "password123").await.unwrap();

        // Check audit logs
        let logs = auth.get_audit_logs(Some(&user.id), 10);
        assert!(!logs.is_empty());

        // Should have CREATE_USER and AUTHENTICATE events
        let actions: Vec<&str> = logs.iter().map(|log| log.action.as_str()).collect();
        assert!(actions.contains(&"CREATE_USER"));
        assert!(actions.contains(&"AUTHENTICATE"));
    }

    #[tokio::test]
    async fn test_get_all_users() {
        let mut auth = AuthManager::new(AuthConfig::default());

        // Create multiple users
        for i in 0..5 {
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "password123".to_string(),
                roles: None,
                metadata: None,
            };
            auth.create_user(request).await.unwrap();
        }

        let users = auth.get_users();
        assert_eq!(users.len(), 5);
    }

    #[tokio::test]
    async fn test_user_last_login_update() {
        let mut auth = AuthManager::new(AuthConfig::default());

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };

        let user = auth.create_user(request).await.unwrap();
        assert!(user.last_login.is_none());

        // Authenticate should update last_login
        let _ = auth.authenticate("testuser", "password123").await.unwrap();
        let updated_user = auth.get_user(&user.id).unwrap();
        assert!(updated_user.last_login.is_some());
    }
}
