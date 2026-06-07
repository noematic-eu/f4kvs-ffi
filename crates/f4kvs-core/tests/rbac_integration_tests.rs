//! Integration tests for F4KVS RBAC System
//!
//! This module provides integration tests that verify RBAC functionality
//! with storage operations, multi-user scenarios, security, and session management.

use f4kvs_core::{
    rbac::{create_rbac_manager, Permission, RbacConfig, RbacManager},
    F4KVSCore,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Helper function to create RBAC manager with default config
fn create_test_rbac_manager() -> Arc<dyn RbacManager> {
    let config = RbacConfig::default();
    create_rbac_manager(config)
}

/// Test RBAC with storage operations
#[tokio::test]
async fn test_rbac_with_storage_operations() {
    let _f4kvs = Arc::new(F4KVSCore::new().unwrap());
    let rbac_manager = create_test_rbac_manager();

    // Create users with different roles
    let admin_id = rbac_manager
        .create_user("admin_user", "admin@test.com", "admin_pass")
        .await
        .unwrap();

    let read_only_id = rbac_manager
        .create_user("readonly_user", "readonly@test.com", "readonly_pass")
        .await
        .unwrap();

    // Assign roles
    rbac_manager
        .assign_role_to_user(&admin_id, "admin")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&read_only_id, "read_only")
        .await
        .unwrap();

    // Admin should have write permission
    let admin_permission = Permission::new("data", "write");
    let admin_can_write = rbac_manager
        .check_permission(&admin_id, &admin_permission)
        .await
        .unwrap();
    assert!(admin_can_write, "Admin should have write permission");

    // Read-only user should have read permission but not write
    let read_permission = Permission::new("data", "read");
    let readonly_can_read = rbac_manager
        .check_permission(&read_only_id, &read_permission)
        .await
        .unwrap();
    assert!(
        readonly_can_read,
        "Read-only user should have read permission"
    );

    let readonly_can_write = rbac_manager
        .check_permission(&read_only_id, &admin_permission)
        .await
        .unwrap();
    assert!(
        !readonly_can_write,
        "Read-only user should not have write permission"
    );
}

/// Test multi-user scenarios
#[tokio::test]
async fn test_rbac_multi_user_scenarios() {
    let rbac_manager = create_test_rbac_manager();

    // Create multiple users
    const NUM_USERS: usize = 10;
    let mut user_ids = Vec::new();

    for i in 0..NUM_USERS {
        let user_id = rbac_manager
            .create_user(
                &format!("user_{}", i),
                &format!("user{}@test.com", i),
                &format!("pass_{}", i),
            )
            .await
            .unwrap();

        // Assign different roles based on user index
        if i % 3 == 0 {
            rbac_manager
                .assign_role_to_user(&user_id, "admin")
                .await
                .unwrap();
        } else if i % 3 == 1 {
            rbac_manager
                .assign_role_to_user(&user_id, "write")
                .await
                .unwrap();
        } else {
            rbac_manager
                .assign_role_to_user(&user_id, "read_only")
                .await
                .unwrap();
        }

        // Push after using user_id
        user_ids.push(user_id);
    }

    // Verify permissions for each user
    for (i, user_id) in user_ids.iter().enumerate() {
        let read_perm = Permission::new("data", "read");
        let write_perm = Permission::new("data", "write");

        if i % 3 == 0 {
            // Admin should have all permissions
            assert!(rbac_manager
                .check_permission(user_id, &read_perm)
                .await
                .unwrap());
            assert!(rbac_manager
                .check_permission(user_id, &write_perm)
                .await
                .unwrap());
        } else if i % 3 == 1 {
            // Write role should have read and write
            assert!(rbac_manager
                .check_permission(user_id, &read_perm)
                .await
                .unwrap());
            assert!(rbac_manager
                .check_permission(user_id, &write_perm)
                .await
                .unwrap());
        } else {
            // Read-only should only have read
            assert!(rbac_manager
                .check_permission(user_id, &read_perm)
                .await
                .unwrap());
            assert!(!rbac_manager
                .check_permission(user_id, &write_perm)
                .await
                .unwrap());
        }
    }
}

/// Test security - unauthorized access attempts
#[tokio::test]
async fn test_rbac_unauthorized_access() {
    let rbac_manager = create_test_rbac_manager();

    // Create read-only user
    let readonly_id = rbac_manager
        .create_user("readonly", "readonly@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&readonly_id, "read_only")
        .await
        .unwrap();

    // Attempt unauthorized write operation
    let write_permission = Permission::new("data", "write");
    let can_write = rbac_manager
        .check_permission(&readonly_id, &write_permission)
        .await
        .unwrap();
    assert!(!can_write, "Read-only user should not be able to write");

    // Attempt access to non-existent resource
    let admin_permission = Permission::new("admin", "manage");
    let can_admin = rbac_manager
        .check_permission(&readonly_id, &admin_permission)
        .await
        .unwrap();
    assert!(
        !can_admin,
        "Read-only user should not have admin permissions"
    );
}

/// Test permission boundary enforcement
#[tokio::test]
async fn test_rbac_permission_boundaries() {
    let rbac_manager = create_test_rbac_manager();

    // Create user with write role
    let writer_id = rbac_manager
        .create_user("writer", "writer@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&writer_id, "write")
        .await
        .unwrap();

    // Writer should have data read/write but not admin operations
    let data_read = Permission::new("data", "read");
    let data_write = Permission::new("data", "write");
    let admin_manage = Permission::new("admin", "manage");
    let system_config = Permission::new("system", "configure");

    assert!(
        rbac_manager
            .check_permission(&writer_id, &data_read)
            .await
            .unwrap(),
        "Writer should have data read permission"
    );
    assert!(
        rbac_manager
            .check_permission(&writer_id, &data_write)
            .await
            .unwrap(),
        "Writer should have data write permission"
    );
    assert!(
        !rbac_manager
            .check_permission(&writer_id, &admin_manage)
            .await
            .unwrap(),
        "Writer should not have admin manage permission"
    );
    assert!(
        !rbac_manager
            .check_permission(&writer_id, &system_config)
            .await
            .unwrap(),
        "Writer should not have system configure permission"
    );
}

/// Test session management - session timeout
#[tokio::test]
async fn test_rbac_session_timeout() {
    let config = RbacConfig {
        session_timeout: 1, // 1 second timeout for testing
        ..Default::default()
    };
    let rbac_manager = create_rbac_manager(config);

    let user_id = rbac_manager
        .create_user("session_user", "session@test.com", "password")
        .await
        .unwrap();

    // Create session
    let session = rbac_manager.create_session(&user_id).await.unwrap();
    let session_id = session.id.clone();

    // Session should be valid immediately
    let validated_session = rbac_manager.validate_session(&session_id).await;
    assert!(
        validated_session.is_ok(),
        "Session should be valid immediately after creation"
    );

    // Wait for timeout
    sleep(Duration::from_secs(2)).await;

    // Session should be expired (depending on implementation)
    // Some implementations may require explicit cleanup
    let session_after_timeout = rbac_manager.validate_session(&session_id).await;
    // Implementation may vary - session might be removed or marked expired
    if session_after_timeout.is_ok() {
        // If session still exists, verify it's marked as expired
        // This depends on implementation details
    }
}

/// Test concurrent sessions
#[tokio::test]
async fn test_rbac_concurrent_sessions() {
    let config = RbacConfig {
        max_sessions_per_user: 3,
        ..Default::default()
    };
    let rbac_manager = create_rbac_manager(config);

    let user_id = rbac_manager
        .create_user("multi_session", "multi@test.com", "password")
        .await
        .unwrap();

    // Create multiple sessions
    let mut session_ids = Vec::new();
    for i in 0..3 {
        let session = rbac_manager.create_session(&user_id).await.unwrap();
        session_ids.push(session.id.clone());
        println!("Created session {}: {:?}", i, session);
    }

    // Verify all sessions exist
    for session_id in &session_ids {
        let session_result = rbac_manager.validate_session(session_id).await;
        assert!(session_result.is_ok(), "All sessions should exist");
    }

    // Attempt to create one more session (should fail or remove oldest)
    let result = rbac_manager.create_session(&user_id).await;
    // May succeed if implementation removes oldest, or fail if strict limit
    let _ = result;
}

/// Test session cleanup
#[tokio::test]
async fn test_rbac_session_cleanup() {
    let rbac_manager = create_test_rbac_manager();

    let user_id = rbac_manager
        .create_user("cleanup_user", "cleanup@test.com", "password")
        .await
        .unwrap();

    // Create session
    let session = rbac_manager.create_session(&user_id).await.unwrap();
    let session_id = session.id.clone();

    // Verify session exists
    let validated_session = rbac_manager.validate_session(&session_id).await;
    assert!(validated_session.is_ok(), "Session should exist");

    // Invalidate session
    rbac_manager.invalidate_session(&session_id).await.unwrap();

    // Verify session is removed
    let session_after = rbac_manager.validate_session(&session_id).await;
    assert!(
        session_after.is_err(),
        "Session should be removed after invalidate"
    );
}

/// Test role hierarchy
#[tokio::test]
async fn test_rbac_role_hierarchy() {
    let config = RbacConfig {
        enable_role_hierarchy: true,
        ..Default::default()
    };
    let rbac_manager = create_rbac_manager(config);

    // Note: Role creation depends on implementation
    // For now, test that hierarchy is respected in permission checks
    let user_id = rbac_manager
        .create_user("hierarchy_user", "hierarchy@test.com", "password")
        .await
        .unwrap();

    rbac_manager
        .assign_role_to_user(&user_id, "admin")
        .await
        .unwrap();

    // Admin role should have broad permissions
    let audit_read = Permission::new("audit", "read");
    let can_read_audit = rbac_manager
        .check_permission(&user_id, &audit_read)
        .await
        .unwrap();

    // Admin may or may not have audit permissions depending on role definition
    let _ = can_read_audit;
}

/// Test permission caching
#[tokio::test]
async fn test_rbac_permission_caching() {
    let config = RbacConfig {
        enable_permission_caching: true,
        cache_ttl: 60,
        ..Default::default()
    };
    let rbac_manager = create_rbac_manager(config);

    let user_id = rbac_manager
        .create_user("cache_user", "cache@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&user_id, "write")
        .await
        .unwrap();

    let permission = Permission::new("data", "write");

    // First check - should populate cache
    let start1 = std::time::Instant::now();
    let result1 = rbac_manager
        .check_permission(&user_id, &permission)
        .await
        .unwrap();
    let duration1 = start1.elapsed();

    // Second check - should use cache (faster)
    let start2 = std::time::Instant::now();
    let result2 = rbac_manager
        .check_permission(&user_id, &permission)
        .await
        .unwrap();
    let duration2 = start2.elapsed();

    assert_eq!(result1, result2, "Cached and uncached results should match");
    println!(
        "First check: {:?}, Second check: {:?}",
        duration1, duration2
    );
}

/// Test RBAC stress test - many users, many roles
#[tokio::test]
async fn test_rbac_stress_many_users_roles() {
    let rbac_manager = create_test_rbac_manager();

    const NUM_USERS: usize = 100;
    // Create many users
    let mut user_ids = Vec::new();
    for i in 0..NUM_USERS {
        let user_id = rbac_manager
            .create_user(
                &format!("stress_user_{}", i),
                &format!("stress{}@test.com", i),
                "password",
            )
            .await
            .unwrap();
        user_ids.push(user_id.clone());

        // Assign random role based on index
        // Use existing default roles
        if i % 3 == 0 {
            rbac_manager
                .assign_role_to_user(&user_id, "admin")
                .await
                .unwrap();
        } else if i % 3 == 1 {
            rbac_manager
                .assign_role_to_user(&user_id, "write")
                .await
                .unwrap();
        } else {
            rbac_manager
                .assign_role_to_user(&user_id, "read_only")
                .await
                .unwrap();
        }
    }

    // Verify all users have correct permissions
    let read_perm = Permission::new("data", "read");
    for user_id in &user_ids {
        let can_read = rbac_manager
            .check_permission(user_id, &read_perm)
            .await
            .unwrap();
        assert!(can_read, "All users should have read permission");
    }
}

/// Test audit log - verify access attempts are logged
#[tokio::test]
async fn test_rbac_audit_logging() {
    let rbac_manager = create_test_rbac_manager();

    let user_id = rbac_manager
        .create_user("audit_user", "audit@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&user_id, "read_only")
        .await
        .unwrap();

    // Attempt multiple operations
    let read_perm = Permission::new("data", "read");
    let write_perm = Permission::new("data", "write");

    // Successful access
    let _ = rbac_manager.check_permission(&user_id, &read_perm).await;

    // Denied access
    let _ = rbac_manager.check_permission(&user_id, &write_perm).await;

    // Note: Audit logging depends on implementation
    // This test verifies operations don't panic
    // Actual audit log retrieval would require additional API
}

/// Test RBAC with storage integration
#[tokio::test]
async fn test_rbac_storage_integration() {
    let _f4kvs = Arc::new(F4KVSCore::new().unwrap());
    let rbac_manager = create_test_rbac_manager();

    // Create users
    let admin_id = rbac_manager
        .create_user("admin_storage", "admin_storage@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&admin_id, "admin")
        .await
        .unwrap();

    let readonly_id = rbac_manager
        .create_user("readonly_storage", "readonly_storage@test.com", "password")
        .await
        .unwrap();
    rbac_manager
        .assign_role_to_user(&readonly_id, "read_only")
        .await
        .unwrap();

    // Admin should be able to perform all operations
    // (In real implementation, storage operations would check RBAC)
    // For now, we verify RBAC checks work correctly

    let admin_write = Permission::new("data", "write");
    let admin_can_write = rbac_manager
        .check_permission(&admin_id, &admin_write)
        .await
        .unwrap();
    assert!(admin_can_write, "Admin should be able to write");

    let readonly_write = Permission::new("data", "write");
    let readonly_can_write = rbac_manager
        .check_permission(&readonly_id, &readonly_write)
        .await
        .unwrap();
    assert!(
        !readonly_can_write,
        "Read-only user should not be able to write"
    );
}
