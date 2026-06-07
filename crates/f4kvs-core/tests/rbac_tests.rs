//! Comprehensive tests for F4KVS Core RBAC System
//!
//! This module provides extensive test coverage for the Role-Based Access Control
//! functionality, including user management, role management, permission management,
//! and session handling.

use f4kvs_core::rbac::{
    create_rbac_manager, Permission, RbacConfig, RbacError, RbacManager, Role, RoleUpdate, Session,
    SimpleRbacManager, User, UserUpdate,
};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[tokio::test]
async fn test_rbac_config_default() {
    let config = RbacConfig::default();
    assert!(config.enabled);
    assert_eq!(config.session_timeout, 3600);
    assert_eq!(config.max_sessions_per_user, 5);
    assert!(config.enable_role_hierarchy);
    assert!(config.enable_permission_caching);
    assert_eq!(config.cache_ttl, 300);
}

#[tokio::test]
async fn test_rbac_config_custom() {
    let config = RbacConfig {
        enabled: false,
        session_timeout: 7200,
        max_sessions_per_user: 10,
        enable_role_hierarchy: false,
        enable_permission_caching: false,
        cache_ttl: 600,
    };

    assert!(!config.enabled);
    assert_eq!(config.session_timeout, 7200);
    assert_eq!(config.max_sessions_per_user, 10);
    assert!(!config.enable_role_hierarchy);
    assert!(!config.enable_permission_caching);
    assert_eq!(config.cache_ttl, 600);
}

#[tokio::test]
async fn test_permission_creation() {
    let permission = Permission::new("data", "read");
    assert_eq!(permission.resource, "data");
    assert_eq!(permission.action, "read");
    assert!(permission.conditions.is_none());
}

#[tokio::test]
async fn test_permission_with_conditions() {
    let mut conditions = HashMap::new();
    conditions.insert("department".to_string(), "engineering".to_string());
    conditions.insert("level".to_string(), "senior".to_string());

    let permission = Permission::with_conditions("data", "write", conditions.clone());
    assert_eq!(permission.resource, "data");
    assert_eq!(permission.action, "write");
    assert_eq!(permission.conditions, Some(conditions));
}

#[tokio::test]
async fn test_permission_matching() {
    let perm1 = Permission::new("data", "read");
    let perm2 = Permission::new("data", "read");
    let perm3 = Permission::new("data", "write");

    assert!(perm1.matches(&perm2));
    assert!(!perm1.matches(&perm3));
}

#[tokio::test]
async fn test_user_creation() {
    let user = User {
        id: "user1".to_string(),
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
        password_hash: "hash123".to_string(),
        roles: HashSet::new(),
        is_active: true,
        created_at: 1234567890,
        last_login: None,
        metadata: HashMap::new(),
    };

    assert_eq!(user.id, "user1");
    assert_eq!(user.username, "alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.password_hash, "hash123");
    assert!(user.roles.is_empty());
    assert!(user.is_active);
    assert_eq!(user.created_at, 1234567890);
    assert!(user.last_login.is_none());
    assert!(user.metadata.is_empty());
}

#[tokio::test]
async fn test_role_creation() {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::new("data", "read"));
    permissions.insert(Permission::new("data", "write"));

    let role = Role {
        id: "role1".to_string(),
        name: "Developer".to_string(),
        description: "Software developer role".to_string(),
        permissions,
        parent_roles: HashSet::new(),
        is_active: true,
        created_at: 1234567890,
    };

    assert_eq!(role.id, "role1");
    assert_eq!(role.name, "Developer");
    assert_eq!(role.description, "Software developer role");
    assert_eq!(role.permissions.len(), 2);
    assert!(role.parent_roles.is_empty());
    assert!(role.is_active);
    assert_eq!(role.created_at, 1234567890);
}

#[tokio::test]
async fn test_session_creation() {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::new("data", "read"));

    let session = Session {
        id: "session1".to_string(),
        user_id: "user1".to_string(),
        created_at: 1234567890,
        expires_at: 1234567890 + 3600,
        permissions,
        metadata: HashMap::new(),
    };

    assert_eq!(session.id, "session1");
    assert_eq!(session.user_id, "user1");
    assert_eq!(session.created_at, 1234567890);
    assert_eq!(session.expires_at, 1234567890 + 3600);
    assert_eq!(session.permissions.len(), 1);
    assert!(session.metadata.is_empty());
}

#[tokio::test]
async fn test_simple_rbac_manager_creation() {
    let config = RbacConfig::default();
    let _manager = SimpleRbacManager::new(config);

    // Test that manager can be created without issues
    assert!(true);
}

#[tokio::test]
async fn test_rbac_manager_creation() {
    let config = RbacConfig::default();
    let _manager = create_rbac_manager(config);

    // Test that manager can be created without issues
    assert!(true);
}

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
async fn test_user_creation_success() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();
    assert!(!user_id.is_empty());
    assert!(user_id.starts_with("user_"));
    assert!(user_id.contains("testuser"));
}

#[tokio::test]
async fn test_user_creation_duplicate_username() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create first user
    let _user_id1 = manager
        .create_user("testuser", "test1@example.com", "password")
        .await
        .unwrap();

    // Try to create user with same username
    let result = manager
        .create_user("testuser", "test2@example.com", "password")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserAlreadyExists { user_id } => {
            assert!(user_id.contains("testuser"));
        }
        _ => panic!("Expected UserAlreadyExists error"),
    }
}

#[tokio::test]
async fn test_user_creation_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager
        .create_user("testuser", "test@example.com", "password")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_get_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.get_user("nonexistent_user").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_get_user_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.get_user("user1").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_update_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let updates = UserUpdate {
        username: Some("newusername".to_string()),
        email: None,
        password_hash: None,
        is_active: None,
        metadata: None,
    };

    let result = manager.update_user("nonexistent_user", updates).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_delete_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.delete_user("nonexistent_user").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_list_users_empty() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let users = manager.list_users().await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_authenticate_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.authenticate_user("nonexistent", "password").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent");
        }
        _ => panic!("Expected UserNotFound error"),
    }
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

    // Check for read-only role
    let read_only_role = roles.iter().find(|r| r.id == "read_only").unwrap();
    assert_eq!(read_only_role.name, "Read Only");
    assert!(read_only_role
        .permissions
        .contains(&Permission::new("data", "read")));

    // Check for write role
    let write_role = roles.iter().find(|r| r.id == "write").unwrap();
    assert_eq!(write_role.name, "Write Access");
    assert!(write_role
        .permissions
        .contains(&Permission::new("data", "read")));
    assert!(write_role
        .permissions
        .contains(&Permission::new("data", "write")));
}

#[tokio::test]
async fn test_get_role_success() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let admin_role = manager.get_role("admin").await.unwrap();
    assert_eq!(admin_role.name, "Administrator");
    assert!(admin_role.permissions.contains(&Permission::new("*", "*")));
}

#[test]
fn test_permission_hash() {
    let perm1 = Permission::new("data", "read");
    let perm2 = Permission::new("data", "read");
    let perm3 = Permission::new("data", "write");

    // Same permissions should have same hash
    let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
    let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
    let mut hasher3 = std::collections::hash_map::DefaultHasher::new();

    perm1.hash(&mut hasher1);
    perm2.hash(&mut hasher2);
    perm3.hash(&mut hasher3);

    assert_eq!(hasher1.finish(), hasher2.finish());
    assert_ne!(hasher1.finish(), hasher3.finish());
}

#[tokio::test]
async fn test_permission_with_conditions_hash() {
    let mut conditions1 = HashMap::new();
    conditions1.insert("dept".to_string(), "eng".to_string());
    conditions1.insert("level".to_string(), "senior".to_string());

    let mut conditions2 = HashMap::new();
    conditions2.insert("level".to_string(), "senior".to_string());
    conditions2.insert("dept".to_string(), "eng".to_string());

    let perm1 = Permission::with_conditions("data", "read", conditions1);
    let perm2 = Permission::with_conditions("data", "read", conditions2);

    // Permissions with same conditions in different order should have same hash
    let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
    let mut hasher2 = std::collections::hash_map::DefaultHasher::new();

    perm1.hash(&mut hasher1);
    perm2.hash(&mut hasher2);

    assert_eq!(hasher1.finish(), hasher2.finish());
}

#[tokio::test]
async fn test_role_creation_success() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let role_id = manager
        .create_role("Developer", "Software developer role")
        .await
        .unwrap();
    assert!(!role_id.is_empty());
    assert!(role_id.starts_with("role_"));
}

#[tokio::test]
async fn test_role_creation_duplicate_name() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create first role
    let _role_id1 = manager
        .create_role("Developer", "First developer role")
        .await
        .unwrap();

    // Try to create role with same name
    let result = manager
        .create_role("Developer", "Second developer role")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleAlreadyExists { role_id } => {
            assert!(role_id.starts_with("role_"));
        }
        _ => panic!("Expected RoleAlreadyExists error"),
    }
}

#[tokio::test]
async fn test_get_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.get_role("nonexistent_role").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_update_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let updates = RoleUpdate {
        name: Some("New Name".to_string()),
        description: None,
        is_active: None,
        parent_roles: None,
    };

    let result = manager.update_role("nonexistent_role", updates).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_delete_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.delete_role("nonexistent_role").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_assign_permission_to_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let permission = Permission::new("data", "read");
    let result = manager
        .assign_permission_to_role("nonexistent_role", permission)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_revoke_permission_from_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let permission = Permission::new("data", "read");
    let result = manager
        .revoke_permission_from_role("nonexistent_role", &permission)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_assign_role_to_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager
        .assign_role_to_user("nonexistent_user", "admin")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_assign_role_to_user_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user first
    let _user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();

    let result = manager
        .assign_role_to_user("testuser", "nonexistent_role")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_revoke_role_from_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager
        .revoke_role_from_user("nonexistent_user", "admin")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_revoke_role_from_user_role_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user first
    let _user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();

    let result = manager
        .revoke_role_from_user("testuser", "nonexistent_role")
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::RoleNotFound { role_id } => {
            assert_eq!(role_id, "nonexistent_role");
        }
        _ => panic!("Expected RoleNotFound error"),
    }
}

#[tokio::test]
async fn test_check_permission_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let permission = Permission::new("data", "read");
    let result = manager
        .check_permission("nonexistent_user", &permission)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_get_user_permissions_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.get_user_permissions("nonexistent_user").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_create_session_user_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.create_session("nonexistent_user").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::UserNotFound { user_id } => {
            assert_eq!(user_id, "nonexistent_user");
        }
        _ => panic!("Expected UserNotFound error"),
    }
}

#[tokio::test]
async fn test_create_session_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.create_session("user1").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_validate_session_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.validate_session("nonexistent_session").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::InvalidSession => {}
        _ => panic!("Expected InvalidSession error"),
    }
}

#[tokio::test]
async fn test_validate_session_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.validate_session("session1").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_invalidate_session_not_found() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    let result = manager.invalidate_session("nonexistent_session").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::InvalidSession => {}
        _ => panic!("Expected InvalidSession error"),
    }
}

#[tokio::test]
async fn test_invalidate_session_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.invalidate_session("session1").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_get_user_permissions_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let permissions = manager.get_user_permissions("user1").await.unwrap();
    assert!(permissions.is_empty());
}

#[tokio::test]
async fn test_list_users_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.list_users().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}

#[tokio::test]
async fn test_list_roles_rbac_disabled() {
    let config = RbacConfig {
        enabled: false,
        ..Default::default()
    };

    let manager = SimpleRbacManager::new(config);

    let result = manager.list_roles().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        RbacError::Internal { message } => {
            assert_eq!(message, "RBAC is disabled");
        }
        _ => panic!("Expected Internal error"),
    }
}
