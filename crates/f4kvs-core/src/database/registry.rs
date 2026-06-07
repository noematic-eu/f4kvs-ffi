//! Engine registry for managing multiple storage engine instances
//!
//! This module provides a centralized registry that manages multiple storage
//! engine instances, allowing different databases and tables to use different
//! storage engines within a single F4KVS server.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::storage_traits::StorageEngine as StorageEngineTrait;
use crate::{F4KvsError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for creating a storage engine instance
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Engine type (e.g., "memory", "lsm-tree", "analytics")
    pub engine_type: String,
    /// Data directory for this engine instance
    pub data_dir: String,
    /// Additional configuration options
    pub options: HashMap<String, String>,
}

type EngineFactoryFn =
    dyn Fn(&EngineConfig) -> Result<Arc<dyn StorageEngineTrait + Send + Sync>> + Send + Sync;

/// Engine registry that manages multiple storage engine instances
pub struct EngineRegistry {
    /// Map of engine ID to engine instance
    engines: Arc<RwLock<HashMap<String, Arc<dyn StorageEngineTrait + Send + Sync>>>>,
    /// Map of database name to engine ID
    database_engines: Arc<RwLock<HashMap<String, String>>>,
    /// Map of "database.table" to engine ID
    table_engines: Arc<RwLock<HashMap<String, String>>>,
    /// Engine factory function
    engine_factory: Box<EngineFactoryFn>,
}

impl EngineRegistry {
    /// Create a new engine registry
    pub fn new<F>(engine_factory: F) -> Self
    where
        F: Fn(&EngineConfig) -> Result<Arc<dyn StorageEngineTrait + Send + Sync>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            database_engines: Arc::new(RwLock::new(HashMap::new())),
            table_engines: Arc::new(RwLock::new(HashMap::new())),
            engine_factory: Box::new(engine_factory),
        }
    }

    /// Create a new engine instance
    pub async fn create_engine(&self, engine_id: String, config: EngineConfig) -> Result<()> {
        let mut engines = self.engines.write().await;

        if engines.contains_key(&engine_id) {
            return Err(F4KvsError::storage(format!(
                "Engine '{}' already exists",
                engine_id
            )));
        }

        let engine = (self.engine_factory)(&config)?;
        engines.insert(engine_id.clone(), engine);

        Ok(())
    }

    /// Get an engine instance by ID
    pub async fn get_engine_by_id(
        &self,
        engine_id: &str,
    ) -> Result<Arc<dyn StorageEngineTrait + Send + Sync>> {
        let engines = self.engines.read().await;
        engines
            .get(engine_id)
            .cloned()
            .ok_or_else(|| F4KvsError::storage(format!("Engine '{}' not found", engine_id)))
    }

    /// Get the engine for a database and optional table
    pub async fn get_engine(
        &self,
        database: &str,
        table: Option<&str>,
    ) -> Result<Arc<dyn StorageEngineTrait + Send + Sync>> {
        // First check if there's a table-specific engine
        if let Some(tbl) = table {
            let table_key = format!("{}.{}", database, tbl);
            let table_engines = self.table_engines.read().await;
            if let Some(engine_id) = table_engines.get(&table_key) {
                return self.get_engine_by_id(engine_id).await;
            }
        }

        // Fall back to database-level engine
        let database_engines = self.database_engines.read().await;
        if let Some(engine_id) = database_engines.get(database) {
            return self.get_engine_by_id(engine_id).await;
        }

        Err(F4KvsError::storage(format!(
            "No engine configured for database '{}'",
            database
        )))
    }

    /// Assign an engine to a database
    pub async fn assign_engine_to_database(&self, database: &str, engine_id: &str) -> Result<()> {
        // Verify engine exists
        self.get_engine_by_id(engine_id).await?;

        let mut database_engines = self.database_engines.write().await;
        database_engines.insert(database.to_string(), engine_id.to_string());
        Ok(())
    }

    /// Assign an engine to a table
    pub async fn assign_engine_to_table(
        &self,
        database: &str,
        table: &str,
        engine_id: &str,
    ) -> Result<()> {
        // Verify engine exists
        self.get_engine_by_id(engine_id).await?;

        let mut table_engines = self.table_engines.write().await;
        let table_key = format!("{}.{}", database, table);
        table_engines.insert(table_key, engine_id.to_string());
        Ok(())
    }

    /// Remove an engine assignment from a database
    pub async fn unassign_engine_from_database(&self, database: &str) -> Result<()> {
        let mut database_engines = self.database_engines.write().await;
        database_engines.remove(database).ok_or_else(|| {
            F4KvsError::storage(format!("Database '{}' has no engine assigned", database))
        })?;
        Ok(())
    }

    /// Remove an engine assignment from a table
    pub async fn unassign_engine_from_table(&self, database: &str, table: &str) -> Result<()> {
        let mut table_engines = self.table_engines.write().await;
        let table_key = format!("{}.{}", database, table);
        table_engines.remove(&table_key).ok_or_else(|| {
            F4KvsError::storage(format!(
                "Table '{}.{}' has no engine assigned",
                database, table
            ))
        })?;
        Ok(())
    }

    /// List all engine IDs
    pub async fn list_engines(&self) -> Vec<String> {
        let engines = self.engines.read().await;
        engines.keys().cloned().collect()
    }

    /// Remove an engine instance
    pub async fn remove_engine(&self, engine_id: &str) -> Result<()> {
        let mut engines = self.engines.write().await;

        if !engines.contains_key(engine_id) {
            return Err(F4KvsError::storage(format!(
                "Engine '{}' not found",
                engine_id
            )));
        }

        // Check if engine is still assigned
        let database_engines = self.database_engines.read().await;
        let table_engines = self.table_engines.read().await;

        if database_engines.values().any(|id| id == engine_id) {
            return Err(F4KvsError::storage(format!(
                "Cannot remove engine '{}' - still assigned to a database",
                engine_id
            )));
        }

        if table_engines.values().any(|id| id == engine_id) {
            return Err(F4KvsError::storage(format!(
                "Cannot remove engine '{}' - still assigned to a table",
                engine_id
            )));
        }

        engines.remove(engine_id);
        Ok(())
    }

    /// Get engine statistics
    pub async fn get_engine_stats(&self, engine_id: &str) -> Result<crate::StorageStats> {
        let engine = self.get_engine_by_id(engine_id).await?;
        engine.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_storage::MemoryStorage;

    fn create_test_registry() -> EngineRegistry {
        EngineRegistry::new(|_config: &EngineConfig| {
            // Simple factory that creates MemoryStorage for testing
            Ok(Arc::new(MemoryStorage::new()) as Arc<dyn StorageEngineTrait + Send + Sync>)
        })
    }

    #[tokio::test]
    async fn test_create_engine() {
        let registry = create_test_registry();
        let config = EngineConfig {
            engine_type: "memory".to_string(),
            data_dir: "./data/test".to_string(),
            options: HashMap::new(),
        };

        assert!(registry
            .create_engine("engine1".to_string(), config)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_assign_engine_to_database() {
        let registry = create_test_registry();
        let config = EngineConfig {
            engine_type: "memory".to_string(),
            data_dir: "./data/test".to_string(),
            options: HashMap::new(),
        };

        registry
            .create_engine("engine1".to_string(), config)
            .await
            .unwrap();
        assert!(registry
            .assign_engine_to_database("test_db", "engine1")
            .await
            .is_ok());

        let engine = registry.get_engine("test_db", None).await.unwrap();
        assert!(engine.get("test_key").await.is_ok());
    }

    #[tokio::test]
    async fn test_table_engine_override() {
        let registry = create_test_registry();

        let config1 = EngineConfig {
            engine_type: "memory".to_string(),
            data_dir: "./data/test1".to_string(),
            options: HashMap::new(),
        };
        let config2 = EngineConfig {
            engine_type: "memory".to_string(),
            data_dir: "./data/test2".to_string(),
            options: HashMap::new(),
        };

        registry
            .create_engine("db_engine".to_string(), config1)
            .await
            .unwrap();
        registry
            .create_engine("table_engine".to_string(), config2)
            .await
            .unwrap();

        registry
            .assign_engine_to_database("test_db", "db_engine")
            .await
            .unwrap();
        registry
            .assign_engine_to_table("test_db", "users", "table_engine")
            .await
            .unwrap();

        // Table should use table_engine
        let table_engine = registry.get_engine("test_db", Some("users")).await.unwrap();

        // Other tables should use db_engine
        let db_engine = registry
            .get_engine("test_db", Some("orders"))
            .await
            .unwrap();

        // They should be different instances
        assert!(
            std::ptr::eq(
                Arc::as_ptr(&table_engine) as *const (),
                Arc::as_ptr(&db_engine) as *const ()
            ) == false
        );
    }
}
