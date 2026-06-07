//! Comprehensive authentication tests for F4KVS Core
//!
//! This module provides comprehensive test coverage for authentication scenarios including:
//! - Multi-user authentication scenarios
//! - Concurrent authentication attempts
//! - Token refresh workflows
//! - Permission escalation attempts
//! - Account lockout mechanisms
//! - Password policy enforcement

use f4kvs_core::auth::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_multi_user_authentication() {
    let mut auth = AuthManager::new(AuthConfig::default());

    // Create multiple users with different roles
    let users = vec![
        ("user1", "user1@example.com", vec!["user".to_string()]),
        ("user2", "user2@example.com", vec!["user".to_string()]),
        ("admin1", "admin1@example.com", vec!["admin".to_string()]),
    ];

    let mut user_ids = Vec::new();
    for (username, email, roles) in users {
        let request = CreateUserRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: "password123".to_string(),
            roles: Some(roles),
            metadata: None,
        };
        let user = auth.create_user(request).await.unwrap();
        user_ids.push((user.id, username.to_string()));
    }

    // Authenticate all users
    for (user_id, username) in &user_ids {
        let context = auth.authenticate(username, "password123").await.unwrap();
        assert_eq!(context.user_id, *user_id);
    }
}

#[tokio::test]
async fn test_concurrent_authentication_attempts() {
    let mut auth = AuthManager::new(AuthConfig::default());

    let request = CreateUserRequest {
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        password: "password123".to_string(),
        roles: Some(vec!["user".to_string()]),
        metadata: None,
    };

    auth.create_user(request).await.unwrap();

    // Use Arc<Mutex> to share auth manager across tasks
    let auth_manager = Arc::new(Mutex::new(auth));

    // Spawn multiple concurrent authentication attempts
    let mut handles = Vec::new();
    for i in 0..10 {
        let auth_clone = auth_manager.clone();
        let handle = tokio::spawn(async move {
            let mut auth = auth_clone.lock().await;
            let result = auth.authenticate("testuser", "password123").await;
            (i, result.is_ok())
        });
        handles.push(handle);
    }

    // Wait for all attempts to complete
    let mut success_count = 0;
    for handle in handles {
        let (_, success) = handle.await.unwrap();
        if success {
            success_count += 1;
        }
    }

    // All concurrent authentications should succeed
    assert_eq!(success_count, 10);
}

#[tokio::test]
async fn test_token_refresh_workflow() {
    let mut auth = AuthManager::new(AuthConfig {
        jwt_expiry_seconds: 60, // Short expiry for testing
        ..Default::default()
    });

    let request = CreateUserRequest {
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        password: "password123".to_string(),
        roles: Some(vec!["user".to_string()]),
        metadata: None,
    };

    auth.create_user(request).await.unwrap();
    let context1 = auth.authenticate("testuser", "password123").await.unwrap();

    // Verify initial token
    let verified1 = auth.verify_token(&context1.token).await.unwrap();
    assert_eq!(verified1.user_id, context1.user_id);

    // Authenticate again to get a new token (simulating refresh)
    let context2 = auth.authenticate("testuser", "password123").await.unwrap();

    // Both tokens should be valid
    let verified2 = auth.verify_token(&context2.token).await.unwrap();
    assert_eq!(verified2.user_id, context1.user_id);
    assert_ne!(context1.token, context2.token); // Tokens should be different
}

#[tokio::test]
async fn test_permission_escalation_attempts() {
    let mut auth = AuthManager::new(AuthConfig::default());

    // Create a regular user
    let user_request = CreateUserRequest {
        username: "regularuser".to_string(),
        email: "regular@example.com".to_string(),
        password: "password123".to_string(),
        roles: Some(vec!["user".to_string()]),
        metadata: None,
    };

    auth.create_user(user_request).await.unwrap();
    let context = auth
        .authenticate("regularuser", "password123")
        .await
        .unwrap();

    // Regular user should NOT have admin permissions
    assert!(!auth.has_permission(&context, "namespace:admin:data", "admin"));
    assert!(!auth.has_permission(&context, "*", "*"));

    // Regular user should have their own namespace permissions
    assert!(auth.has_permission(&context, "namespace:user:data", "read"));
    assert!(auth.has_permission(&context, "namespace:user:data", "write"));
}

#[tokio::test]
async fn test_account_lockout_mechanism() {
    let mut auth = AuthManager::new(AuthConfig {
        max_login_attempts: 3,
        lockout_duration_seconds: 2, // Short lockout for testing
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
        let result = auth.authenticate("testuser", "wrongpassword").await;
        assert!(result.is_err());
    }

    // Account should be locked out
    let result = auth.authenticate("testuser", "password123").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));

    // Wait for lockout to expire
    sleep(Duration::from_secs(3)).await;

    // Should be able to authenticate after lockout expires
    let context = auth.authenticate("testuser", "password123").await.unwrap();
    assert_eq!(context.username, "testuser");
}

#[tokio::test]
async fn test_password_policy_enforcement() {
    let mut auth = AuthManager::new(AuthConfig {
        password_min_length: 12,
        ..Default::default()
    });

    // Test password too short
    let short_password_request = CreateUserRequest {
        username: "user1".to_string(),
        email: "user1@example.com".to_string(),
        password: "short".to_string(), // Too short
        roles: None,
        metadata: None,
    };

    let result = auth.create_user(short_password_request).await;
    assert!(result.is_err());

    // Test password meets minimum length
    let valid_password_request = CreateUserRequest {
        username: "user2".to_string(),
        email: "user2@example.com".to_string(),
        password: "longpassword123".to_string(), // Meets minimum
        roles: None,
        metadata: None,
    };

    let user = auth.create_user(valid_password_request).await.unwrap();
    assert_eq!(user.username, "user2");
}

#[tokio::test]
async fn test_concurrent_user_creation() {
    let auth_manager = Arc::new(Mutex::new(AuthManager::new(AuthConfig::default())));

    // Spawn multiple concurrent user creation tasks
    let mut handles = Vec::new();
    for i in 0..20 {
        let auth_clone = auth_manager.clone();
        let handle = tokio::spawn(async move {
            let mut auth = auth_clone.lock().await;
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "password123".to_string(),
                roles: None,
                metadata: None,
            };
            auth.create_user(request).await
        });
        handles.push(handle);
    }

    // Wait for all creations to complete
    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }

    // All users should be created successfully
    assert_eq!(success_count, 20);

    // Verify all users exist
    let auth = auth_manager.lock().await;
    let users = auth.get_users();
    assert!(users.len() >= 20);
}

#[tokio::test]
async fn test_role_assignment_and_permissions() {
    let mut auth = AuthManager::new(AuthConfig::default());

    // Create a custom role
    let role_request = CreateRoleRequest {
        name: "custom_role".to_string(),
        description: "Custom role with specific permissions".to_string(),
        permissions: vec![
            Permission {
                resource: "data:custom".to_string(),
                action: "read".to_string(),
                conditions: HashMap::new(),
            },
            Permission {
                resource: "data:custom".to_string(),
                action: "write".to_string(),
                conditions: HashMap::new(),
            },
        ],
    };

    let role = auth.create_role(role_request).await.unwrap();

    // Create user with custom role
    let user_request = CreateUserRequest {
        username: "customuser".to_string(),
        email: "custom@example.com".to_string(),
        password: "password123".to_string(),
        roles: Some(vec![role.id.clone()]),
        metadata: None,
    };

    let _user = auth.create_user(user_request).await.unwrap();
    let context = auth
        .authenticate("customuser", "password123")
        .await
        .unwrap();

    // User should have custom role permissions
    assert!(auth.has_permission(&context, "data:custom", "read"));
    assert!(auth.has_permission(&context, "data:custom", "write"));

    // User should not have permissions for other resources
    assert!(!auth.has_permission(&context, "data:other", "read"));
}

#[tokio::test]
async fn test_user_metadata_handling() {
    let mut auth = AuthManager::new(AuthConfig::default());

    let mut metadata = HashMap::new();
    metadata.insert("department".to_string(), "engineering".to_string());
    metadata.insert("level".to_string(), "senior".to_string());

    let request = CreateUserRequest {
        username: "metauser".to_string(),
        email: "meta@example.com".to_string(),
        password: "password123".to_string(),
        roles: None,
        metadata: Some(metadata.clone()),
    };

    let user = auth.create_user(request).await.unwrap();
    assert_eq!(
        user.metadata.get("department"),
        Some(&"engineering".to_string())
    );
    assert_eq!(user.metadata.get("level"), Some(&"senior".to_string()));

    // Update metadata
    let mut new_metadata = HashMap::new();
    new_metadata.insert("department".to_string(), "product".to_string());
    let update_request = UpdateUserRequest {
        email: None,
        roles: None,
        is_active: None,
        metadata: Some(new_metadata.clone()),
    };

    let updated_user = auth.update_user(&user.id, update_request).await.unwrap();
    assert_eq!(
        updated_user.metadata.get("department"),
        Some(&"product".to_string())
    );
}

#[tokio::test]
async fn test_audit_log_filtering() {
    let mut auth = AuthManager::new(AuthConfig {
        enable_audit_logging: true,
        ..Default::default()
    });

    // Create multiple users
    let mut user_ids = Vec::new();
    for i in 0..5 {
        let request = CreateUserRequest {
            username: format!("user{}", i),
            email: format!("user{}@example.com", i),
            password: "password123".to_string(),
            roles: None,
            metadata: None,
        };
        let user = auth.create_user(request).await.unwrap();
        user_ids.push(user.id.clone());
        let _ = auth
            .authenticate(&format!("user{}", i), "password123")
            .await
            .unwrap();
    }

    // Get audit logs for specific user
    let user1_logs = auth.get_audit_logs(Some(&user_ids[0]), 10);
    assert!(!user1_logs.is_empty());

    // All logs should be for user1
    for log in &user1_logs {
        assert_eq!(log.user_id, user_ids[0]);
    }

    // Get all audit logs
    let all_logs = auth.get_audit_logs(None, 100);
    assert!(all_logs.len() >= 10); // At least 5 CREATE_USER + 5 AUTHENTICATE events
}

#[tokio::test]
async fn test_inactive_user_authentication() {
    let mut auth = AuthManager::new(AuthConfig::default());

    let request = CreateUserRequest {
        username: "inactiveuser".to_string(),
        email: "inactive@example.com".to_string(),
        password: "password123".to_string(),
        roles: None,
        metadata: None,
    };

    let user = auth.create_user(request).await.unwrap();

    // Deactivate user
    let update_request = UpdateUserRequest {
        email: None,
        roles: None,
        is_active: Some(false),
        metadata: None,
    };

    auth.update_user(&user.id, update_request).await.unwrap();

    // Inactive user should not be able to authenticate
    let result = auth.authenticate("inactiveuser", "password123").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));
}
