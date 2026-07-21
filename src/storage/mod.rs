use crate::query::{Query, QueryExecution, QueryPlan, QueryResult};
use crate::types::Entry;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum StorageError {
	#[error("Database error: {0}")]
	Database(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Entry not found: {0}")]
	NotFound(Uuid),

	#[error("Invalid vector dimensions")]
	InvalidDimensions,

	#[error("Invalid argument: {0}")]
	InvalidArgument(String),

	#[error("Storage backend error: {0}")]
	Backend(Box<dyn std::error::Error + Send + Sync>),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Result of checking backend data and schema integrity
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IntegrityReport {
	/// Problems found during the check
	pub issues: Vec<IntegrityIssue>,
}

impl IntegrityReport {
	/// Whether no integrity problems were found
	pub fn is_healthy(&self) -> bool {
		self.issues.is_empty()
	}
}

/// A single integrity problem
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrityIssue {
	/// Area in which the problem was found
	pub area: String,
	/// Human-readable problem description
	pub message: String,
}

/// Database-wide identity of the embedding representation
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingProfile {
	/// Provider or model identifier
	pub model: String,
	/// Optional model revision or application-specific version
	pub version: Option<String>,
	/// Required vector dimensions
	pub dimensions: usize,
}

/// Mutation recorded in an entry's durable revision history
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RevisionOperation {
	/// Existing data captured when revision tracking was introduced
	Snapshot,
	/// Entry creation
	Insert,
	/// Entry update
	Update,
	/// Entry deletion
	Delete,
}

/// Immutable snapshot of an entry at a mutation boundary
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EntryRevision {
	/// Unique revision identifier
	pub revision_id: Uuid,
	/// Entry to which the revision belongs
	pub entry_id: Uuid,
	/// Mutation that produced this revision
	pub operation: RevisionOperation,
	/// Complete entry state at that boundary
	pub snapshot: Entry,
	/// Time at which the revision was recorded
	pub recorded_at: chrono::DateTime<chrono::Utc>,
}

/// Trait that all storage backends must implement
///
/// This allows ContextDB to work with SQLite, PostgreSQL, MySQL, or any other backend
pub trait StorageBackend: Send {
	/// Insert a new entry
	fn insert(&mut self, entry: &Entry) -> StorageResult<()>;

	/// Insert entries atomically
	fn insert_batch(&mut self, entries: &[Entry]) -> StorageResult<()>;

	/// Get an entry by ID
	fn get(&self, id: Uuid) -> StorageResult<Entry>;

	/// Execute a query and return matching entries
	fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>>;

	/// Execute a query and return matching entries with top-level provenance
	fn execute(&self, query: &Query) -> StorageResult<QueryExecution> {
		let results = self.query(query)?;
		let plan = results
			.iter()
			.find_map(|result| result.plan.clone())
			.unwrap_or_else(|| QueryPlan::fallback(self.backend_name(), query, results.len()));
		Ok(QueryExecution { results, plan })
	}

	/// Update an existing entry
	fn update(&mut self, entry: &Entry) -> StorageResult<()>;

	/// Update entries atomically
	fn update_batch(&mut self, entries: &[Entry]) -> StorageResult<()>;

	/// Delete an entry by ID
	fn delete(&mut self, id: Uuid) -> StorageResult<()>;

	/// Delete entries atomically
	fn delete_batch(&mut self, ids: &[Uuid]) -> StorageResult<()>;

	/// Count total entries
	fn count(&self) -> StorageResult<usize>;

	/// Check schema and stored-data integrity
	fn integrity_check(&self) -> StorageResult<IntegrityReport>;

	/// Write a consistent backend snapshot to a destination path
	fn backup_to(&self, destination: &Path) -> StorageResult<()>;

	/// Read the configured embedding profile, if one has been set
	fn embedding_profile(&self) -> StorageResult<Option<EmbeddingProfile>>;

	/// Configure the embedding profile, rejecting incompatible stored data
	fn set_embedding_profile(&mut self, profile: &EmbeddingProfile) -> StorageResult<()>;

	/// Attest that legacy, unidentified vectors use this embedding profile
	fn adopt_legacy_embedding_profile(&mut self, _profile: &EmbeddingProfile) -> StorageResult<()> {
		Err(StorageError::Database(
			"Legacy embedding-profile adoption is not supported by this backend".to_string(),
		))
	}

	/// Atomically replace every stored vector and change the embedding profile
	fn migrate_embeddings(
		&mut self,
		_profile: &EmbeddingProfile,
		_replacements: &[(Uuid, Vec<f32>)],
	) -> StorageResult<()> {
		Err(StorageError::Database(
			"Embedding migration is not supported by this backend".to_string(),
		))
	}

	/// Return durable revision history for an entry
	fn revisions(&self, id: Uuid) -> StorageResult<Vec<EntryRevision>>;

	/// Create a selective SQLite-style index for a JSON Pointer context path
	fn create_context_index(&mut self, path: &str) -> StorageResult<String>;

	/// Get backend name for debugging
	fn backend_name(&self) -> &str;
}

// Export concrete implementations
pub mod sqlite;

// Re-export for convenience
pub use sqlite::SqliteStorage;

#[cfg(test)]
mod tests {
	use super::*;

	// ==================== StorageError Tests ====================

	#[test]
	fn test_storage_error_database_display() {
		let error = StorageError::Database("Connection failed".to_string());
		assert_eq!(error.to_string(), "Database error: Connection failed");
	}

	#[test]
	fn test_storage_error_not_found_display() {
		let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
		let error = StorageError::NotFound(id);
		assert_eq!(
			error.to_string(),
			"Entry not found: 550e8400-e29b-41d4-a716-446655440000"
		);
	}

	#[test]
	fn test_storage_error_invalid_dimensions_display() {
		let error = StorageError::InvalidDimensions;
		assert_eq!(error.to_string(), "Invalid vector dimensions");
	}

	#[test]
	fn test_storage_error_invalid_argument_display() {
		let error = StorageError::InvalidArgument("top_k must be greater than zero".to_string());
		assert_eq!(
			error.to_string(),
			"Invalid argument: top_k must be greater than zero"
		);
	}

	#[test]
	fn test_storage_error_serialization_from_json() {
		let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
		let error: StorageError = json_err.into();
		assert!(error.to_string().contains("Serialization error"));
	}

	#[test]
	fn test_storage_error_is_debug() {
		let error = StorageError::Database("test".to_string());
		let debug_str = format!("{:?}", error);
		assert!(debug_str.contains("Database"));
	}

	// ==================== StorageResult Tests ====================

	#[test]
	fn test_storage_result_ok() {
		let result: StorageResult<i32> = Ok(42);
		assert!(result.is_ok());
		assert!(matches!(result, Ok(42)));
	}

	#[test]
	fn test_storage_result_err() {
		let result: StorageResult<i32> = Err(StorageError::InvalidDimensions);
		assert!(result.is_err());
	}

	#[test]
	fn test_storage_result_map() {
		let result: StorageResult<i32> = Ok(42);
		let mapped = result.map(|x| x * 2);
		assert!(matches!(mapped, Ok(84)));
	}

	#[test]
	fn test_storage_result_and_then() {
		let result: StorageResult<i32> = Ok(42);
		let chained = result.and_then(|x| {
			if x > 0 {
				Ok(x * 2)
			} else {
				Err(StorageError::InvalidDimensions)
			}
		});
		assert!(matches!(chained, Ok(84)));
	}

	// ==================== StorageBackend Trait Tests ====================

	// Test that SqliteStorage implements StorageBackend
	#[test]
	fn test_sqlite_implements_storage_backend() {
		fn assert_storage_backend<T: StorageBackend>() {}
		assert_storage_backend::<SqliteStorage>();
	}

	// Test that SqliteStorage is Send (required by trait)
	#[test]
	fn test_sqlite_is_send() {
		fn assert_send<T: Send>() {}
		assert_send::<SqliteStorage>();
	}
}
