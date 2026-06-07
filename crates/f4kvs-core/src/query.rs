//! Advanced querying operations for F4KVS Core
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{Result, StorageEngine, Value};
use std::collections::HashMap;

/// Query builder for advanced operations
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    prefix: Option<String>,
    start_key: Option<String>,
    end_key: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    include_values: bool,
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            prefix: None,
            start_key: None,
            end_key: None,
            limit: None,
            offset: None,
            include_values: false,
        }
    }

    /// Filter by prefix
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_string());
        self
    }

    /// Filter by key range
    pub fn with_range(mut self, start: &str, end: &str) -> Self {
        self.start_key = Some(start.to_string());
        self.end_key = Some(end.to_string());
        self
    }

    /// Limit the number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip a number of results
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Include values in the results
    pub fn with_values(mut self) -> Self {
        self.include_values = true;
        self
    }

    /// Execute the query against a storage engine
    pub async fn execute<S: StorageEngine>(&self, storage: &S) -> Result<QueryResult> {
        let mut keys = if let Some(prefix) = &self.prefix {
            storage.scan_prefix(prefix).await?
        } else if let (Some(start), Some(end)) = (&self.start_key, &self.end_key) {
            storage.scan_range(start, end).await?
        } else {
            storage.keys().await?
        };

        // Calculate total count before applying offset/limit
        let total_count = keys.len();

        // Apply offset
        if let Some(offset) = self.offset {
            if offset >= keys.len() {
                return Ok(QueryResult::new(Vec::new(), total_count));
            }
            keys = keys.into_iter().skip(offset).collect();
        }

        // Apply limit
        if let Some(limit) = self.limit {
            keys.truncate(limit);
        }

        // Get values if requested
        let pairs = if self.include_values {
            let mut pairs = Vec::new();
            for key in &keys {
                if let Some(value) = storage.get(key).await? {
                    pairs.push((key.clone(), value));
                }
            }
            pairs
        } else {
            keys.into_iter().map(|k| (k, Value::Null)).collect()
        };

        Ok(QueryResult::new(pairs, total_count))
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a query operation
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Key-value pairs (or key-null pairs if values not requested)
    pub pairs: Vec<(String, Value)>,
    /// Total number of matching keys before limit/offset
    pub total_count: usize,
}

impl QueryResult {
    /// Create a new query result
    pub fn new(pairs: Vec<(String, Value)>, total_count: usize) -> Self {
        Self { pairs, total_count }
    }

    /// Get just the keys
    pub fn keys(&self) -> Vec<&String> {
        self.pairs.iter().map(|(k, _)| k).collect()
    }

    /// Get just the values (excluding nulls)
    pub fn values(&self) -> Vec<&Value> {
        self.pairs
            .iter()
            .map(|(_, v)| v)
            .filter(|v| !v.is_null())
            .collect()
    }

    /// Get the number of results
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Check if the result is empty
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Convert to a HashMap
    pub fn to_hashmap(&self) -> HashMap<String, Value> {
        self.pairs.iter().cloned().collect()
    }
}

/// Advanced querying operations for F4KVS Core
pub struct QueryEngine<S: StorageEngine> {
    storage: S,
}

impl<S: StorageEngine> QueryEngine<S> {
    /// Create a new query engine
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Find keys by pattern (simple wildcard matching)
    pub async fn find_keys_by_pattern(&self, pattern: &str) -> Result<Vec<String>> {
        let all_keys = self.storage.keys().await?;
        let mut matching_keys = Vec::new();

        for key in all_keys {
            if self.matches_pattern(&key, pattern) {
                matching_keys.push(key);
            }
        }

        Ok(matching_keys)
    }

    /// Find keys by value type
    pub async fn find_keys_by_value_type(&self, value_type: &str) -> Result<Vec<String>> {
        let all_keys = self.storage.keys().await?;
        let mut matching_keys = Vec::new();

        for key in all_keys {
            if let Some(value) = self.storage.get(&key).await? {
                if value.type_name() == value_type {
                    matching_keys.push(key);
                }
            }
        }

        Ok(matching_keys)
    }

    /// Get statistics for a specific prefix
    pub async fn get_prefix_stats(&self, prefix: &str) -> Result<PrefixStats> {
        let keys = self.storage.scan_prefix(prefix).await?;
        let mut total_size = 0;
        let mut value_types = HashMap::new();

        for key in &keys {
            if let Some(value) = self.storage.get(key).await? {
                total_size += key.len() + value.memory_size();
                *value_types
                    .entry(value.type_name().to_string())
                    .or_insert(0) += 1;
            }
        }

        Ok(PrefixStats {
            key_count: keys.len(),
            total_size,
            value_types,
        })
    }

    /// Check if a pattern matches a key (simple wildcard: * matches any chars)
    fn matches_pattern(&self, key: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') {
            return key == pattern;
        }

        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            key.starts_with(prefix) && key.ends_with(suffix)
        } else {
            // Multiple wildcards - simple implementation
            let mut key_chars = key.chars();
            let mut pattern_chars = pattern.chars();

            while let (Some(k), Some(p)) = (key_chars.next(), pattern_chars.next()) {
                if p == '*' {
                    // Skip to next non-wildcard in pattern
                    for next_p in pattern_chars.by_ref() {
                        if next_p != '*' {
                            // Find this character in the remaining key
                            for next_k in key_chars.by_ref() {
                                if next_k == next_p {
                                    break;
                                }
                            }
                            break;
                        }
                    }
                } else if k != p {
                    return false;
                }
            }

            key_chars.next().is_none() && pattern_chars.next().is_none()
        }
    }
}

/// Statistics for a specific prefix
#[derive(Debug, Clone)]
pub struct PrefixStats {
    /// Number of keys with this prefix
    pub key_count: usize,
    /// Total size of keys and values
    pub total_size: usize,
    /// Count of each value type
    pub value_types: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStorage, StorageMode};

    #[tokio::test]
    async fn test_query_builder_prefix() {
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Add some test data
        storage
            .put("user:1", &Value::String("Alice".to_string()))
            .await
            .unwrap();
        storage
            .put("user:2", &Value::String("Bob".to_string()))
            .await
            .unwrap();
        storage
            .put("admin:1", &Value::String("Admin".to_string()))
            .await
            .unwrap();

        let query = QueryBuilder::new()
            .with_prefix("user:")
            .with_values()
            .execute(&storage)
            .await
            .unwrap();

        assert_eq!(query.len(), 2);
        assert_eq!(query.total_count, 2); // user:1 and user:2
    }

    #[tokio::test]
    async fn test_query_builder_range() {
        let storage = MemoryStorage::with_mode(StorageMode::BTreeMap);

        // Add some test data
        storage
            .put("a", &Value::String("1".to_string()))
            .await
            .unwrap();
        storage
            .put("b", &Value::String("2".to_string()))
            .await
            .unwrap();
        storage
            .put("c", &Value::String("3".to_string()))
            .await
            .unwrap();
        storage
            .put("d", &Value::String("4".to_string()))
            .await
            .unwrap();

        let query = QueryBuilder::new()
            .with_range("b", "d")
            .with_values()
            .execute(&storage)
            .await
            .unwrap();

        assert_eq!(query.len(), 2); // b and c
        assert_eq!(query.total_count, 2);
    }

    #[tokio::test]
    async fn test_query_builder_limit_offset() {
        let storage = MemoryStorage::with_mode(StorageMode::BTreeMap);

        // Add some test data
        for i in 0..10 {
            storage
                .put(&format!("key_{:02}", i), &Value::Int64(i as i64))
                .await
                .unwrap();
        }

        let query = QueryBuilder::new()
            .with_offset(2)
            .with_limit(3)
            .with_values()
            .execute(&storage)
            .await
            .unwrap();

        assert_eq!(query.len(), 3);
        assert_eq!(query.total_count, 10);
    }

    #[tokio::test]
    async fn test_query_engine_pattern_matching() {
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Add some test data
        storage
            .put("user:alice", &Value::String("Alice".to_string()))
            .await
            .unwrap();
        storage
            .put("user:bob", &Value::String("Bob".to_string()))
            .await
            .unwrap();
        storage
            .put("admin:charlie", &Value::String("Charlie".to_string()))
            .await
            .unwrap();

        let query_engine = QueryEngine::new(storage);

        let keys = query_engine.find_keys_by_pattern("user:*").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user:alice".to_string()));
        assert!(keys.contains(&"user:bob".to_string()));
    }

    #[tokio::test]
    async fn test_query_engine_value_type_filtering() {
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Add some test data
        storage
            .put("str1", &Value::String("hello".to_string()))
            .await
            .unwrap();
        storage.put("int1", &Value::Int64(42)).await.unwrap();
        storage
            .put("str2", &Value::String("world".to_string()))
            .await
            .unwrap();
        storage.put("bool1", &Value::Bool(true)).await.unwrap();

        let query_engine = QueryEngine::new(storage);

        let string_keys = query_engine
            .find_keys_by_value_type("String")
            .await
            .unwrap();
        assert_eq!(string_keys.len(), 2);
        assert!(string_keys.contains(&"str1".to_string()));
        assert!(string_keys.contains(&"str2".to_string()));
    }

    #[tokio::test]
    async fn test_query_engine_prefix_stats() {
        let storage = MemoryStorage::with_mode(StorageMode::HashMap);

        // Add some test data
        storage
            .put("user:1", &Value::String("Alice".to_string()))
            .await
            .unwrap();
        storage.put("user:2", &Value::Int64(25)).await.unwrap();
        storage
            .put("admin:1", &Value::String("Admin".to_string()))
            .await
            .unwrap();

        let query_engine = QueryEngine::new(storage);

        let stats = query_engine.get_prefix_stats("user:").await.unwrap();
        assert_eq!(stats.key_count, 2);
        assert!(stats.total_size > 0);
        assert_eq!(stats.value_types.get("String"), Some(&1));
        assert_eq!(stats.value_types.get("Int64"), Some(&1));
    }
}
