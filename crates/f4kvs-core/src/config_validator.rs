//! Configuration Validation for F4KVS Core
//!
//! This module provides comprehensive configuration validation with detailed error messages,
//! validation rules, and suggestions for fixing configuration issues.

use crate::{F4KvsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Type alias for validation function
pub type ValidationFunction = dyn Fn(&str, &str) -> Option<ValidationError> + Send + Sync;

/// Type alias for warning function
pub type WarningFunction = dyn Fn(&str, &str) -> Option<ValidationWarning> + Send + Sync;

/// Configuration validation result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the configuration is valid
    pub is_valid: bool,
    /// List of validation errors
    pub errors: Vec<ValidationError>,
    /// List of validation warnings
    pub warnings: Vec<ValidationWarning>,
    /// Suggestions for fixing issues
    pub suggestions: Vec<String>,
}

/// Configuration validation error
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Field path (e.g., "core.max_key_size")
    pub field: String,
    /// Error message
    pub message: String,
    /// Current value
    pub current_value: String,
    /// Expected value or range
    pub expected: String,
    /// Severity level
    pub severity: ValidationSeverity,
    /// Specific fix suggestion for this error
    pub fix_suggestion: Option<String>,
}

/// Configuration validation warning
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Field path
    pub field: String,
    /// Warning message
    pub message: String,
    /// Current value
    pub current_value: String,
    /// Suggestion
    pub suggestion: String,
}

/// Validation severity levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Critical error that prevents operation
    Critical,
    /// Error that may cause issues
    Error,
    /// Warning about potential problems
    Warning,
    /// Information about configuration
    Info,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (current: {}, expected: {})",
            self.field, self.message, self.current_value, self.expected
        )
    }
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (current: {}, suggestion: {})",
            self.field, self.message, self.current_value, self.suggestion
        )
    }
}

/// Configuration validation rules
pub struct ValidationRule {
    /// Field path pattern
    pub field_pattern: String,
    /// Validation function
    pub validator: Box<ValidationFunction>,
    /// Warning function (optional)
    pub warning: Option<Box<WarningFunction>>,
}

/// Configuration validator
pub struct ConfigValidator {
    /// Validation rules
    rules: Vec<ValidationRule>,
    /// Custom validators
    custom_validators: HashMap<String, Box<ValidationFunction>>,
    /// Enabled features (for feature flag validation)
    enabled_features: Vec<String>,
}

impl ConfigValidator {
    /// Create a new configuration validator
    pub fn new() -> Self {
        let mut validator = Self {
            rules: Vec::new(),
            custom_validators: HashMap::new(),
            enabled_features: Vec::new(),
        };
        validator.add_default_rules();
        validator
    }

    /// Create a new configuration validator with enabled features
    pub fn with_features(features: Vec<String>) -> Self {
        let mut validator = Self {
            rules: Vec::new(),
            custom_validators: HashMap::new(),
            enabled_features: features,
        };
        validator.add_default_rules();
        validator
    }

    /// Add a custom validation rule
    pub fn add_rule(mut self, rule: ValidationRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add a custom validator for a specific field
    pub fn add_custom_validator<F>(mut self, field: String, validator: F) -> Self
    where
        F: Fn(&str, &str) -> Option<ValidationError> + Send + Sync + 'static,
    {
        self.custom_validators.insert(field, Box::new(validator));
        self
    }

    /// Validate configuration values
    pub fn validate(&self, config_values: &HashMap<String, String>) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // Apply custom validators first
        for (field, value) in config_values {
            if let Some(validator) = self.custom_validators.get(field) {
                if let Some(mut error) = validator(field, value) {
                    // Ensure fix_suggestion is set
                    if error.fix_suggestion.is_none() {
                        error.fix_suggestion = self.generate_fix_suggestion(&error.field);
                    }
                    errors.push(error);
                }
            }
        }

        // Apply general rules
        for rule in &self.rules {
            for (field, value) in config_values {
                if self.field_matches_pattern(field, &rule.field_pattern) {
                    if let Some(mut error) = (rule.validator)(field, value) {
                        // Ensure fix_suggestion is set
                        if error.fix_suggestion.is_none() {
                            error.fix_suggestion = self.generate_fix_suggestion(&error.field);
                        }
                        errors.push(error);
                    }

                    if let Some(ref warning_fn) = rule.warning {
                        if let Some(warning) = warning_fn(field, value) {
                            warnings.push(warning);
                        }
                    }
                }
            }
        }

        // Cross-field validation
        errors.extend(self.validate_cross_fields(config_values));

        // Feature flag validation
        errors.extend(self.validate_feature_flags(config_values));

        // Conflict detection
        errors.extend(self.detect_conflicts(config_values));
        warnings.extend(self.detect_conflict_warnings(config_values));

        // Generate suggestions based on errors
        suggestions.extend(self.generate_suggestions(&errors, &warnings));

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            suggestions,
        }
    }

    /// Add default validation rules
    fn add_default_rules(&mut self) {
        // Core configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "core.max_key_size".to_string(),
            validator: Box::new(|field, value| match value.parse::<usize>() {
                Ok(0) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Maximum key size must be greater than 0".to_string(),
                    current_value: value.to_string(),
                    expected: "> 0".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set max_key_size to 1024 (1KB) for most use cases".to_string(),
                    ),
                }),
                Ok(size) if size > 1024 * 1024 => Some(ValidationError {
                    field: field.to_string(),
                    message: "Maximum key size is very large, consider reducing".to_string(),
                    current_value: value.to_string(),
                    expected: "<= 1MB".to_string(),
                    severity: ValidationSeverity::Warning,
                    fix_suggestion: Some(
                        "Consider reducing max_key_size to 1024 (1KB) or less".to_string(),
                    ),
                }),
                Ok(_) => None,
                Err(_) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid number format".to_string(),
                    current_value: value.to_string(),
                    expected: "positive integer".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set max_key_size to a positive integer (e.g., 1024)".to_string(),
                    ),
                }),
            }),
            warning: Some(Box::new(|field, value| match value.parse::<usize>() {
                Ok(size) if size < 1024 => Some(ValidationWarning {
                    field: field.to_string(),
                    message: "Small key size limit may restrict functionality".to_string(),
                    current_value: value.to_string(),
                    suggestion: "Consider increasing to at least 1KB".to_string(),
                }),
                _ => None,
            })),
        });

        self.rules.push(ValidationRule {
            field_pattern: "core.max_value_size".to_string(),
            validator: Box::new(|field, value| match value.parse::<usize>() {
                Ok(0) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Maximum value size must be greater than 0".to_string(),
                    current_value: value.to_string(),
                    expected: "> 0".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set max_value_size to 10485760 (10MB) for balanced performance"
                            .to_string(),
                    ),
                }),
                Ok(size) if size > 100 * 1024 * 1024 => Some(ValidationError {
                    field: field.to_string(),
                    message: "Very large value size may cause memory issues".to_string(),
                    current_value: value.to_string(),
                    expected: "<= 100MB".to_string(),
                    severity: ValidationSeverity::Warning,
                    fix_suggestion: Some(
                        "Consider reducing max_value_size to 10485760 (10MB) or less".to_string(),
                    ),
                }),
                Ok(_) => None,
                Err(_) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid number format".to_string(),
                    current_value: value.to_string(),
                    expected: "positive integer".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set max_value_size to a positive integer (e.g., 10485760)".to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        self.rules.push(ValidationRule {
            field_pattern: "core.operation_timeout".to_string(),
            validator: Box::new(|field, value| match value.parse::<u64>() {
                Ok(0) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Operation timeout must be greater than 0".to_string(),
                    current_value: value.to_string(),
                    expected: "> 0 seconds".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set operation_timeout to 30 seconds for most use cases".to_string(),
                    ),
                }),
                Ok(timeout) if timeout > 3600 => Some(ValidationError {
                    field: field.to_string(),
                    message: "Very long timeout may cause performance issues".to_string(),
                    current_value: value.to_string(),
                    expected: "<= 3600 seconds".to_string(),
                    severity: ValidationSeverity::Warning,
                    fix_suggestion: Some("Consider reducing timeout to 30-300 seconds".to_string()),
                }),
                Ok(_) => None,
                Err(_) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid number format".to_string(),
                    current_value: value.to_string(),
                    expected: "positive integer".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set operation_timeout to a positive integer (e.g., 30)".to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        self.rules.push(ValidationRule {
            field_pattern: "core.storage_mode".to_string(),
            validator: Box::new(|field, value| match value.to_lowercase().as_str() {
                "hashmap" | "btreemap" => None,
                _ => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid storage mode".to_string(),
                    current_value: value.to_string(),
                    expected: "hashmap or btreemap".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set storage_mode to 'hashmap' (faster) or 'btreemap' (ordered)"
                            .to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        // Environment configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "environment.name".to_string(),
            validator: Box::new(|field, value| match value {
                "development" | "staging" | "production" => None,
                _ => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid environment name".to_string(),
                    current_value: value.to_string(),
                    expected: "development, staging, or production".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set environment.name to 'development', 'staging', or 'production'"
                            .to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        // Security configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "security.jwt_secret".to_string(),
            validator: Box::new(|field, value| {
                if value.is_empty() {
                    Some(ValidationError {
                        field: field.to_string(),
                        message: "JWT secret cannot be empty".to_string(),
                        current_value: "".to_string(),
                        expected: "non-empty string".to_string(),
                        severity: ValidationSeverity::Critical,
                        fix_suggestion: Some(
                            "Generate a strong JWT secret: openssl rand -base64 32".to_string(),
                        ),
                    })
                } else if value.len() < 32 {
                    Some(ValidationError {
                        field: field.to_string(),
                        message: "JWT secret is too short for security".to_string(),
                        current_value: value.to_string(),
                        expected: "at least 32 characters".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some(
                            "Generate a longer JWT secret: openssl rand -base64 32".to_string(),
                        ),
                    })
                } else if value == "f4kvs-default-secret-change-in-production" {
                    Some(ValidationError {
                        field: field.to_string(),
                        message: "Using default JWT secret in production is dangerous".to_string(),
                        current_value: value.to_string(),
                        expected: "strong, unique secret".to_string(),
                        severity: ValidationSeverity::Critical,
                        fix_suggestion: Some(
                            "Generate a unique JWT secret: openssl rand -base64 32".to_string(),
                        ),
                    })
                } else {
                    None
                }
            }),
            warning: None,
        });

        // Logging configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "logging.level".to_string(),
            validator: Box::new(|field, value| match value.to_lowercase().as_str() {
                "trace" | "debug" | "info" | "warn" | "error" => None,
                _ => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid log level".to_string(),
                    current_value: value.to_string(),
                    expected: "trace, debug, info, warn, or error".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set logging.level to one of: trace, debug, info, warn, error".to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        self.rules.push(ValidationRule {
            field_pattern: "logging.format".to_string(),
            validator: Box::new(|field, value| match value.to_lowercase().as_str() {
                "json" | "text" => None,
                _ => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid log format".to_string(),
                    current_value: value.to_string(),
                    expected: "json or text".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some("Set logging.format to 'json' or 'text'".to_string()),
                }),
            }),
            warning: None,
        });

        // Performance configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "performance.memory_pool.block_size".to_string(),
            validator: Box::new(|field, value| match value.parse::<usize>() {
                Ok(0) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Block size must be greater than 0".to_string(),
                    current_value: value.to_string(),
                    expected: "> 0".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set block_size to 4096 for better performance".to_string(),
                    ),
                }),
                Ok(size) if size < 1024 => Some(ValidationError {
                    field: field.to_string(),
                    message: "Small block size may cause fragmentation".to_string(),
                    current_value: value.to_string(),
                    expected: ">= 1024 bytes".to_string(),
                    severity: ValidationSeverity::Warning,
                    fix_suggestion: Some(
                        "Consider increasing block_size to 4096 for better performance".to_string(),
                    ),
                }),
                Ok(_) => None,
                Err(_) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid number format".to_string(),
                    current_value: value.to_string(),
                    expected: "positive integer".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set block_size to a positive integer (e.g., 4096)".to_string(),
                    ),
                }),
            }),
            warning: None,
        });

        // Monitoring configuration rules
        self.rules.push(ValidationRule {
            field_pattern: "monitoring.prometheus_port".to_string(),
            validator: Box::new(|field, value| match value.parse::<u16>() {
                Ok(0) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Prometheus port must be greater than 0".to_string(),
                    current_value: value.to_string(),
                    expected: "1-65535".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set prometheus_port to 9090 (standard Prometheus port)".to_string(),
                    ),
                }),
                Ok(_) => None,
                Err(_) => Some(ValidationError {
                    field: field.to_string(),
                    message: "Invalid port number".to_string(),
                    current_value: value.to_string(),
                    expected: "1-65535".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Set prometheus_port to a valid port number (1-65535), e.g., 9090"
                            .to_string(),
                    ),
                }),
            }),
            warning: Some(Box::new(|field, value| match value.parse::<u16>() {
                Ok(port) if port < 1024 => Some(ValidationWarning {
                    field: field.to_string(),
                    message: "Using privileged port may require root access".to_string(),
                    current_value: value.to_string(),
                    suggestion: "Consider using port >= 1024".to_string(),
                }),
                _ => None,
            })),
        });
    }

    /// Check if a field matches a pattern
    fn field_matches_pattern(&self, field: &str, pattern: &str) -> bool {
        if let Some(stripped) = pattern.strip_suffix('*') {
            field.starts_with(stripped)
        } else {
            field == pattern
        }
    }

    /// Validate cross-field dependencies
    fn validate_cross_fields(
        &self,
        config_values: &HashMap<String, String>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // If encryption is enabled, encryption_key must be set
        if config_values
            .get("security.enable_encryption")
            .map(|v| v == "true")
            .unwrap_or(false)
            && config_values
                .get("security.encryption_key")
                .map(|v| v.is_empty())
                .unwrap_or(true)
        {
            errors.push(ValidationError {
                field: "security.encryption_key".to_string(),
                message: "Encryption key is required when encryption is enabled".to_string(),
                current_value: "not set".to_string(),
                expected: "non-empty encryption key".to_string(),
                severity: ValidationSeverity::Critical,
                fix_suggestion: Some("Set security.encryption_key to a secure key, or disable security.enable_encryption".to_string()),
            });
        }

        // If auth is enabled, jwt_secret must be set and strong
        if config_values
            .get("security.enable_auth")
            .map(|v| v == "true")
            .unwrap_or(false)
        {
            if let Some(jwt_secret) = config_values.get("security.jwt_secret") {
                if jwt_secret.is_empty() {
                    errors.push(ValidationError {
                        field: "security.jwt_secret".to_string(),
                        message: "JWT secret is required when authentication is enabled"
                            .to_string(),
                        current_value: "empty".to_string(),
                        expected: "non-empty JWT secret (at least 32 characters)".to_string(),
                        severity: ValidationSeverity::Critical,
                        fix_suggestion: Some(
                            "Generate a strong JWT secret: openssl rand -base64 32".to_string(),
                        ),
                    });
                }
            } else {
                errors.push(ValidationError {
                    field: "security.jwt_secret".to_string(),
                    message: "JWT secret is required when authentication is enabled".to_string(),
                    current_value: "not set".to_string(),
                    expected: "non-empty JWT secret (at least 32 characters)".to_string(),
                    severity: ValidationSeverity::Critical,
                    fix_suggestion: Some(
                        "Generate a strong JWT secret: openssl rand -base64 32".to_string(),
                    ),
                });
            }
        }

        // Validate memory pool configuration consistency
        if let (Some(max_pool_size), Some(initial_blocks)) = (
            config_values
                .get("performance.memory_pool.max_pool_size")
                .and_then(|v| v.parse::<usize>().ok()),
            config_values
                .get("performance.memory_pool.initial_blocks")
                .and_then(|v| v.parse::<usize>().ok()),
        ) {
            if initial_blocks > max_pool_size {
                errors.push(ValidationError {
                    field: "performance.memory_pool.initial_blocks".to_string(),
                    message: "Initial blocks cannot exceed max pool size".to_string(),
                    current_value: initial_blocks.to_string(),
                    expected: format!("<= {}", max_pool_size),
                    severity: ValidationSeverity::Error,
                    fix_suggestion: Some(format!(
                        "Reduce initial_blocks to {} or less, or increase max_pool_size",
                        max_pool_size
                    )),
                });
            }
        }

        errors
    }

    /// Validate feature flag requirements
    fn validate_feature_flags(
        &self,
        config_values: &HashMap<String, String>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check if encryption feature is enabled when encryption config is set
        if config_values
            .get("security.enable_encryption")
            .map(|v| v == "true")
            .unwrap_or(false)
            && !self
                .enabled_features
                .iter()
                .any(|f| f == "encryption" || f == "security")
        {
            errors.push(ValidationError {
                field: "security.enable_encryption".to_string(),
                message: "Encryption feature is not enabled".to_string(),
                current_value: "true".to_string(),
                expected: "encryption feature enabled".to_string(),
                severity: ValidationSeverity::Error,
                fix_suggestion: Some(
                    "Enable the 'encryption' feature: cargo build --features encryption"
                        .to_string(),
                ),
            });
        }

        // Check if compression features are enabled when compression config is set
        if config_values.contains_key("compression.enabled") {
            let compression_enabled = config_values
                .get("compression.enabled")
                .map(|v| v == "true")
                .unwrap_or(false);

            if compression_enabled
                && !self.enabled_features.iter().any(|f| {
                    f == "compression" || f == "lz4" || f == "zstd" || f == "gzip" || f == "snappy"
                })
            {
                errors.push(ValidationError {
                    field: "compression.enabled".to_string(),
                    message: "Compression feature is not enabled".to_string(),
                    current_value: "true".to_string(),
                    expected: "compression feature enabled".to_string(),
                    severity: ValidationSeverity::Warning,
                    fix_suggestion: Some(
                        "Enable compression features: cargo build --features compression"
                            .to_string(),
                    ),
                });
            }
        }

        errors
    }

    /// Detect configuration conflicts
    fn detect_conflicts(&self, config_values: &HashMap<String, String>) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Storage mode validation is already handled in individual field validation

        // Check for debug mode in production
        if config_values
            .get("environment.name")
            .map(|v| v == "production")
            .unwrap_or(false)
            && config_values
                .get("environment.debug")
                .map(|v| v == "true")
                .unwrap_or(false)
        {
            errors.push(ValidationError {
                field: "environment.debug".to_string(),
                message: "Debug mode should not be enabled in production".to_string(),
                current_value: "true".to_string(),
                expected: "false".to_string(),
                severity: ValidationSeverity::Error,
                fix_suggestion: Some(
                    "Set environment.debug to false in production environments".to_string(),
                ),
            });
        }

        errors
    }

    /// Detect configuration conflicts that should be warnings
    fn detect_conflict_warnings(
        &self,
        config_values: &HashMap<String, String>,
    ) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Warn if cache size is very large compared to memory pool
        if let (Some(cache_size), Some(memory_pool_size)) = (
            config_values
                .get("performance.cache.max_size")
                .and_then(|v| v.parse::<usize>().ok()),
            config_values
                .get("performance.memory_pool.max_pool_size")
                .and_then(|v| v.parse::<usize>().ok()),
        ) {
            if cache_size > memory_pool_size * 2 {
                warnings.push(ValidationWarning {
                    field: "performance.cache.max_size".to_string(),
                    message: "Cache size is very large compared to memory pool size".to_string(),
                    current_value: cache_size.to_string(),
                    suggestion: format!(
                        "Consider reducing cache size or increasing memory pool size (current: {})",
                        memory_pool_size
                    ),
                });
            }
        }

        warnings
    }

    /// Generate fix suggestion for a field
    fn generate_fix_suggestion(&self, field: &str) -> Option<String> {
        match field {
            "core.max_key_size" => {
                Some("Set max_key_size to 1024 (1KB) for most use cases".to_string())
            }
            "core.max_value_size" => {
                Some("Set max_value_size to 10485760 (10MB) for balanced performance".to_string())
            }
            "security.jwt_secret" => {
                Some("Generate a strong JWT secret: openssl rand -base64 32".to_string())
            }
            "environment.name" => Some(
                "Set environment.name to 'development', 'staging', or 'production'".to_string(),
            ),
            "performance.memory_pool.block_size" => {
                Some("Consider setting block_size to 4096 for better performance".to_string())
            }
            _ => None,
        }
    }

    /// Generate suggestions based on errors and warnings
    fn generate_suggestions(
        &self,
        errors: &[ValidationError],
        warnings: &[ValidationWarning],
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Use fix_suggestion from errors if available
        for error in errors {
            if let Some(ref suggestion) = error.fix_suggestion {
                if !suggestions.contains(suggestion) {
                    suggestions.push(suggestion.clone());
                }
            } else {
                // Fallback to field-based suggestions
                match error.field.as_str() {
                    "core.max_key_size" => {
                        suggestions.push(
                            "Consider setting max_key_size to 1024 (1KB) for most use cases"
                                .to_string(),
                        );
                    }
                    "core.max_value_size" => {
                        suggestions.push("Consider setting max_value_size to 10485760 (10MB) for balanced performance".to_string());
                    }
                    "security.jwt_secret" => {
                        suggestions.push(
                            "Generate a strong JWT secret: openssl rand -base64 32".to_string(),
                        );
                    }
                    "environment.name" => {
                        suggestions.push(
                            "Set environment.name to 'development', 'staging', or 'production'"
                                .to_string(),
                        );
                    }
                    _ => {}
                }
            }
        }

        for warning in warnings {
            match warning.field.as_str() {
                "performance.memory_pool.block_size" => {
                    suggestions.push(
                        "Consider increasing block_size to 4096 for better performance".to_string(),
                    );
                }
                "monitoring.prometheus_port" => {
                    suggestions.push(
                        "Consider using port 9090 for Prometheus (standard port)".to_string(),
                    );
                }
                _ => {}
            }
        }

        suggestions
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration validation utilities
pub struct ConfigValidationUtils;

impl ConfigValidationUtils {
    /// Validate a configuration file
    pub async fn validate_file(path: &str) -> Result<ValidationResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| F4KvsError::io_with_path(path, e.to_string()))?;

        let config_values = if path.ends_with(".toml") {
            toml::from_str::<HashMap<String, String>>(&content)
                .map_err(|e| F4KvsError::config_with_field("toml_parse", e.to_string()))?
        } else {
            serde_json::from_str::<HashMap<String, String>>(&content)
                .map_err(|e| F4KvsError::config_with_field("json_parse", e.to_string()))?
        };

        let validator = ConfigValidator::new();
        Ok(validator.validate(&config_values))
    }

    /// Generate a configuration template
    pub fn generate_template() -> String {
        r#"# F4KVS Configuration Template
# Copy this file and modify as needed

[core]
max_key_size = 1024
max_value_size = 10485760
operation_timeout = 30
strict_key_validation = true
storage_mode = "BTreeMap"

[environment]
name = "development"
debug = true
config_file = "f4kvs.toml"
hot_reload = true
reload_interval = 5

[performance]
enable_simd = true

[performance.memory_pool]
block_size = 4096
max_pool_size = 5000
initial_blocks = 50
enabled = true

[performance.cache]
max_size = 10000
ttl_seconds = 3600
enabled = true
eviction_policy = "LRU"

[performance.batch]
max_batch_size = 1000
timeout_ms = 100
enabled = true

[security]
enable_auth = false
jwt_secret = "f4kvs-default-secret-change-in-production"
jwt_expiry_seconds = 3600
enable_encryption = false
encryption_key = ""
enable_audit_logging = false

[logging]
level = "info"
format = "text"
structured = false
file_path = ""
console = true

[monitoring]
enable_prometheus = false
prometheus_port = 9090
enable_health_checks = true
health_check_interval = 30
enable_profiling = false
"#
        .to_string()
    }

    /// Check configuration health
    pub fn check_health(config_values: &HashMap<String, String>) -> ValidationResult {
        let validator = ConfigValidator::new();
        validator.validate(config_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_validation_result() {
        let result = ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        };
        assert!(result.is_valid);
    }

    #[test]
    fn test_config_validator() {
        let validator = ConfigValidator::new();
        let mut config = HashMap::new();
        config.insert("core.max_key_size".to_string(), "1024".to_string());
        config.insert("core.max_value_size".to_string(), "10485760".to_string());

        let result = validator.validate(&config);
        assert!(result.is_valid);
    }

    #[test]
    fn test_invalid_config() {
        let validator = ConfigValidator::new();
        let mut config = HashMap::new();
        config.insert("core.max_key_size".to_string(), "0".to_string());
        config.insert("core.max_value_size".to_string(), "invalid".to_string());

        let result = validator.validate(&config);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validation_error_display() {
        let error = ValidationError {
            field: "core.max_key_size".to_string(),
            message: "Must be greater than 0".to_string(),
            current_value: "0".to_string(),
            expected: "> 0".to_string(),
            severity: ValidationSeverity::Critical,
            fix_suggestion: Some("Set max_key_size to 1024 (1KB) for most use cases".to_string()),
        };

        let display = format!("{}", error);
        assert!(display.contains("core.max_key_size"));
        assert!(display.contains("Must be greater than 0"));
    }

    #[test]
    fn test_template_generation() {
        let template = ConfigValidationUtils::generate_template();
        assert!(template.contains("[core]"));
        assert!(template.contains("max_key_size = 1024"));
        assert!(template.contains("[environment]"));
        assert!(template.contains("[performance]"));
        assert!(template.contains("[security]"));
        assert!(template.contains("[logging]"));
        assert!(template.contains("[monitoring]"));
    }
}
