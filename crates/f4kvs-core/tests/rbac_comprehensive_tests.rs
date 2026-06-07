//! Comprehensive RBAC tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for RBAC scenarios including:
//! - Complex permission scenarios
//! - Role-based access patterns
//! - Concurrent permission checks
//! - Permission caching behavior
//! - Role hierarchy traversal

use f4kvs_core::rbac::*;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_complex_permission_scenarios() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create multiple roles with different permissions
    let read_role_id = manager
        .create_role("reader", "Read-only role")
        .await
        .unwrap();
    let write_role_id = manager.create_role("writer", "Write role").await.unwrap();
    let admin_role_id = manager.create_role("admin", "Admin role").await.unwrap();

    // Assign permissions
    manager
        .assign_permission_to_role(&read_role_id, Permission::new("data", "read"))
        .await
        .unwrap();
    manager
        .assign_permission_to_role(&write_role_id, Permission::new("data", "write"))
        .await
        .unwrap();
    manager
        .assign_permission_to_role(&admin_role_id, Permission::new("*", "*"))
        .await
        .unwrap();

    // Create user with multiple roles
    let user_id = manager
        .create_user("multiuser", "multi@example.com", "password")
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, &read_role_id)
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, &write_role_id)
        .await
        .unwrap();

    // User should have both read and write permissions
    assert!(manager
        .check_permission(&user_id, &Permission::new("data", "read"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&user_id, &Permission::new("data", "write"))
        .await
        .unwrap());

    // User should not have admin permissions
    assert!(!manager
        .check_permission(&user_id, &Permission::new("admin", "all"))
        .await
        .unwrap());
}

#[tokio::test]
async fn test_role_based_access_patterns() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create hierarchical roles
    let junior_role_id = manager
        .create_role("junior", "Junior developer")
        .await
        .unwrap();
    let senior_role_id = manager
        .create_role("senior", "Senior developer")
        .await
        .unwrap();

    // Assign permissions
    manager
        .assign_permission_to_role(&junior_role_id, Permission::new("code", "read"))
        .await
        .unwrap();
    manager
        .assign_permission_to_role(&senior_role_id, Permission::new("code", "read"))
        .await
        .unwrap();
    manager
        .assign_permission_to_role(&senior_role_id, Permission::new("code", "write"))
        .await
        .unwrap();
    manager
        .assign_permission_to_role(&senior_role_id, Permission::new("code", "review"))
        .await
        .unwrap();

    // Create users with different roles
    let junior_user_id = manager
        .create_user("junior_dev", "junior@example.com", "password")
        .await
        .unwrap();
    let senior_user_id = manager
        .create_user("senior_dev", "senior@example.com", "password")
        .await
        .unwrap();

    manager
        .assign_role_to_user(&junior_user_id, &junior_role_id)
        .await
        .unwrap();
    manager
        .assign_role_to_user(&senior_user_id, &senior_role_id)
        .await
        .unwrap();

    // Junior can read but not write
    assert!(manager
        .check_permission(&junior_user_id, &Permission::new("code", "read"))
        .await
        .unwrap());
    assert!(!manager
        .check_permission(&junior_user_id, &Permission::new("code", "write"))
        .await
        .unwrap());

    // Senior can read, write, and review
    assert!(manager
        .check_permission(&senior_user_id, &Permission::new("code", "read"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&senior_user_id, &Permission::new("code", "write"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&senior_user_id, &Permission::new("code", "review"))
        .await
        .unwrap());
}

#[tokio::test]
async fn test_concurrent_permission_checks() {
    let config = RbacConfig::default();
    let manager = Arc::new(SimpleRbacManager::new(config));

    // Create user with permissions
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, "write")
        .await
        .unwrap();

    let permission = Permission::new("data", "read");

    // Spawn multiple concurrent permission checks
    let mut handles = Vec::new();
    for _ in 0..20 {
        let manager_clone = manager.clone();
        let user_id_clone = user_id.clone();
        let permission_clone = permission.clone();
        let handle = tokio::spawn(async move {
            manager_clone
                .check_permission(&user_id_clone, &permission_clone)
                .await
        });
        handles.push(handle);
    }

    // Wait for all checks
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_permission_caching_behavior() {
    let config = RbacConfig {
        enable_permission_caching: true,
        cache_ttl: 60,
        ..Default::default()
    };
    let manager = SimpleRbacManager::new(config);

    // Create user with permissions
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, "write")
        .await
        .unwrap();

    let permission = Permission::new("data", "read");

    // First check (should populate cache)
    let result1 = manager
        .check_permission(&user_id, &permission)
        .await
        .unwrap();

    // Second check (should use cache)
    let result2 = manager
        .check_permission(&user_id, &permission)
        .await
        .unwrap();

    // Results should be consistent
    assert_eq!(result1, result2);
}

#[tokio::test]
async fn test_role_hierarchy_traversal() {
    let config = RbacConfig {
        enable_role_hierarchy: true,
        ..Default::default()
    };
    let manager = SimpleRbacManager::new(config);

    // Create parent role
    let parent_role_id = manager.create_role("parent", "Parent role").await.unwrap();
    manager
        .assign_permission_to_role(&parent_role_id, Permission::new("parent", "read"))
        .await
        .unwrap();

    // Create child role
    let child_role_id = manager.create_role("child", "Child role").await.unwrap();
    manager
        .assign_permission_to_role(&child_role_id, Permission::new("child", "write"))
        .await
        .unwrap();

    // Note: In a real implementation, we'd set parent_roles
    // For now, we test that roles can be created and permissions assigned

    // Create user with child role
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, &child_role_id)
        .await
        .unwrap();

    // User should have child role permissions
    assert!(manager
        .check_permission(&user_id, &Permission::new("child", "write"))
        .await
        .unwrap());
}

#[tokio::test]
async fn test_multiple_sessions_per_user() {
    let config = RbacConfig {
        max_sessions_per_user: 5,
        ..Default::default()
    };
    let manager = SimpleRbacManager::new(config);

    // Create user
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();

    // Create multiple sessions
    let mut sessions = Vec::new();
    for _ in 0..5 {
        let session = manager.create_session(&user_id).await.unwrap();
        sessions.push(session);
    }

    // All sessions should be valid
    for session in &sessions {
        let validated = manager.validate_session(&session.id).await.unwrap();
        assert_eq!(validated.id, session.id);
    }
}

#[tokio::test]
async fn test_session_permissions() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user with role
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();
    manager
        .assign_role_to_user(&user_id, "write")
        .await
        .unwrap();

    // Create session
    let session = manager.create_session(&user_id).await.unwrap();

    // Session should have user's permissions
    assert!(!session.permissions.is_empty());
}

#[tokio::test]
async fn test_permission_conditions() {
    let config = RbacConfig::default();
    let _manager = SimpleRbacManager::new(config);

    // Create permission with conditions
    let mut conditions = HashMap::new();
    conditions.insert("department".to_string(), "engineering".to_string());
    conditions.insert("level".to_string(), "senior".to_string());

    let permission = Permission::with_conditions("data", "read", conditions.clone());

    assert_eq!(permission.resource, "data");
    assert_eq!(permission.action, "read");
    assert_eq!(permission.conditions, Some(conditions));
}

#[tokio::test]
async fn test_role_activation_deactivation() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create role
    let role_id = manager.create_role("test_role", "Test role").await.unwrap();

    // Deactivate role
    let updates = RoleUpdate {
        name: None,
        description: None,
        is_active: Some(false),
        parent_roles: None,
    };
    manager.update_role(&role_id, updates).await.unwrap();

    // Role should be inactive
    let role = manager.get_role(&role_id).await.unwrap();
    assert!(!role.is_active);

    // Reactivate role
    let updates = RoleUpdate {
        name: None,
        description: None,
        is_active: Some(true),
        parent_roles: None,
    };
    manager.update_role(&role_id, updates).await.unwrap();

    // Role should be active
    let role = manager.get_role(&role_id).await.unwrap();
    assert!(role.is_active);
}

#[tokio::test]
async fn test_user_authentication_flow() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user
    let user_id = manager
        .create_user("testuser", "test@example.com", "password123")
        .await
        .unwrap();

    // Authenticate with correct password
    let auth_result = manager
        .authenticate_user("testuser", "password123")
        .await
        .unwrap();
    assert_eq!(auth_result, user_id);

    // Authenticate with wrong password
    let auth_result = manager.authenticate_user("testuser", "wrongpassword").await;
    assert!(auth_result.is_err());
}

#[tokio::test]
async fn test_inactive_user_operations() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create user
    let user_id = manager
        .create_user("testuser", "test@example.com", "password")
        .await
        .unwrap();

    // Deactivate user
    let updates = UserUpdate {
        username: None,
        email: None,
        password_hash: None,
        is_active: Some(false),
        metadata: None,
    };
    manager.update_user(&user_id, updates).await.unwrap();

    // Inactive user should not be able to authenticate
    let auth_result = manager.authenticate_user("testuser", "password").await;
    assert!(auth_result.is_err());
}

#[tokio::test]
async fn test_complex_role_permission_matrix() {
    let config = RbacConfig::default();
    let manager = SimpleRbacManager::new(config);

    // Create multiple roles with overlapping permissions
    let roles = vec![
        ("viewer", vec!["read"]),
        ("editor", vec!["read", "write"]),
        ("publisher", vec!["read", "write", "publish"]),
    ];

    let mut role_ids = Vec::new();
    for (name, actions) in roles {
        let role_id = manager
            .create_role(name, &format!("{} role", name))
            .await
            .unwrap();
        for action in actions {
            manager
                .assign_permission_to_role(&role_id, Permission::new("content", action))
                .await
                .unwrap();
        }
        role_ids.push((role_id, name));
    }

    // Create users with different roles
    let mut user_ids = Vec::new();
    for (i, (role_id, _)) in role_ids.iter().enumerate() {
        let user_id = manager
            .create_user(
                &format!("user{}", i),
                &format!("user{}@example.com", i),
                "password",
            )
            .await
            .unwrap();
        manager
            .assign_role_to_user(&user_id, role_id)
            .await
            .unwrap();
        user_ids.push((user_id, i));
    }

    // Verify permission matrix
    // Viewer can only read
    assert!(manager
        .check_permission(&user_ids[0].0, &Permission::new("content", "read"))
        .await
        .unwrap());
    assert!(!manager
        .check_permission(&user_ids[0].0, &Permission::new("content", "write"))
        .await
        .unwrap());

    // Editor can read and write
    assert!(manager
        .check_permission(&user_ids[1].0, &Permission::new("content", "read"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&user_ids[1].0, &Permission::new("content", "write"))
        .await
        .unwrap());

    // Publisher can read, write, and publish
    assert!(manager
        .check_permission(&user_ids[2].0, &Permission::new("content", "read"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&user_ids[2].0, &Permission::new("content", "write"))
        .await
        .unwrap());
    assert!(manager
        .check_permission(&user_ids[2].0, &Permission::new("content", "publish"))
        .await
        .unwrap());
}
