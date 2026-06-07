//! Audit logging for F4KVS
//!
//! This module provides comprehensive audit logging capabilities for tracking
//! security events, data access, and system operations.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "std")]
use chrono::{DateTime, Utc};

#[cfg(feature = "std")]
use uuid::Uuid;

/// Audit event types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Authentication event
    Authentication {
        /// User ID
        user_id: String,
        /// Success or failure
        success: bool,
        /// Failure reason if applicable
        reason: Option<String>,
    },
    /// Authorization event
    Authorization {
        /// User ID
        user_id: String,
        /// Resource accessed
        resource: String,
        /// Action performed
        action: String,
        /// Allowed or denied
        allowed: bool,
    },
    /// Data access event
    DataAccess {
        /// User ID
        user_id: String,
        /// Key accessed
        key: String,
        /// Operation type
        operation: String,
        /// Success or failure
        success: bool,
    },
    /// Encryption event
    Encryption {
        /// Key ID used
        key_id: String,
        /// Operation type
        operation: String,
        /// Success or failure
        success: bool,
    },
    /// Configuration change event
    ConfigurationChange {
        /// User ID making the change
        user_id: String,
        /// Configuration key changed
        config_key: String,
        /// Old value
        old_value: Option<String>,
        /// New value
        new_value: Option<String>,
    },
    /// Security event
    Security {
        /// Event description
        description: String,
        /// Severity level
        severity: SecuritySeverity,
    },
}

/// Security severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SecuritySeverity {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry ID
    pub id: String,
    /// Event type
    pub event_type: AuditEventType,
    /// Timestamp
    #[cfg(feature = "std")]
    pub timestamp: DateTime<Utc>,
    #[cfg(not(feature = "std"))]
    pub timestamp: u64, // Unix timestamp as fallback
    /// Source IP address (if available)
    pub source_ip: Option<String>,
    /// User agent (if available)
    pub user_agent: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Audit logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Maximum number of entries to keep in memory
    pub max_entries: usize,
    /// Log file path (if file logging is enabled)
    pub log_file: Option<String>,
    /// Enable file logging
    pub enable_file_logging: bool,
    /// Log retention period in days
    pub retention_days: u32,
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 10000,
            log_file: None,
            enable_file_logging: false,
            retention_days: 90,
        }
    }
}

/// Audit logger
pub struct AuditLogger {
    config: AuditLogConfig,
    entries: Arc<RwLock<VecDeque<AuditLogEntry>>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(config: AuditLogConfig) -> Self {
        Self {
            config,
            entries: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Log an audit event
    pub async fn log(&self, event_type: AuditEventType) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        let entry = AuditLogEntry {
            #[cfg(feature = "std")]
            id: Uuid::new_v4().to_string(),
            #[cfg(not(feature = "std"))]
            id: format!(
                "entry_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            event_type,
            #[cfg(feature = "std")]
            timestamp: Utc::now(),
            #[cfg(not(feature = "std"))]
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            source_ip: None,
            user_agent: None,
            metadata: std::collections::HashMap::new(),
        };

        let mut entries = self.entries.write().await;
        entries.push_back(entry);

        // Trim if exceeds max entries
        while entries.len() > self.config.max_entries {
            entries.pop_front();
        }

        Ok(())
    }

    /// Log an audit event with metadata
    pub async fn log_with_metadata(
        &self,
        event_type: AuditEventType,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        let entry = AuditLogEntry {
            #[cfg(feature = "std")]
            id: Uuid::new_v4().to_string(),
            #[cfg(not(feature = "std"))]
            id: format!(
                "entry_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            event_type,
            #[cfg(feature = "std")]
            timestamp: Utc::now(),
            #[cfg(not(feature = "std"))]
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            source_ip: None,
            user_agent: None,
            metadata,
        };

        let mut entries = self.entries.write().await;
        entries.push_back(entry);

        // Trim if exceeds max entries
        while entries.len() > self.config.max_entries {
            entries.pop_front();
        }

        Ok(())
    }

    /// Get audit log entries
    pub async fn get_entries(&self, limit: Option<usize>) -> Vec<AuditLogEntry> {
        let entries = self.entries.read().await;
        let limit = limit.unwrap_or(entries.len());
        entries.iter().rev().take(limit).cloned().collect()
    }

    /// Get audit log entries filtered by event type
    pub async fn get_entries_by_type(
        &self,
        event_type: &AuditEventType,
        limit: Option<usize>,
    ) -> Vec<AuditLogEntry> {
        let entries = self.entries.read().await;
        let limit = limit.unwrap_or(entries.len());
        entries
            .iter()
            .rev()
            .filter(|entry| {
                matches!(
                    (&entry.event_type, event_type),
                    (
                        AuditEventType::Authentication { .. },
                        AuditEventType::Authentication { .. }
                    ) | (
                        AuditEventType::Authorization { .. },
                        AuditEventType::Authorization { .. }
                    ) | (
                        AuditEventType::DataAccess { .. },
                        AuditEventType::DataAccess { .. }
                    ) | (
                        AuditEventType::Encryption { .. },
                        AuditEventType::Encryption { .. }
                    ) | (
                        AuditEventType::ConfigurationChange { .. },
                        AuditEventType::ConfigurationChange { .. }
                    ) | (
                        AuditEventType::Security { .. },
                        AuditEventType::Security { .. }
                    )
                )
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Clear old audit log entries
    pub async fn clear_old_entries(&self) -> Result<usize, String> {
        #[cfg(feature = "std")]
        {
            use chrono::Duration;
            let cutoff = Utc::now() - Duration::days(self.config.retention_days as i64);
            let mut entries = self.entries.write().await;
            let initial_len = entries.len();

            entries.retain(|entry| entry.timestamp >= cutoff);

            Ok(initial_len - entries.len())
        }

        #[cfg(not(feature = "std"))]
        {
            let cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - (self.config.retention_days as u64 * 86400);
            let mut entries = self.entries.write().await;
            let initial_len = entries.len();

            entries.retain(|entry| entry.timestamp >= cutoff);

            Ok(initial_len - entries.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_logging() {
        let config = AuditLogConfig::default();
        let logger = AuditLogger::new(config);

        logger
            .log(AuditEventType::Authentication {
                user_id: "user1".to_string(),
                success: true,
                reason: None,
            })
            .await
            .unwrap();

        let entries = logger.get_entries(None).await;
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].event_type,
            AuditEventType::Authentication { .. }
        ));
    }

    #[tokio::test]
    async fn test_audit_logging_disabled() {
        let mut config = AuditLogConfig::default();
        config.enabled = false;
        let logger = AuditLogger::new(config);

        logger
            .log(AuditEventType::Authentication {
                user_id: "user1".to_string(),
                success: true,
                reason: None,
            })
            .await
            .unwrap();

        let entries = logger.get_entries(None).await;
        assert_eq!(entries.len(), 0);
    }
}
