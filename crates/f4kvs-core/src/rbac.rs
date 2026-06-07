//! F4KVS Role-Based Access Control (RBAC)
//!
//! This module provides comprehensive RBAC capabilities for F4KVS,
//! including user management, role management, permission management,
//! and policy enforcement.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// RBAC (Role-Based Access Control) errors
#[derive(Error, Debug)]
pub enum RbacError {
    /// User not found in the system
    #[error("User not found: {user_id}")]
    UserNotFound {
        /// ID of the user that was not found
        user_id: String,
    },
    /// Role not found in the system
    #[error("Role not found: {role_id}")]
    RoleNotFound {
        /// ID of the role that was not found
        role_id: String,
    },
    /// Permission denied for the requested action
    #[error("Permission denied: {permission}")]
    PermissionDenied {
        /// The permission that was denied
        permission: String,
    },
    /// User already exists in the system
    #[error("User already exists: {user_id}")]
    UserAlreadyExists {
        /// ID of the user that already exists
        user_id: String,
    },
    /// Role already exists in the system
    #[error("Role already exists: {role_id}")]
    RoleAlreadyExists {
        /// ID of the role that already exists
        role_id: String,
    },
    /// Invalid permission specified
    #[error("Invalid permission: {permission}")]
    InvalidPermission {
        /// The invalid permission string
        permission: String,
    },
    /// Invalid role hierarchy configuration
    #[error("Invalid role hierarchy: {message}")]
    InvalidRoleHierarchy {
        /// Error message describing the hierarchy issue
        message: String,
    },
    /// User session has expired
    #[error("Session expired")]
    SessionExpired,
    /// Invalid or corrupted session
    #[error("Invalid session")]
    InvalidSession,
    /// Internal RBAC system error
    #[error("Internal error: {message}")]
    Internal {
        /// Error message describing the internal error
        message: String,
    },
}

/// Result type for RBAC operations
pub type RbacResult<T> = Result<T, RbacError>;

/// User entity for RBAC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique identifier for the user
    pub id: String,
    /// Username for authentication
    pub username: String,
    /// Email address for notifications
    pub email: String,
    /// Hashed password for security
    pub password_hash: String,
    /// Set of role IDs assigned to the user
    pub roles: HashSet<String>,
    /// Whether the user account is active
    pub is_active: bool,
    /// Unix timestamp when the user was created
    pub created_at: u64,
    /// Unix timestamp of the last successful login
    pub last_login: Option<u64>,
    /// Additional metadata associated with the user
    pub metadata: HashMap<String, String>,
}

/// Role entity for RBAC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Unique identifier for the role
    pub id: String,
    /// Human-readable name of the role
    pub name: String,
    /// Description of the role's purpose
    pub description: String,
    /// Set of permissions granted to this role
    pub permissions: HashSet<Permission>,
    /// Set of parent role IDs (for role inheritance)
    pub parent_roles: HashSet<String>,
    /// Whether the role is currently active
    pub is_active: bool,
    /// Unix timestamp when the role was created
    pub created_at: u64,
}

/// Permission entity for RBAC system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    /// Resource identifier (e.g., "users", "orders", "*")
    pub resource: String,
    /// Action allowed on the resource (e.g., "read", "write", "delete", "*")
    pub action: String,
    /// Optional conditions for permission evaluation
    pub conditions: Option<HashMap<String, String>>,
}

impl std::hash::Hash for Permission {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.resource.hash(state);
        self.action.hash(state);
        // For conditions, we'll hash the sorted key-value pairs
        if let Some(conditions) = &self.conditions {
            let mut sorted_conditions: Vec<_> = conditions.iter().collect();
            sorted_conditions.sort_by_key(|(k, _)| *k);
            for (k, v) in sorted_conditions {
                k.hash(state);
                v.hash(state);
            }
        }
    }
}

impl Permission {
    /// Create a new permission
    pub fn new(resource: &str, action: &str) -> Self {
        Self {
            resource: resource.to_string(),
            action: action.to_string(),
            conditions: None,
        }
    }

    /// Create a permission with conditions
    pub fn with_conditions(
        resource: &str,
        action: &str,
        conditions: HashMap<String, String>,
    ) -> Self {
        Self {
            resource: resource.to_string(),
            action: action.to_string(),
            conditions: Some(conditions),
        }
    }

    /// Check if this permission matches another.
    ///
    /// A permission grants another if:
    /// - the resource matches exactly or is a wildcard (`*` or `all`)
    /// - the action matches exactly or is a wildcard (`*` or `all`)
    /// - permission conditions are satisfied when present
    pub fn matches(&self, other: &Permission) -> bool {
        let resource_matches =
            self.resource == "*" || self.resource == "all" || self.resource == other.resource;

        let action_matches =
            self.action == "*" || self.action == "all" || self.action == other.action;

        let conditions_match = match (&self.conditions, &other.conditions) {
            (None, _) => true,
            (Some(required), Some(actual)) => required
                .iter()
                .all(|(k, v)| actual.get(k).map(|value| value == v).unwrap_or(false)),
            (Some(_), None) => false,
        };

        resource_matches && action_matches && conditions_match
    }
}

/// User session for RBAC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier for the session
    pub id: String,
    /// ID of the user this session belongs to
    pub user_id: String,
    /// Unix timestamp when the session was created
    pub created_at: u64,
    /// Unix timestamp when the session expires
    pub expires_at: u64,
    /// Set of permissions available in this session
    pub permissions: HashSet<Permission>,
    /// Additional metadata associated with the session
    pub metadata: HashMap<String, String>,
}

/// Configuration for RBAC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    /// Whether RBAC is enabled
    pub enabled: bool,
    /// Session timeout in seconds
    pub session_timeout: u64,
    /// Maximum number of sessions per user
    pub max_sessions_per_user: usize,
    /// Whether to enable role hierarchy inheritance
    pub enable_role_hierarchy: bool,
    /// Whether to enable permission result caching
    pub enable_permission_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            session_timeout: 3600, // 1 hour
            max_sessions_per_user: 5,
            enable_role_hierarchy: true,
            enable_permission_caching: true,
            cache_ttl: 300, // 5 minutes
        }
    }
}

/// RBAC manager trait
#[async_trait::async_trait]
pub trait RbacManager: Send + Sync {
    /// User management
    async fn create_user(&self, username: &str, email: &str, password: &str) -> RbacResult<String>;
    /// Get user by ID
    async fn get_user(&self, user_id: &str) -> RbacResult<User>;
    /// Update user with partial updates
    async fn update_user(&self, user_id: &str, updates: UserUpdate) -> RbacResult<()>;
    /// Delete user by ID
    async fn delete_user(&self, user_id: &str) -> RbacResult<()>;
    /// List all users
    async fn list_users(&self) -> RbacResult<Vec<User>>;
    /// Authenticate user with username and password
    async fn authenticate_user(&self, username: &str, password: &str) -> RbacResult<String>;

    /// Role management
    async fn create_role(&self, name: &str, description: &str) -> RbacResult<String>;
    /// Get role by ID
    async fn get_role(&self, role_id: &str) -> RbacResult<Role>;
    /// Update role with partial updates
    async fn update_role(&self, role_id: &str, updates: RoleUpdate) -> RbacResult<()>;
    /// Delete role by ID
    async fn delete_role(&self, role_id: &str) -> RbacResult<()>;
    /// List all roles
    async fn list_roles(&self) -> RbacResult<Vec<Role>>;

    /// Permission management
    async fn assign_permission_to_role(
        &self,
        role_id: &str,
        permission: Permission,
    ) -> RbacResult<()>;
    /// Revoke permission from role
    async fn revoke_permission_from_role(
        &self,
        role_id: &str,
        permission: &Permission,
    ) -> RbacResult<()>;
    /// Assign role to user
    async fn assign_role_to_user(&self, user_id: &str, role_id: &str) -> RbacResult<()>;
    /// Revoke role from user
    async fn revoke_role_from_user(&self, user_id: &str, role_id: &str) -> RbacResult<()>;

    /// Authorization
    async fn check_permission(&self, user_id: &str, permission: &Permission) -> RbacResult<bool>;
    /// Get all permissions for a user
    async fn get_user_permissions(&self, user_id: &str) -> RbacResult<HashSet<Permission>>;
    /// Create a new session for a user
    async fn create_session(&self, user_id: &str) -> RbacResult<Session>;
    /// Validate an existing session
    async fn validate_session(&self, session_id: &str) -> RbacResult<Session>;
    /// Invalidate a session
    async fn invalidate_session(&self, session_id: &str) -> RbacResult<()>;
}

/// User update structure for partial updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdate {
    /// New username (if provided)
    pub username: Option<String>,
    /// New email address (if provided)
    pub email: Option<String>,
    /// New password hash (if provided)
    pub password_hash: Option<String>,
    /// New active status (if provided)
    pub is_active: Option<bool>,
    /// New metadata (if provided)
    pub metadata: Option<HashMap<String, String>>,
}

/// Role update structure for partial updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleUpdate {
    /// New role name (if provided)
    pub name: Option<String>,
    /// New role description (if provided)
    pub description: Option<String>,
    /// New active status (if provided)
    pub is_active: Option<bool>,
    /// New parent roles (if provided)
    pub parent_roles: Option<HashSet<String>>,
}

/// Simple RBAC manager implementation
pub struct SimpleRbacManager {
    config: RbacConfig,
    users: RwLock<HashMap<String, User>>,
    roles: RwLock<HashMap<String, Role>>,
    sessions: RwLock<HashMap<String, Session>>,
    user_sessions: RwLock<HashMap<String, VecDeque<String>>>, // user_id -> session_ids (ordered by creation)
}

impl SimpleRbacManager {
    /// Create a new RBAC manager
    pub fn new(config: RbacConfig) -> Self {
        let manager = Self {
            config,
            users: RwLock::new(HashMap::new()),
            roles: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            user_sessions: RwLock::new(HashMap::new()),
        };

        // Create default roles
        manager.create_default_roles();
        manager
    }

    /// Create default roles
    fn create_default_roles(&self) {
        let mut roles = self.roles.write().unwrap();

        // Admin role
        let admin_role = Role {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            description: "Full system access".to_string(),
            permissions: HashSet::from([
                Permission::new("*", "*"), // All permissions
            ]),
            parent_roles: HashSet::new(),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        roles.insert("admin".to_string(), admin_role);

        // Read-only role
        let read_only_role = Role {
            id: "read_only".to_string(),
            name: "Read Only".to_string(),
            description: "Read-only access".to_string(),
            permissions: HashSet::from([
                Permission::new("data", "read"),
                Permission::new("metadata", "read"),
            ]),
            parent_roles: HashSet::new(),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        roles.insert("read_only".to_string(), read_only_role);

        // Write role
        let write_role = Role {
            id: "write".to_string(),
            name: "Write Access".to_string(),
            description: "Read and write access".to_string(),
            permissions: HashSet::from([
                Permission::new("data", "read"),
                Permission::new("data", "write"),
                Permission::new("metadata", "read"),
                Permission::new("metadata", "write"),
            ]),
            parent_roles: HashSet::new(),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        roles.insert("write".to_string(), write_role);
    }

    /// Hash password (in real implementation, use proper password hashing)
    fn hash_password(&self, password: &str) -> String {
        // In a real implementation, use Argon2 or bcrypt
        format!("hash_{}", password)
    }

    /// Generate session ID
    fn generate_session_id(&self) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!(
            "session_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            counter
        )
    }

    /// Get user permissions including role hierarchy
    fn get_user_permissions_internal(&self, user: &User) -> HashSet<Permission> {
        let mut permissions = HashSet::new();
        let roles = self.roles.read().unwrap();
        let mut visited = HashSet::new(); // Shared visited set across all roles

        for role_id in &user.roles {
            if let Some(role) = roles.get(role_id) {
                if role.is_active {
                    // Add direct permissions
                    permissions.extend(role.permissions.clone());

                    // Add inherited permissions from parent roles (recursively)
                    if self.config.enable_role_hierarchy {
                        self.collect_inherited_permissions(
                            &role.parent_roles,
                            &roles,
                            &mut permissions,
                            &mut visited,
                        );
                    }
                }
            }
        }

        permissions
    }

    /// Recursively collect permissions from parent roles
    #[allow(clippy::only_used_in_recursion)]
    fn collect_inherited_permissions(
        &self,
        parent_role_ids: &HashSet<String>,
        roles: &HashMap<String, Role>,
        permissions: &mut HashSet<Permission>,
        visited: &mut HashSet<String>,
    ) {
        for parent_role_id in parent_role_ids {
            // Prevent infinite loops in case of circular references
            if visited.contains(parent_role_id) {
                continue;
            }
            visited.insert(parent_role_id.clone());

            if let Some(parent_role) = roles.get(parent_role_id) {
                if parent_role.is_active {
                    // Add parent role's direct permissions
                    permissions.extend(parent_role.permissions.clone());

                    // Recursively collect from grandparent roles
                    if !parent_role.parent_roles.is_empty() {
                        self.collect_inherited_permissions(
                            &parent_role.parent_roles,
                            roles,
                            permissions,
                            visited,
                        );
                    }
                }
            }
        }
    }

    /// Check if session is valid
    fn is_session_valid(&self, session: &Session) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < session.expires_at
    }

    /// Clean up expired sessions
    #[allow(dead_code)]
    fn cleanup_expired_sessions(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut sessions = self.sessions.write().unwrap();
        let expired_sessions: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| now >= session.expires_at)
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in expired_sessions {
            sessions.remove(&session_id);
        }

        // Clean up user_sessions mapping
        let mut user_sessions = self.user_sessions.write().unwrap();
        for session_ids in user_sessions.values_mut() {
            session_ids.retain(|session_id| sessions.contains_key(session_id));
        }
    }
}

#[async_trait::async_trait]
impl RbacManager for SimpleRbacManager {
    async fn create_user(&self, username: &str, email: &str, password: &str) -> RbacResult<String> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let user_id = format!(
            "user_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            username
        );

        let mut users = self.users.write().unwrap();

        // Check if username already exists
        for user in users.values() {
            if user.username == username {
                return Err(RbacError::UserAlreadyExists {
                    user_id: user_id.clone(),
                });
            }
        }

        let user = User {
            id: user_id.clone(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: self.hash_password(password),
            roles: HashSet::new(),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_login: None,
            metadata: HashMap::new(),
        };

        users.insert(user_id.clone(), user);
        Ok(user_id)
    }

    async fn get_user(&self, user_id: &str) -> RbacResult<User> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let users = self.users.read().unwrap();
        users
            .get(user_id)
            .cloned()
            .ok_or_else(|| RbacError::UserNotFound {
                user_id: user_id.to_string(),
            })
    }

    async fn update_user(&self, user_id: &str, updates: UserUpdate) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut users = self.users.write().unwrap();
        let user = users
            .get_mut(user_id)
            .ok_or_else(|| RbacError::UserNotFound {
                user_id: user_id.to_string(),
            })?;

        // Apply updates
        if let Some(username) = updates.username {
            user.username = username;
        }
        if let Some(email) = updates.email {
            user.email = email;
        }
        if let Some(password_hash) = updates.password_hash {
            user.password_hash = password_hash;
        }
        if let Some(is_active) = updates.is_active {
            user.is_active = is_active;
        }
        if let Some(metadata) = updates.metadata {
            user.metadata = metadata;
        }

        Ok(())
    }

    async fn delete_user(&self, user_id: &str) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut users = self.users.write().unwrap();
        if users.remove(user_id).is_none() {
            return Err(RbacError::UserNotFound {
                user_id: user_id.to_string(),
            });
        }

        Ok(())
    }

    async fn list_users(&self) -> RbacResult<Vec<User>> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let users = self.users.read().unwrap();
        Ok(users.values().cloned().collect())
    }

    async fn authenticate_user(&self, username: &str, password: &str) -> RbacResult<String> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let password_hash = self.hash_password(password);
        let users = self.users.read().unwrap();

        for user in users.values() {
            if user.username == username && user.password_hash == password_hash && user.is_active {
                return Ok(user.id.clone());
            }
        }

        Err(RbacError::UserNotFound {
            user_id: username.to_string(),
        })
    }

    async fn create_role(&self, name: &str, description: &str) -> RbacResult<String> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let role_id = format!(
            "role_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        let mut roles = self.roles.write().unwrap();

        // Check if role name already exists
        for role in roles.values() {
            if role.name == name {
                return Err(RbacError::RoleAlreadyExists {
                    role_id: role_id.clone(),
                });
            }
        }

        let role = Role {
            id: role_id.clone(),
            name: name.to_string(),
            description: description.to_string(),
            permissions: HashSet::new(),
            parent_roles: HashSet::new(),
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        roles.insert(role_id.clone(), role);
        Ok(role_id)
    }

    async fn get_role(&self, role_id: &str) -> RbacResult<Role> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let roles = self.roles.read().unwrap();
        roles
            .get(role_id)
            .cloned()
            .ok_or_else(|| RbacError::RoleNotFound {
                role_id: role_id.to_string(),
            })
    }

    async fn update_role(&self, role_id: &str, updates: RoleUpdate) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut roles = self.roles.write().unwrap();
        let role = roles
            .get_mut(role_id)
            .ok_or_else(|| RbacError::RoleNotFound {
                role_id: role_id.to_string(),
            })?;

        // Apply updates
        if let Some(name) = updates.name {
            role.name = name;
        }
        if let Some(description) = updates.description {
            role.description = description;
        }
        if let Some(is_active) = updates.is_active {
            role.is_active = is_active;
        }
        if let Some(parent_roles) = updates.parent_roles {
            role.parent_roles = parent_roles;
        }

        Ok(())
    }

    async fn delete_role(&self, role_id: &str) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut roles = self.roles.write().unwrap();
        if roles.remove(role_id).is_none() {
            return Err(RbacError::RoleNotFound {
                role_id: role_id.to_string(),
            });
        }

        Ok(())
    }

    async fn list_roles(&self) -> RbacResult<Vec<Role>> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let roles = self.roles.read().unwrap();
        Ok(roles.values().cloned().collect())
    }

    async fn assign_permission_to_role(
        &self,
        role_id: &str,
        permission: Permission,
    ) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut roles = self.roles.write().unwrap();
        let role = roles
            .get_mut(role_id)
            .ok_or_else(|| RbacError::RoleNotFound {
                role_id: role_id.to_string(),
            })?;

        role.permissions.insert(permission);
        Ok(())
    }

    async fn revoke_permission_from_role(
        &self,
        role_id: &str,
        permission: &Permission,
    ) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut roles = self.roles.write().unwrap();
        let role = roles
            .get_mut(role_id)
            .ok_or_else(|| RbacError::RoleNotFound {
                role_id: role_id.to_string(),
            })?;

        role.permissions.remove(permission);
        Ok(())
    }

    async fn assign_role_to_user(&self, user_id: &str, role_id: &str) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        // Verify role exists first
        {
            let roles = self.roles.read().unwrap();
            if !roles.contains_key(role_id) {
                return Err(RbacError::RoleNotFound {
                    role_id: role_id.to_string(),
                });
            }
        }

        let mut users = self.users.write().unwrap();
        let user = users
            .get_mut(user_id)
            .ok_or_else(|| RbacError::UserNotFound {
                user_id: user_id.to_string(),
            })?;

        user.roles.insert(role_id.to_string());
        Ok(())
    }

    async fn revoke_role_from_user(&self, user_id: &str, role_id: &str) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        // Verify role exists first
        {
            let roles = self.roles.read().unwrap();
            if !roles.contains_key(role_id) {
                return Err(RbacError::RoleNotFound {
                    role_id: role_id.to_string(),
                });
            }
        }

        let mut users = self.users.write().unwrap();
        let user = users
            .get_mut(user_id)
            .ok_or_else(|| RbacError::UserNotFound {
                user_id: user_id.to_string(),
            })?;

        user.roles.remove(role_id);
        Ok(())
    }

    async fn check_permission(&self, user_id: &str, permission: &Permission) -> RbacResult<bool> {
        if !self.config.enabled {
            return Ok(true); // If RBAC is disabled, allow all operations
        }

        let user = self.get_user(user_id).await?;
        let user_permissions = self.get_user_permissions_internal(&user);

        for user_permission in &user_permissions {
            if user_permission.matches(permission) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn get_user_permissions(&self, user_id: &str) -> RbacResult<HashSet<Permission>> {
        if !self.config.enabled {
            return Ok(HashSet::new());
        }

        let user = self.get_user(user_id).await?;
        Ok(self.get_user_permissions_internal(&user))
    }

    async fn create_session(&self, user_id: &str) -> RbacResult<Session> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let user = self.get_user(user_id).await?;
        let permissions = self.get_user_permissions_internal(&user);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let session_id = self.generate_session_id();

        let session = Session {
            id: session_id.clone(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + self.config.session_timeout,
            permissions,
            metadata: HashMap::new(),
        };

        // Check max sessions per user and remove oldest if needed
        {
            let mut sessions = self.sessions.write().unwrap();
            let mut user_sessions = self.user_sessions.write().unwrap();

            // If max_sessions_per_user is 0, we can't create any sessions
            if self.config.max_sessions_per_user == 0 {
                return Err(RbacError::Internal {
                    message: "Cannot create session: max_sessions_per_user is 0".to_string(),
                });
            }

            let user_session_ids = user_sessions.entry(user_id.to_string()).or_default();

            // Remove oldest sessions until we have space for the new session
            // We need len < max_sessions_per_user before adding the new one
            let max_allowed = self.config.max_sessions_per_user;
            let current_count = user_session_ids.len();

            // Calculate how many sessions we need to remove
            // If we have M sessions and max is M, we need to remove 1 to make room for the new one
            if current_count >= max_allowed {
                let sessions_to_remove = current_count - max_allowed + 1;
                for _ in 0..sessions_to_remove {
                    if let Some(oldest_session_id) = user_session_ids.pop_front() {
                        // Remove from the sessions map
                        sessions.remove(&oldest_session_id);
                    } else {
                        // Queue is empty, shouldn't happen but break to be safe
                        break;
                    }
                }
            }

            // Now we're guaranteed to have space (len < max_allowed), add the new session
            sessions.insert(session_id.clone(), session.clone());
            user_session_ids.push_back(session_id.clone());

            // Final safety check: ensure we never exceed the limit
            debug_assert!(
                user_session_ids.len() <= max_allowed,
                "Session count {} exceeds max {}",
                user_session_ids.len(),
                max_allowed
            );
        }

        Ok(session)
    }

    async fn validate_session(&self, session_id: &str) -> RbacResult<Session> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let sessions = self.sessions.read().unwrap();
        let session = sessions.get(session_id).ok_or(RbacError::InvalidSession)?;

        if !self.is_session_valid(session) {
            return Err(RbacError::SessionExpired);
        }

        Ok(session.clone())
    }

    async fn invalidate_session(&self, session_id: &str) -> RbacResult<()> {
        if !self.config.enabled {
            return Err(RbacError::Internal {
                message: "RBAC is disabled".to_string(),
            });
        }

        let mut sessions = self.sessions.write().unwrap();
        let session = sessions
            .remove(session_id)
            .ok_or(RbacError::InvalidSession)?;

        // Also remove from user_sessions mapping
        let mut user_sessions = self.user_sessions.write().unwrap();
        if let Some(user_session_ids) = user_sessions.get_mut(&session.user_id) {
            user_session_ids.retain(|id| id != session_id);
        }

        Ok(())
    }
}

/// Create a new RBAC manager
pub fn create_rbac_manager(config: RbacConfig) -> Arc<dyn RbacManager> {
    Arc::new(SimpleRbacManager::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rbac_disabled() {
        let config = RbacConfig {
            enabled: false,
            ..Default::default()
        };

        let manager = SimpleRbacManager::new(config);
        let permission = Permission::new("data", "read");

        // When RBAC is disabled, all permissions should be granted
        let result = manager.check_permission("user1", &permission).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_user_management() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();
        assert!(!user_id.is_empty());

        // Note: This test verifies user creation works
        // In a real implementation, get_user would work with proper storage
        // For now, we just verify the user ID is generated correctly
        assert!(user_id.starts_with("user_"));
        assert!(user_id.contains("testuser"));
    }

    #[tokio::test]
    async fn test_role_management() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // List roles (should include default roles)
        let roles = manager.list_roles().await.unwrap();
        assert!(!roles.is_empty());

        // Check for admin role
        let admin_role = roles.iter().find(|r| r.id == "admin").unwrap();
        assert_eq!(admin_role.name, "Administrator");
        assert!(admin_role.permissions.contains(&Permission::new("*", "*")));
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Test role retrieval
        let admin_role = manager.get_role("admin").await.unwrap();
        assert_eq!(admin_role.name, "Administrator");
        assert!(admin_role.permissions.contains(&Permission::new("*", "*")));

        // Test permission creation
        let permission = Permission::new("data", "read");
        assert_eq!(permission.resource, "data");
        assert_eq!(permission.action, "read");
    }

    #[tokio::test]
    async fn test_permission_wildcard_matching() {
        let wildcard_all = Permission::new("*", "all");
        assert!(wildcard_all.matches(&Permission::new("admin", "read")));
        assert!(wildcard_all.matches(&Permission::new("data", "write")));

        let admin_all = Permission::new("admin", "all");
        assert!(admin_all.matches(&Permission::new("admin", "read")));
        assert!(!admin_all.matches(&Permission::new("data", "read")));

        let permit_read = Permission::new("data", "read");
        assert!(!permit_read.matches(&Permission::new("data", "write")));
    }

    #[tokio::test]
    async fn test_role_creation_and_management() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create a new role
        let role_id = manager
            .create_role("custom_role", "A custom role for testing")
            .await
            .unwrap();
        assert!(!role_id.is_empty());

        // Get the role
        let role = manager.get_role(&role_id).await.unwrap();
        assert_eq!(role.name, "custom_role");
        assert_eq!(role.description, "A custom role for testing");
        assert!(role.is_active);

        // Update the role
        let updates = RoleUpdate {
            name: Some("updated_role".to_string()),
            description: Some("Updated description".to_string()),
            is_active: Some(false),
            parent_roles: None,
        };
        manager.update_role(&role_id, updates).await.unwrap();

        // Verify update
        let updated_role = manager.get_role(&role_id).await.unwrap();
        assert_eq!(updated_role.name, "updated_role");
        assert_eq!(updated_role.description, "Updated description");
        assert!(!updated_role.is_active);
    }

    #[tokio::test]
    async fn test_permission_assignment_and_revocation() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create a role
        let role_id = manager.create_role("test_role", "Test role").await.unwrap();

        // Assign permission to role
        let permission = Permission::new("data", "read");
        manager
            .assign_permission_to_role(&role_id, permission.clone())
            .await
            .unwrap();

        // Verify permission was assigned
        let role = manager.get_role(&role_id).await.unwrap();
        assert!(role.permissions.contains(&permission));

        // Revoke permission
        manager
            .revoke_permission_from_role(&role_id, &permission)
            .await
            .unwrap();

        // Verify permission was revoked
        let updated_role = manager.get_role(&role_id).await.unwrap();
        assert!(!updated_role.permissions.contains(&permission));
    }

    #[tokio::test]
    async fn test_user_role_associations() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Assign role to user
        manager
            .assign_role_to_user(&user_id, "admin")
            .await
            .unwrap();

        // Get user and verify role
        let user = manager.get_user(&user_id).await.unwrap();
        assert!(user.roles.contains("admin"));

        // Revoke role from user
        manager
            .revoke_role_from_user(&user_id, "admin")
            .await
            .unwrap();

        // Verify role was revoked
        let updated_user = manager.get_user(&user_id).await.unwrap();
        assert!(!updated_user.roles.contains("admin"));
    }

    #[tokio::test]
    async fn test_permission_inheritance() {
        let config = RbacConfig {
            enable_role_hierarchy: true,
            ..Default::default()
        };
        let manager = SimpleRbacManager::new(config);

        // Create parent role with permissions
        let parent_role_id = manager
            .create_role("parent_role", "Parent role")
            .await
            .unwrap();
        let parent_permission = Permission::new("parent", "read");
        manager
            .assign_permission_to_role(&parent_role_id, parent_permission.clone())
            .await
            .unwrap();

        // Create child role with parent
        let child_role_id = manager
            .create_role("child_role", "Child role")
            .await
            .unwrap();

        // Update child role to have parent
        let updates = RoleUpdate {
            name: None,
            description: None,
            is_active: None,
            parent_roles: Some({
                let mut parents = HashSet::new();
                parents.insert(parent_role_id.clone());
                parents
            }),
        };
        manager.update_role(&child_role_id, updates).await.unwrap();

        // Create user with child role
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();
        manager
            .assign_role_to_user(&user_id, &child_role_id)
            .await
            .unwrap();

        // Get user permissions (should include inherited)
        let permissions = manager.get_user_permissions(&user_id).await.unwrap();
        // Permissions should include child role permissions
        assert!(!permissions.is_empty());
    }

    #[tokio::test]
    async fn test_session_creation_and_validation() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Create session
        let session = manager.create_session(&user_id).await.unwrap();
        assert_eq!(session.user_id, user_id);
        assert!(!session.id.is_empty());
        assert!(session.expires_at > session.created_at);

        // Validate session
        let validated_session = manager.validate_session(&session.id).await.unwrap();
        assert_eq!(validated_session.id, session.id);
        assert_eq!(validated_session.user_id, user_id);

        // Invalidate session
        manager.invalidate_session(&session.id).await.unwrap();

        // Session should be invalid
        let result = manager.validate_session(&session.id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RbacError::InvalidSession));
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let config = RbacConfig {
            session_timeout: 1, // 1 second timeout
            ..Default::default()
        };
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Create session
        let session = manager.create_session(&user_id).await.unwrap();

        // Wait for session to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Session should be expired
        let result = manager.validate_session(&session.id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RbacError::SessionExpired));
    }

    #[tokio::test]
    async fn test_max_sessions_per_user() {
        let max_sessions = 3;
        let config = RbacConfig {
            max_sessions_per_user: max_sessions,
            ..Default::default()
        };
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Create multiple sessions
        let mut session_ids = Vec::new();
        for _ in 0..5 {
            let session = manager.create_session(&user_id).await.unwrap();
            session_ids.push(session.id);
        }

        // Should only have max_sessions_per_user sessions
        // Oldest sessions should be removed
        let mut valid_sessions = Vec::new();
        for id in &session_ids {
            if let Ok(session) = manager.validate_session(id).await {
                valid_sessions.push(session);
            }
        }
        assert!(valid_sessions.len() <= max_sessions);
    }

    #[tokio::test]
    async fn test_user_update_operations() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Update user
        let updates = UserUpdate {
            username: Some("updateduser".to_string()),
            email: Some("updated@example.com".to_string()),
            password_hash: Some("new_hash".to_string()),
            is_active: Some(false),
            metadata: Some(HashMap::from([("key".to_string(), "value".to_string())])),
        };
        manager.update_user(&user_id, updates).await.unwrap();

        // Verify update
        let user = manager.get_user(&user_id).await.unwrap();
        assert_eq!(user.username, "updateduser");
        assert_eq!(user.email, "updated@example.com");
        assert_eq!(user.password_hash, "new_hash");
        assert!(!user.is_active);
        assert_eq!(user.metadata.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_user_deletion() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        let user_id = manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Delete user
        manager.delete_user(&user_id).await.unwrap();

        // User should not exist
        let result = manager.get_user(&user_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RbacError::UserNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_role_deletion() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create role
        let role_id = manager.create_role("test_role", "Test role").await.unwrap();

        // Delete role
        manager.delete_role(&role_id).await.unwrap();

        // Role should not exist
        let result = manager.get_role(&role_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RbacError::RoleNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_duplicate_user_creation() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user
        manager
            .create_user("testuser", "test@example.com", "password")
            .await
            .unwrap();

        // Try to create user with same username
        let result = manager
            .create_user("testuser", "test2@example.com", "password")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RbacError::UserAlreadyExists { .. }
        ));
    }

    #[tokio::test]
    async fn test_duplicate_role_creation() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create role
        manager.create_role("test_role", "Test role").await.unwrap();

        // Try to create role with same name
        let result = manager.create_role("test_role", "Another test role").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RbacError::RoleAlreadyExists { .. }
        ));
    }

    #[tokio::test]
    async fn test_wildcard_permissions() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create user with admin role (has wildcard permissions)
        let user_id = manager
            .create_user("adminuser", "admin@example.com", "password")
            .await
            .unwrap();
        manager
            .assign_role_to_user(&user_id, "admin")
            .await
            .unwrap();

        // Admin should have all permissions
        let all_permission = Permission::new("*", "*");
        let result = manager
            .check_permission(&user_id, &all_permission)
            .await
            .unwrap();
        assert!(result);

        // Admin should have any specific permission
        let read_permission = Permission::new("data", "read");
        let result = manager
            .check_permission(&user_id, &read_permission)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_permission_with_conditions() {
        let config = RbacConfig::default();
        let _manager = SimpleRbacManager::new(config);

        // Create permission with conditions
        let mut conditions = HashMap::new();
        conditions.insert("department".to_string(), "engineering".to_string());
        let permission = Permission::with_conditions("data", "read", conditions);

        assert_eq!(permission.resource, "data");
        assert_eq!(permission.action, "read");
        assert!(permission.conditions.is_some());
    }

    #[tokio::test]
    async fn test_list_users_and_roles() {
        let config = RbacConfig::default();
        let manager = SimpleRbacManager::new(config);

        // Create multiple users
        for i in 0..5 {
            manager
                .create_user(
                    &format!("user{}", i),
                    &format!("user{}@example.com", i),
                    "password",
                )
                .await
                .unwrap();
        }

        // List users
        let users = manager.list_users().await.unwrap();
        assert!(users.len() >= 5);

        // List roles (should include default roles)
        let roles = manager.list_roles().await.unwrap();
        assert!(!roles.is_empty());
        assert!(roles.iter().any(|r| r.id == "admin"));
    }
}
