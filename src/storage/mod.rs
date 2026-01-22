use crate::query::{Query, QueryResult};
use crate::types::Entry;
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

	#[error("Storage backend error: {0}")]
	Backend(Box<dyn std::error::Error + Send + Sync>),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Trait that all storage backends must implement
///
/// This allows ContextDB to work with SQLite, PostgreSQL, MySQL, or any other backend
pub trait StorageBackend: Send {
	/// Insert a new entry
	fn insert(&mut self, entry: &Entry) -> StorageResult<()>;

	/// Get an entry by ID
	fn get(&self, id: Uuid) -> StorageResult<Entry>;

	/// Execute a query and return matching entries
	fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>>;

	/// Update an existing entry
	fn update(&mut self, entry: &Entry) -> StorageResult<()>;

	/// Delete an entry by ID
	fn delete(&mut self, id: Uuid) -> StorageResult<()>;

	/// Count total entries
	fn count(&self) -> StorageResult<usize>;

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
