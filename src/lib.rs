//! # ContextDB
//!
//! A novel database for semantic data with human accountability.
//!
//! ContextDB treats meaning (vector embeddings) and expression (human-readable text)
//! as co-equal representations of the same data. It provides:
//!
//! - **Semantic search** via vector similarity
//! - **Human-readable queries** via structured filters
//! - **Graph relationships** between entries
//! - **Temporal queries** with built-in provenance
//! - **Unified query language** that blends all modalities
//!
//! ## Example
//!
//! ```
//! use contextdb::{ContextDB, Entry, Query, ExpressionFilter};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an in-memory database
//! let mut db = ContextDB::in_memory()?;
//!
//! // Insert an entry with semantic meaning and human expression
//! let entry = Entry::new(
//!     vec![0.1, 0.2, 0.3],  // embedding vector
//!     "User doesn't like red onions".to_string(),
//! );
//! db.insert(&entry)?;
//!
//! // Query by semantic similarity
//! let query = Query::new()
//!     .with_meaning(vec![0.15, 0.25, 0.35], Some(0.8))
//!     .with_limit(10);
//!
//! let results = db.query(&query)?;
//!
//! // Query by text content
//! let text_query = Query::new()
//!     .with_expression(ExpressionFilter::Contains("onion".to_string()));
//!
//! let text_results = db.query(&text_query)?;
//! # Ok(())
//! # }
//! ```

mod query;
mod storage;
mod types;

pub use query::{
	ContextFilter, ExpressionFilter, MeaningFilter, Query, QueryResult, RelationFilter,
	TemporalFilter,
};
pub use storage::{SqliteStorage, StorageBackend, StorageError, StorageResult};
pub use types::{cosine_similarity, Entry};

#[cfg(feature = "ffi")]
pub mod ffi;

use std::path::Path;

/// Main ContextDB interface
///
/// Uses a trait-based storage backend, allowing you to swap SQLite, PostgreSQL, MySQL, etc.
pub struct ContextDB {
	storage: Box<dyn StorageBackend>,
}

impl ContextDB {
	/// Create a new in-memory ContextDB instance using SQLite
	pub fn in_memory() -> StorageResult<Self> {
		Ok(Self {
			storage: Box::new(SqliteStorage::in_memory()?),
		})
	}

	/// Create a new file-backed ContextDB instance using SQLite
	pub fn new<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
		Ok(Self {
			storage: Box::new(SqliteStorage::new(path)?),
		})
	}

	/// Create a ContextDB with a custom storage backend
	///
	/// This allows you to use PostgreSQL, MySQL, or any other backend that implements StorageBackend
	pub fn with_backend<B: StorageBackend + 'static>(backend: B) -> Self {
		Self {
			storage: Box::new(backend),
		}
	}

	/// Insert a new entry into the database
	pub fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
		self.storage.insert(entry)
	}

	/// Get an entry by its ID
	pub fn get(&self, id: uuid::Uuid) -> StorageResult<Entry> {
		self.storage.get(id)
	}

	/// Execute a query and return matching entries
	pub fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
		self.storage.query(query)
	}

	/// Update an existing entry
	pub fn update(&mut self, entry: &Entry) -> StorageResult<()> {
		self.storage.update(entry)
	}

	/// Delete an entry by ID
	pub fn delete(&mut self, id: uuid::Uuid) -> StorageResult<()> {
		self.storage.delete(id)
	}

	/// Count total entries in the database
	pub fn count(&self) -> StorageResult<usize> {
		self.storage.count()
	}

	/// Get the name of the storage backend being used
	pub fn backend_name(&self) -> &str {
		self.storage.backend_name()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::{TimeZone, Utc};

	// ==================== Basic Operations ====================

	#[test]
	fn test_basic_operations() {
		let mut db = ContextDB::in_memory().unwrap();

		let entry = Entry::new(vec![0.1, 0.2, 0.3], "Test entry".to_string());

		db.insert(&entry).unwrap();
		assert_eq!(db.count().unwrap(), 1);

		let retrieved = db.get(entry.id).unwrap();
		assert_eq!(retrieved.expression, "Test entry");
	}

	#[test]
	fn test_in_memory_creation() {
		let db = ContextDB::in_memory();
		assert!(db.is_ok());
		assert_eq!(db.unwrap().backend_name(), "SQLite");
	}

	#[test]
	fn test_with_custom_backend() {
		let backend = SqliteStorage::in_memory().unwrap();
		let db = ContextDB::with_backend(backend);
		assert_eq!(db.backend_name(), "SQLite");
	}

	#[test]
	fn test_empty_database_count() {
		let db = ContextDB::in_memory().unwrap();
		assert_eq!(db.count().unwrap(), 0);
	}

	#[test]
	fn test_get_nonexistent_entry() {
		let db = ContextDB::in_memory().unwrap();
		let fake_id = uuid::Uuid::new_v4();
		assert!(db.get(fake_id).is_err());
	}

	// ==================== CRUD Operations ====================

	#[test]
	fn test_insert_and_retrieve() {
		let mut db = ContextDB::in_memory().unwrap();
		let entry = Entry::new(vec![0.1, 0.2], "Test".to_string())
			.with_context(serde_json::json!({"key": "value"}));

		db.insert(&entry).unwrap();
		let retrieved = db.get(entry.id).unwrap();

		assert_eq!(retrieved.id, entry.id);
		assert_eq!(retrieved.expression, entry.expression);
		assert_eq!(retrieved.meaning, entry.meaning);
		assert_eq!(retrieved.context, entry.context);
	}

	#[test]
	fn test_update_entry() {
		let mut db = ContextDB::in_memory().unwrap();
		let mut entry = Entry::new(vec![0.1], "Original".to_string());
		db.insert(&entry).unwrap();

		entry.expression = "Updated".to_string();
		entry.updated_at = Utc::now();
		db.update(&entry).unwrap();

		let retrieved = db.get(entry.id).unwrap();
		assert_eq!(retrieved.expression, "Updated");
	}

	#[test]
	fn test_delete_entry() {
		let mut db = ContextDB::in_memory().unwrap();
		let entry = Entry::new(vec![0.1], "To be deleted".to_string());
		db.insert(&entry).unwrap();
		assert_eq!(db.count().unwrap(), 1);

		db.delete(entry.id).unwrap();
		assert_eq!(db.count().unwrap(), 0);
	}

	#[test]
	fn test_delete_nonexistent_entry() {
		let mut db = ContextDB::in_memory().unwrap();
		let fake_id = uuid::Uuid::new_v4();
		assert!(db.delete(fake_id).is_err());
	}

	// ==================== Semantic Query Tests ====================

	#[test]
	fn test_semantic_query() {
		let mut db = ContextDB::in_memory().unwrap();

		let entry1 = Entry::new(vec![1.0, 0.0, 0.0], "First entry".to_string());
		let entry2 = Entry::new(vec![0.9, 0.1, 0.0], "Similar entry".to_string());
		let entry3 = Entry::new(vec![0.0, 0.0, 1.0], "Different entry".to_string());

		db.insert(&entry1).unwrap();
		db.insert(&entry2).unwrap();
		db.insert(&entry3).unwrap();

		let query = Query::new()
			.with_meaning(vec![1.0, 0.0, 0.0], Some(0.7))
			.with_limit(2);

		let results = db.query(&query).unwrap();
		assert!(results.len() <= 2);
		assert!(results[0].similarity_score.unwrap() > 0.7);
	}

	#[test]
	fn test_semantic_query_ordering() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![1.0, 0.0], "Exact match".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.7, 0.7], "Partial match".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.0, 1.0], "No match".to_string()))
			.unwrap();

		let query = Query::new().with_meaning(vec![1.0, 0.0], None);
		let results = db.query(&query).unwrap();

		// Results should be ordered by similarity (highest first)
		assert!(results[0].similarity_score.unwrap() >= results[1].similarity_score.unwrap());
		assert!(results[1].similarity_score.unwrap() >= results[2].similarity_score.unwrap());
	}

	// ==================== Text Query Tests ====================

	#[test]
	fn test_text_query() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1, 0.2], "loves onions".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.1, 0.2], "hates red onions".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.1, 0.2], "prefers garlic".to_string()))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::Contains("onion".to_string()));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_text_query_equals() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "exact match".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.2], "Exact Match".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.3], "exact match with more".to_string()))
			.unwrap();

		let query =
			Query::new().with_expression(ExpressionFilter::Equals("exact match".to_string()));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
	}

	#[test]
	fn test_text_query_starts_with() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "Hello World".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.2], "Hello There".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.3], "World Hello".to_string()))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::StartsWith("Hello".to_string()));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 2);
	}

	// ==================== Context Query Tests ====================

	#[test]
	fn test_context_query() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(
			&Entry::new(vec![0.1], "User 1".to_string())
				.with_context(serde_json::json!({"role": "admin"})),
		)
		.unwrap();
		db.insert(
			&Entry::new(vec![0.2], "User 2".to_string())
				.with_context(serde_json::json!({"role": "user"})),
		)
		.unwrap();
		db.insert(
			&Entry::new(vec![0.3], "User 3".to_string())
				.with_context(serde_json::json!({"role": "admin"})),
		)
		.unwrap();

		let query = Query::new().with_context(ContextFilter::PathEquals(
			"/role".to_string(),
			serde_json::json!("admin"),
		));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_context_query_nested() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(
			&Entry::new(vec![0.1], "Entry 1".to_string()).with_context(serde_json::json!({
				"metadata": {"level": 1, "active": true}
			})),
		)
		.unwrap();
		db.insert(
			&Entry::new(vec![0.2], "Entry 2".to_string()).with_context(serde_json::json!({
				"metadata": {"level": 2, "active": false}
			})),
		)
		.unwrap();

		let query = Query::new().with_context(ContextFilter::PathEquals(
			"/metadata/active".to_string(),
			serde_json::json!(true),
		));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "Entry 1");
	}

	#[test]
	fn test_context_query_path_contains() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(
			&Entry::new(vec![0.1], "Tagged".to_string())
				.with_context(serde_json::json!({"tags": ["important", "urgent"]})),
		)
		.unwrap();
		db.insert(
			&Entry::new(vec![0.2], "Not tagged".to_string())
				.with_context(serde_json::json!({"tags": ["normal"]})),
		)
		.unwrap();

		let query = Query::new().with_context(ContextFilter::PathContains(
			"/tags".to_string(),
			serde_json::json!("urgent"),
		));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
	}

	// ==================== Temporal Query Tests ====================

	#[test]
	fn test_temporal_query_created_after() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "Recent entry".to_string()))
			.unwrap();

		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let query = Query::new().with_temporal(TemporalFilter::CreatedAfter(past));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
	}

	#[test]
	fn test_temporal_query_created_before_filters_out() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "Entry".to_string()))
			.unwrap();

		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let query = Query::new().with_temporal(TemporalFilter::CreatedBefore(past));

		let results = db.query(&query).unwrap();
		assert!(results.is_empty());
	}

	// ==================== Combined Query Tests ====================

	#[test]
	fn test_combined_semantic_and_text_query() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![1.0, 0.0], "Hello semantic".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![1.0, 0.0], "Goodbye semantic".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.0, 1.0], "Hello different".to_string()))
			.unwrap();

		let query = Query::new()
			.with_meaning(vec![1.0, 0.0], Some(0.9))
			.with_expression(ExpressionFilter::Contains("hello".to_string()));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "Hello semantic");
	}

	#[test]
	fn test_combined_all_filters() {
		let mut db = ContextDB::in_memory().unwrap();

		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();

		// This entry should match all filters
		db.insert(
			&Entry::new(vec![1.0, 0.0], "Target entry".to_string())
				.with_context(serde_json::json!({"type": "target"})),
		)
		.unwrap();

		// Wrong vector
		db.insert(
			&Entry::new(vec![0.0, 1.0], "Target entry".to_string())
				.with_context(serde_json::json!({"type": "target"})),
		)
		.unwrap();

		// Wrong text
		db.insert(
			&Entry::new(vec![1.0, 0.0], "Other entry".to_string())
				.with_context(serde_json::json!({"type": "target"})),
		)
		.unwrap();

		// Wrong context
		db.insert(
			&Entry::new(vec![1.0, 0.0], "Target entry".to_string())
				.with_context(serde_json::json!({"type": "other"})),
		)
		.unwrap();

		let query = Query::new()
			.with_meaning(vec![1.0, 0.0], Some(0.9))
			.with_expression(ExpressionFilter::Contains("target".to_string()))
			.with_context(ContextFilter::PathEquals(
				"/type".to_string(),
				serde_json::json!("target"),
			))
			.with_temporal(TemporalFilter::CreatedAfter(past));

		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 1);
	}

	// ==================== Query Options Tests ====================

	#[test]
	fn test_query_with_limit() {
		let mut db = ContextDB::in_memory().unwrap();

		for i in 0..20 {
			db.insert(&Entry::new(vec![i as f32], format!("Entry {}", i)))
				.unwrap();
		}

		let query = Query::new().with_limit(5);
		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 5);
	}

	#[test]
	fn test_query_with_explanation() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![1.0, 0.0], "Test".to_string()))
			.unwrap();

		let query = Query::new()
			.with_meaning(vec![1.0, 0.0], Some(0.9))
			.with_explanation();

		let results = db.query(&query).unwrap();
		assert!(results[0].explanation.is_some());
	}

	// ==================== Relations Tests ====================

	#[test]
	fn test_entry_with_relations() {
		let mut db = ContextDB::in_memory().unwrap();

		let entry1 = Entry::new(vec![0.1], "Entry 1".to_string());
		let entry2 = Entry::new(vec![0.2], "Entry 2".to_string());
		db.insert(&entry1).unwrap();
		db.insert(&entry2).unwrap();

		let entry3 = Entry::new(vec![0.3], "Entry 3".to_string())
			.add_relation(entry1.id)
			.add_relation(entry2.id);
		db.insert(&entry3).unwrap();

		let retrieved = db.get(entry3.id).unwrap();
		assert_eq!(retrieved.relations.len(), 2);
		assert!(retrieved.relations.contains(&entry1.id));
		assert!(retrieved.relations.contains(&entry2.id));
	}

	// ==================== Edge Cases ====================

	#[test]
	fn test_empty_query_returns_all() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "Entry 1".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.2], "Entry 2".to_string()))
			.unwrap();
		db.insert(&Entry::new(vec![0.3], "Entry 3".to_string()))
			.unwrap();

		let query = Query::new();
		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn test_query_no_matches() {
		let mut db = ContextDB::in_memory().unwrap();

		db.insert(&Entry::new(vec![0.1], "Entry".to_string()))
			.unwrap();

		let query =
			Query::new().with_expression(ExpressionFilter::Equals("Nonexistent".to_string()));

		let results = db.query(&query).unwrap();
		assert!(results.is_empty());
	}

	#[test]
	fn test_unicode_content() {
		let mut db = ContextDB::in_memory().unwrap();

		let entry = Entry::new(vec![0.1], "Hello 世界 🌍 مرحبا".to_string())
			.with_context(serde_json::json!({"greeting": "你好"}));
		db.insert(&entry).unwrap();

		let retrieved = db.get(entry.id).unwrap();
		assert_eq!(retrieved.expression, "Hello 世界 🌍 مرحبا");
		assert_eq!(retrieved.context["greeting"], "你好");
	}

	#[test]
	fn test_large_vector() {
		let mut db = ContextDB::in_memory().unwrap();

		let large_vector: Vec<f32> = (0..1536).map(|i| i as f32 / 1536.0).collect();
		let entry = Entry::new(large_vector.clone(), "Large embedding".to_string());
		db.insert(&entry).unwrap();

		let retrieved = db.get(entry.id).unwrap();
		assert_eq!(retrieved.meaning.len(), 1536);
	}

	#[test]
	fn test_many_entries() {
		let mut db = ContextDB::in_memory().unwrap();

		for i in 0..100 {
			let entry = Entry::new(vec![i as f32 / 100.0], format!("Entry {}", i));
			db.insert(&entry).unwrap();
		}

		assert_eq!(db.count().unwrap(), 100);

		let query = Query::new().with_limit(10);
		let results = db.query(&query).unwrap();
		assert_eq!(results.len(), 10);
	}
}
