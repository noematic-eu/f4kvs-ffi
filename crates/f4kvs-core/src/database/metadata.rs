//! Metadata storage for databases and tables
//!
//! This module handles persistence of database and table metadata,
//! allowing the system to recover database/table definitions on startup.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::storage_traits::StorageEngine as StorageEngineTrait;
use crate::Value;
use crate::{F4KvsError, Result};
use std::collections::HashMap;
use std::sync::Arc;

use super::{Database, Table};

/// Metadata manager for databases and tables
pub struct MetadataManager {
    /// Storage engine for metadata
    storage: Arc<dyn StorageEngineTrait + Send + Sync>,
}

impl MetadataManager {
    /// Create a new metadata manager
    pub fn new(storage: Arc<dyn StorageEngineTrait + Send + Sync>) -> Self {
        Self { storage }
    }

    /// Store database metadata
    pub async fn store_database(&self, database: &Database) -> Result<()> {
        let key = format!("db:{}", database.name);
        let value = serde_json::to_string(database).map_err(|e| {
            F4KvsError::serialization(format!("Failed to serialize database: {}", e))
        })?;

        self.storage.put(&key, &Value::String(value)).await?;

        Ok(())
    }

    /// Load database metadata
    pub async fn load_database(&self, name: &str) -> Result<Option<Database>> {
        let key = format!("db:{}", name);
        match self.storage.get(&key).await? {
            Some(Value::String(json)) => {
                let db: Database = serde_json::from_str(&json).map_err(|e| {
                    F4KvsError::serialization(format!("Failed to deserialize database: {}", e))
                })?;
                Ok(Some(db))
            }
            _ => Ok(None),
        }
    }

    /// Store table metadata
    pub async fn store_table(&self, table: &Table) -> Result<()> {
        let key = format!("table:{}:{}", table.database, table.name);
        let value = serde_json::to_string(table)
            .map_err(|e| F4KvsError::serialization(format!("Failed to serialize table: {}", e)))?;

        self.storage.put(&key, &Value::String(value)).await?;

        // Also update the database's table list
        self.update_database_table_list(&table.database, &table.name, true)
            .await?;

        Ok(())
    }

    /// Load table metadata
    pub async fn load_table(&self, database: &str, table: &str) -> Result<Option<Table>> {
        let key = format!("table:{}:{}", database, table);
        match self.storage.get(&key).await? {
            Some(Value::String(json)) => {
                let tbl: Table = serde_json::from_str(&json).map_err(|e| {
                    F4KvsError::serialization(format!("Failed to deserialize table: {}", e))
                })?;
                Ok(Some(tbl))
            }
            _ => Ok(None),
        }
    }

    /// List all databases
    pub async fn list_databases(&self) -> Result<Vec<String>> {
        let prefix = "db:";
        let keys = self.storage.scan_prefix(prefix).await?;

        Ok(keys
            .iter()
            .filter_map(|k| k.strip_prefix(prefix))
            .map(|s| s.to_string())
            .collect())
    }

    /// List all tables in a database
    pub async fn list_tables(&self, database: &str) -> Result<Vec<String>> {
        let prefix = format!("table:{}:", database);
        let keys = self.storage.scan_prefix(&prefix).await?;

        Ok(keys
            .iter()
            .filter_map(|k| k.strip_prefix(&prefix))
            .map(|s| s.to_string())
            .collect())
    }

    /// Delete database metadata
    pub async fn delete_database(&self, name: &str) -> Result<()> {
        let key = format!("db:{}", name);
        self.storage.delete(&key).await?;
        Ok(())
    }

    /// Delete table metadata
    pub async fn delete_table(&self, database: &str, table: &str) -> Result<()> {
        let key = format!("table:{}:{}", database, table);
        self.storage.delete(&key).await?;

        // Update the database's table list
        self.update_database_table_list(database, table, false)
            .await?;

        Ok(())
    }

    /// Update database's table list
    async fn update_database_table_list(
        &self,
        database: &str,
        table: &str,
        add: bool,
    ) -> Result<()> {
        let mut db = match self.load_database(database).await? {
            Some(db) => db,
            None => {
                return Err(F4KvsError::storage(format!(
                    "Database '{}' not found",
                    database
                )))
            }
        };

        if add {
            // Table will be added when we reload from metadata
            // For now, we just ensure the database exists
        } else {
            db.tables.remove(table);
        }

        self.store_database(&db).await?;
        Ok(())
    }

    /// Load all databases and their tables
    pub async fn load_all(&self) -> Result<HashMap<String, Database>> {
        let db_names = self.list_databases().await?;
        let mut databases = HashMap::new();

        for db_name in db_names {
            if let Some(mut db) = self.load_database(&db_name).await? {
                // Load all tables for this database
                let table_names = self.list_tables(&db_name).await?;
                for table_name in table_names {
                    if let Some(table) = self.load_table(&db_name, &table_name).await? {
                        db.tables.insert(table_name, table);
                    }
                }
                databases.insert(db_name, db);
            }
        }

        Ok(databases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_storage::MemoryStorage;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_store_load_database() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = MetadataManager::new(storage);

        let db = Database::new(
            "test_db".to_string(),
            "lsm-tree".to_string(),
            PathBuf::from("./data/test_db"),
        );

        manager.store_database(&db).await.unwrap();
        let loaded = manager.load_database("test_db").await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test_db");
    }

    #[tokio::test]
    async fn test_store_load_table() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = MetadataManager::new(storage);

        let db = Database::new(
            "test_db".to_string(),
            "lsm-tree".to_string(),
            PathBuf::from("./data/test_db"),
        );
        manager.store_database(&db).await.unwrap();

        use super::super::{ColumnDefinition, ColumnType, TableConfig, TableSchema};
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

        manager.store_table(&table).await.unwrap();
        let loaded = manager.load_table("test_db", "users").await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "users");
    }
}
