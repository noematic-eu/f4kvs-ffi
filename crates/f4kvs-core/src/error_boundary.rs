//! Error boundaries for F4KVS Core operations
//!
//! This module provides error boundary patterns for the core engine
//! to isolate failures and provide graceful degradation.

use crate::{F4KvsError, Result, StorageEngine};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Core error boundary configuration
#[derive(Debug, Clone)]
pub struct CoreErrorBoundaryConfig {
    /// Enable automatic retry for recoverable errors
    pub enable_retry: bool,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay for exponential backoff
    pub base_retry_delay: Duration,
    /// Enable graceful degradation for non-critical operations
    pub enable_graceful_degradation: bool,
    /// Timeout for operations
    pub operation_timeout: Duration,
}

impl Default for CoreErrorBoundaryConfig {
    fn default() -> Self {
        Self {
            enable_retry: true,
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            enable_graceful_degradation: true,
            operation_timeout: Duration::from_secs(30),
        }
    }
}

/// Core error boundary for F4KVS operations
pub struct CoreErrorBoundary {
    storage: Arc<dyn StorageEngine + Send + Sync>,
    config: CoreErrorBoundaryConfig,
}

impl CoreErrorBoundary {
    /// Create a new core error boundary
    pub fn new(
        storage: Arc<dyn StorageEngine + Send + Sync>,
        config: CoreErrorBoundaryConfig,
    ) -> Self {
        Self { storage, config }
    }

    /// Execute operation with error boundary protection
    pub async fn execute_with_boundary<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<dyn StorageEngine + Send + Sync>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        if self.config.enable_retry {
            self.execute_with_retry(operation).await
        } else {
            operation(Arc::clone(&self.storage)).await
        }
    }

    /// Execute operation with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<dyn StorageEngine + Send + Sync>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut attempt = 0;
        let mut delay = self.config.base_retry_delay;

        loop {
            match operation(Arc::clone(&self.storage)).await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_recoverable() && attempt < self.config.max_retries => {
                    attempt += 1;
                    warn!(
                        attempt = attempt,
                        error = %e,
                        "Retrying operation after recoverable error"
                    );

                    tokio::time::sleep(delay).await;
                    delay = self.calculate_backoff_delay(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff_delay(&self, current_delay: Duration) -> Duration {
        Duration::from_millis(
            (current_delay.as_millis() * 2).min(5000) as u64, // Cap at 5 seconds
        )
    }

    /// Execute operation with timeout
    pub async fn execute_with_timeout<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<dyn StorageEngine + Send + Sync>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        tokio::time::timeout(
            self.config.operation_timeout,
            self.execute_with_boundary(operation),
        )
        .await
        .map_err(|_| {
            F4KvsError::timeout_with_operation(
                "boundary_execution",
                self.config.operation_timeout.as_millis() as u64,
            )
        })?
    }

    /// Execute operation with graceful degradation
    pub async fn execute_with_graceful_degradation<F, Fut, T>(
        &self,
        operation: F,
        fallback: impl Fn() -> T,
    ) -> Result<T>
    where
        F: Fn(Arc<dyn StorageEngine + Send + Sync>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        if self.config.enable_graceful_degradation {
            match self.execute_with_boundary(operation).await {
                Ok(result) => Ok(result),
                Err(e) => {
                    warn!(
                        error = %e,
                        "Operation failed, using graceful degradation"
                    );
                    Ok(fallback())
                }
            }
        } else {
            self.execute_with_boundary(operation).await
        }
    }
}

/// Error boundary factory for core operations
pub struct CoreErrorBoundaryFactory;

impl CoreErrorBoundaryFactory {
    /// Create a core error boundary with default configuration
    pub fn create_boundary(storage: Arc<dyn StorageEngine + Send + Sync>) -> CoreErrorBoundary {
        CoreErrorBoundary::new(storage, CoreErrorBoundaryConfig::default())
    }

    /// Create a core error boundary with custom configuration
    pub fn create_boundary_with_config(
        storage: Arc<dyn StorageEngine + Send + Sync>,
        config: CoreErrorBoundaryConfig,
    ) -> CoreErrorBoundary {
        CoreErrorBoundary::new(storage, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_storage::MemoryStorage;

    #[tokio::test]
    async fn test_core_error_boundary_success() {
        let storage = Arc::new(MemoryStorage::with_mode(
            crate::config::StorageMode::HashMap,
        ));
        let boundary = CoreErrorBoundaryFactory::create_boundary(storage.clone());

        let storage_clone = Arc::clone(&storage);
        let result = boundary
            .execute_with_boundary(|_| {
                let storage_clone = Arc::clone(&storage_clone);
                async move {
                    storage_clone
                        .put("key", &crate::Value::String("value".to_string()))
                        .await
                }
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_core_error_boundary_graceful_degradation() {
        let storage = Arc::new(MemoryStorage::with_mode(
            crate::config::StorageMode::HashMap,
        ));
        let boundary = CoreErrorBoundaryFactory::create_boundary(storage);

        let result = boundary
            .execute_with_graceful_degradation(
                |_| async { Err::<String, F4KvsError>(F4KvsError::internal("Test error")) },
                || "fallback_value".to_string(),
            )
            .await;

        assert!(result.is_ok());
        // Note: The graceful degradation test would need a more sophisticated setup
        // to properly test the fallback mechanism
    }
}
