//! Configuration system for database and table engine selection
//!
//! This module provides configuration parsing and management for
//! per-database and per-table storage engine selection.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{F4KvsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::registry::EngineConfig;

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Default storage engine for this database
    pub default_engine: String,
    /// Data directory for this database
    pub data_dir: PathBuf,
    /// Engine-specific configuration
    pub engine_config: Option<HashMap<String, String>>,
    /// Tables in this database
    #[serde(default)]
    pub tables: HashMap<String, TableEngineConfig>,
}

/// Table-specific engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEngineConfig {
    /// Storage engine for this table (overrides database default)
    pub engine: Option<String>,
    /// Partition configuration
    pub partition_by: Option<String>,
    /// Partition interval (e.g., "1d", "1h")
    pub partition_interval: Option<String>,
    /// Compression settings
    pub compression: Option<String>,
    /// TTL settings
    pub ttl: Option<u64>,
}

/// Root configuration for databases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasesConfig {
    /// Map of database name to configuration
    #[serde(default)]
    pub databases: HashMap<String, DatabaseConfig>,
}

impl DatabasesConfig {
    /// Load configuration from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str)
            .map_err(|e| F4KvsError::config(format!("Failed to parse TOML: {}", e)))
    }

    /// Load configuration from file
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| F4KvsError::io(format!("Failed to read config file: {}", e)))?;
        Self::from_toml(&contents)
    }

    /// Get database configuration
    pub fn get_database(&self, name: &str) -> Option<&DatabaseConfig> {
        self.databases.get(name)
    }

    /// Get table engine configuration
    pub fn get_table_config(&self, database: &str, table: &str) -> Option<&TableEngineConfig> {
        self.databases
            .get(database)
            .and_then(|db| db.tables.get(table))
    }

    /// Get engine configuration for a database
    pub fn get_database_engine_config(&self, database: &str) -> Result<EngineConfig> {
        let db_config = self.databases.get(database).ok_or_else(|| {
            F4KvsError::config(format!("Database '{}' not found in config", database))
        })?;

        Ok(EngineConfig {
            engine_type: db_config.default_engine.clone(),
            data_dir: db_config.data_dir.to_string_lossy().to_string(),
            options: db_config.engine_config.clone().unwrap_or_default(),
        })
    }

    /// Get engine configuration for a table
    pub fn get_table_engine_config(&self, database: &str, table: &str) -> Result<EngineConfig> {
        let db_config = self.databases.get(database).ok_or_else(|| {
            F4KvsError::config(format!("Database '{}' not found in config", database))
        })?;

        let table_config = db_config.tables.get(table);

        let engine_type = table_config
            .and_then(|tc| tc.engine.as_ref())
            .cloned()
            .unwrap_or_else(|| db_config.default_engine.clone());

        let mut options = db_config.engine_config.clone().unwrap_or_default();

        if let Some(tc) = table_config {
            if let Some(ref partition_by) = tc.partition_by {
                options.insert("partition_by".to_string(), partition_by.clone());
            }
            if let Some(ref partition_interval) = tc.partition_interval {
                options.insert("partition_interval".to_string(), partition_interval.clone());
            }
            if let Some(ref compression) = tc.compression {
                options.insert("compression".to_string(), compression.clone());
            }
            if let Some(ttl) = tc.ttl {
                options.insert("ttl".to_string(), ttl.to_string());
            }
        }

        Ok(EngineConfig {
            engine_type,
            data_dir: db_config.data_dir.to_string_lossy().to_string(),
            options,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[databases.analytics_db]
default_engine = "analytics"
data_dir = "./data/analytics_db"

[databases.analytics_db.tables.events]
engine = "analytics"
partition_by = "timestamp"
partition_interval = "1d"

[databases.analytics_db.tables.users]
engine = "lsm-tree"

[databases.cache_db]
default_engine = "memory"
data_dir = "./data/cache_db"
"#;

        let config = DatabasesConfig::from_toml(toml_str).unwrap();

        assert!(config.get_database("analytics_db").is_some());
        assert!(config.get_database("cache_db").is_some());

        let analytics_db = config.get_database("analytics_db").unwrap();
        assert_eq!(analytics_db.default_engine, "analytics");

        let events_config = config.get_table_config("analytics_db", "events").unwrap();
        assert_eq!(events_config.engine, Some("analytics".to_string()));
        assert_eq!(events_config.partition_by, Some("timestamp".to_string()));
    }

    #[test]
    fn test_get_engine_config() {
        let toml_str = r#"
[databases.test_db]
default_engine = "lsm-tree"
data_dir = "./data/test_db"

[databases.test_db.tables.users]
engine = "memory"
"#;

        let config = DatabasesConfig::from_toml(toml_str).unwrap();

        let db_config = config.get_database_engine_config("test_db").unwrap();
        assert_eq!(db_config.engine_type, "lsm-tree");

        let table_config = config.get_table_engine_config("test_db", "users").unwrap();
        assert_eq!(table_config.engine_type, "memory");

        // Table without explicit engine should use database default
        let table_config2 = config.get_table_engine_config("test_db", "orders").unwrap();
        assert_eq!(table_config2.engine_type, "lsm-tree");
    }
}
