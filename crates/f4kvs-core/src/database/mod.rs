//! Database and table abstraction layer for F4KVS
//!
//! This module provides a logical abstraction layer above storage engines,
//! allowing multiple databases and tables to coexist in a single F4KVS server
//! instance, each potentially using different storage engines.

pub mod config;
pub mod metadata;
pub mod registry;

use crate::{F4KvsError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a database in F4KVS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    /// Database name
    pub name: String,
    /// Default storage engine for this database
    pub default_engine: String,
    /// Tables in this database
    pub tables: HashMap<String, Table>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Data directory for this database
    pub data_dir: PathBuf,
}

impl Database {
    /// Create a new database
    pub fn new(name: String, default_engine: String, data_dir: PathBuf) -> Self {
        Self {
            name,
            default_engine,
            tables: HashMap::new(),
            created_at: Utc::now(),
            data_dir,
        }
    }

    /// Add a table to this database
    pub fn add_table(&mut self, table: Table) -> Result<()> {
        if self.tables.contains_key(&table.name) {
            return Err(F4KvsError::storage(format!(
                "Table '{}' already exists in database '{}'",
                table.name, self.name
            )));
        }
        self.tables.insert(table.name.clone(), table);
        Ok(())
    }

    /// Get a table by name
    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    /// Remove a table
    pub fn remove_table(&mut self, name: &str) -> Result<()> {
        if self.tables.remove(name).is_none() {
            return Err(F4KvsError::storage(format!(
                "Table '{}' does not exist in database '{}'",
                name, self.name
            )));
        }
        Ok(())
    }

    /// List all table names
    pub fn list_tables(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }
}

/// Represents a table in a database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Database this table belongs to
    pub database: String,
    /// Storage engine for this table (can override database default)
    pub engine: String,
    /// Table schema
    pub schema: TableSchema,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Table-specific configuration
    pub config: TableConfig,
}

impl Table {
    /// Create a new table
    pub fn new(
        name: String,
        database: String,
        engine: String,
        schema: TableSchema,
        config: TableConfig,
    ) -> Self {
        Self {
            name,
            database,
            engine,
            schema,
            created_at: Utc::now(),
            config,
        }
    }

    /// Get the full qualified name (database.table)
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.database, self.name)
    }
}

/// Table schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Column definitions
    pub columns: Vec<ColumnDefinition>,
    /// Primary key columns
    pub primary_key: Vec<String>,
    /// Partition key columns (for analytics engine)
    pub partition_key: Option<Vec<String>>,
}

impl TableSchema {
    /// Create a new schema
    pub fn new(columns: Vec<ColumnDefinition>, primary_key: Vec<String>) -> Self {
        Self {
            columns,
            primary_key,
            partition_key: None,
        }
    }

    /// Create a schema with partition key
    pub fn with_partition_key(
        columns: Vec<ColumnDefinition>,
        primary_key: Vec<String>,
        partition_key: Vec<String>,
    ) -> Self {
        Self {
            columns,
            primary_key,
            partition_key: Some(partition_key),
        }
    }
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    /// Column name
    pub name: String,
    /// Column data type
    pub data_type: ColumnType,
    /// Whether the column is nullable
    pub nullable: bool,
    /// Default value (if any)
    pub default: Option<String>,
}

/// Column data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    /// String type
    String,
    /// 64-bit signed integer
    Int64,
    /// 64-bit unsigned integer
    UInt64,
    /// 64-bit floating point
    Float64,
    /// Boolean
    Bool,
    /// Timestamp
    Timestamp,
    /// Binary data
    Bytes,
    /// JSON data
    Json,
}

impl ColumnType {
    /// Get the type name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            ColumnType::String => "String",
            ColumnType::Int64 => "Int64",
            ColumnType::UInt64 => "UInt64",
            ColumnType::Float64 => "Float64",
            ColumnType::Bool => "Bool",
            ColumnType::Timestamp => "Timestamp",
            ColumnType::Bytes => "Bytes",
            ColumnType::Json => "Json",
        }
    }
}

/// Table-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableConfig {
    /// Partition configuration (for analytics engine)
    pub partition_by: Option<String>,
    /// Partition interval (e.g., "1d", "1h")
    pub partition_interval: Option<String>,
    /// Compression settings
    pub compression: Option<String>,
    /// TTL settings
    pub ttl: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::new(
            "test_db".to_string(),
            "lsm-tree".to_string(),
            PathBuf::from("./data/test_db"),
        );
        assert_eq!(db.name, "test_db");
        assert_eq!(db.default_engine, "lsm-tree");
        assert!(db.tables.is_empty());
    }

    #[test]
    fn test_table_creation() {
        let schema = TableSchema::new(
            vec![ColumnDefinition {
                name: "id".to_string(),
                data_type: ColumnType::Int64,
                nullable: false,
                default: None,
            }],
            vec!["id".to_string()],
        );

        let table = Table::new(
            "users".to_string(),
            "test_db".to_string(),
            "lsm-tree".to_string(),
            schema,
            TableConfig::default(),
        );

        assert_eq!(table.name, "users");
        assert_eq!(table.database, "test_db");
        assert_eq!(table.qualified_name(), "test_db.users");
    }

    #[test]
    fn test_add_remove_table() {
        let mut db = Database::new(
            "test_db".to_string(),
            "lsm-tree".to_string(),
            PathBuf::from("./data/test_db"),
        );

        let schema = TableSchema::new(
            vec![ColumnDefinition {
                name: "id".to_string(),
                data_type: ColumnType::Int64,
                nullable: false,
                default: None,
            }],
            vec!["id".to_string()],
        );

        let table = Table::new(
            "users".to_string(),
            "test_db".to_string(),
            "lsm-tree".to_string(),
            schema,
            TableConfig::default(),
        );

        assert!(db.add_table(table).is_ok());
        assert!(db.get_table("users").is_some());
        assert!(db.remove_table("users").is_ok());
        assert!(db.get_table("users").is_none());
    }
}
