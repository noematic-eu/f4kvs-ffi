//! Monitoring hooks for F4KVS Core
//!
//! This module provides hooks for external monitoring systems to integrate
//! with F4KVS Core. It allows monitoring systems to register callbacks for
//! various events and metrics.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Types of events that can be monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonitoringEvent {
    /// Operation started
    OperationStart,
    /// Operation completed
    OperationComplete,
    /// Operation failed
    OperationFailed,
    /// Memory usage changed
    MemoryUsageChanged,
    /// Health check performed
    HealthCheck,
    /// Shutdown initiated
    ShutdownInitiated,
    /// Shutdown completed
    ShutdownCompleted,
}

/// Monitoring hook callback function type
pub type MonitoringHook = Arc<dyn Fn(MonitoringEvent, &MonitoringContext) + Send + Sync>;

/// Context information passed to monitoring hooks
#[derive(Debug, Clone)]
pub struct MonitoringContext {
    /// Event-specific data (JSON-like structure)
    pub data: std::collections::HashMap<String, String>,
    /// Timestamp of the event
    pub timestamp: std::time::Instant,
}

impl MonitoringContext {
    /// Create a new monitoring context
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
            timestamp: std::time::Instant::now(),
        }
    }

    /// Add a key-value pair to the context
    pub fn with_data(mut self, key: String, value: String) -> Self {
        self.data.insert(key, value);
        self
    }
}

impl Default for MonitoringContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitoring hooks manager
pub struct MonitoringHooks {
    hooks: Arc<RwLock<Vec<MonitoringHook>>>,
    disabled: bool,
}

impl MonitoringHooks {
    /// Create a new monitoring hooks manager
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
            disabled: false,
        }
    }

    /// Create a disabled monitoring hooks manager (no-op)
    pub fn disabled() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
            disabled: true,
        }
    }

    /// Register a monitoring hook
    pub async fn register_hook(&self, hook: MonitoringHook) {
        let mut hooks = self.hooks.write().await;
        hooks.push(hook);
        debug!("Registered monitoring hook (total: {})", hooks.len());
    }

    /// Unregister all hooks
    pub async fn clear_hooks(&self) {
        let mut hooks = self.hooks.write().await;
        hooks.clear();
        debug!("Cleared all monitoring hooks");
    }

    /// Trigger a monitoring event
    pub async fn trigger(&self, event: MonitoringEvent, context: MonitoringContext) {
        if self.disabled {
            trace!(event = ?event, "Monitoring is disabled; event dropped");
            return; // No-op when monitoring is disabled
        }

        let hooks = self.hooks.read().await;
        if hooks.is_empty() {
            return;
        }

        debug!(
            "Triggering monitoring event: {:?} ({} hooks)",
            event,
            hooks.len()
        );
        for hook in hooks.iter() {
            hook(event, &context);
        }
    }

    /// Get the number of registered hooks
    pub async fn hook_count(&self) -> usize {
        self.hooks.read().await.len()
    }
}

impl Default for MonitoringHooks {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for monitoring context
pub struct MonitoringContextBuilder {
    context: MonitoringContext,
}

impl MonitoringContextBuilder {
    /// Create a new context builder
    pub fn new() -> Self {
        Self {
            context: MonitoringContext::new(),
        }
    }

    /// Add a key-value pair
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.data.insert(key.into(), value.into());
        self
    }

    /// Build the context
    pub fn build(self) -> MonitoringContext {
        self.context
    }
}

impl Default for MonitoringContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_monitoring_hooks() {
        let hooks = MonitoringHooks::new();
        let call_count = Arc::new(AtomicU32::new(0));

        let call_count_clone = Arc::clone(&call_count);
        let hook: MonitoringHook = Arc::new(move |event, _context| {
            assert_eq!(event, MonitoringEvent::OperationStart);
            call_count_clone.fetch_add(1, Ordering::Relaxed);
        });

        hooks.register_hook(hook).await;
        assert_eq!(hooks.hook_count().await, 1);

        let context = MonitoringContext::new();
        hooks
            .trigger(MonitoringEvent::OperationStart, context)
            .await;

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_monitoring_context_builder() {
        let context = MonitoringContextBuilder::new()
            .with("key1", "value1")
            .with("key2", "value2")
            .build();

        assert_eq!(context.data.get("key1"), Some(&"value1".to_string()));
        assert_eq!(context.data.get("key2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_hooks() {
        let hooks = MonitoringHooks::new();
        let call_count = Arc::new(AtomicU32::new(0));

        // Register multiple hooks
        for _ in 0..5 {
            let call_count_clone = Arc::clone(&call_count);
            let hook: MonitoringHook = Arc::new(move |_event, _context| {
                call_count_clone.fetch_add(1, Ordering::Relaxed);
            });
            hooks.register_hook(hook).await;
        }

        assert_eq!(hooks.hook_count().await, 5);

        let context = MonitoringContext::new();
        hooks
            .trigger(MonitoringEvent::OperationComplete, context)
            .await;

        assert_eq!(call_count.load(Ordering::Relaxed), 5);
    }
}
