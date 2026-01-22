use contextdb::{ContextDB, Entry, Query, SqliteStorage, StorageBackend, StorageResult};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("=== ContextDB Backend Flexibility Demo ===\n");

	// Example 1: Using SQLite (in-memory)
	println!("1. Creating in-memory SQLite database");
	let mut db_memory = ContextDB::in_memory()?;
	println!("   Backend: {}\n", db_memory.backend_name());

	// Example 2: Using SQLite (file-based)
	println!("2. Creating file-based SQLite database");
	let mut db_file = ContextDB::new("/tmp/contextdb_demo.db")?;
	println!("   Backend: {}\n", db_file.backend_name());

	// Example 3: Using custom backend directly
	println!("3. Creating database with explicit SQLite backend");
	let sqlite_backend = SqliteStorage::in_memory()?;
	let mut db_custom = ContextDB::with_backend(sqlite_backend);
	println!("   Backend: {}\n", db_custom.backend_name());

	// Add some data
	println!("4. Adding entries to in-memory database");
	let entries = vec![
		Entry::new(vec![0.1, 0.2, 0.3], "User prefers TypeScript".to_string()).with_context(
			json!({
				"category": "programming",
				"language": "typescript"
			}),
		),
		Entry::new(
			vec![0.2, 0.3, 0.4],
			"User has 10 years experience".to_string(),
		)
		.with_context(json!({
			"category": "experience",
			"years": 10
		})),
	];

	for entry in &entries {
		db_memory.insert(entry)?;
	}
	println!("   Inserted {} entries", db_memory.count()?);
	println!();

	// Query the data
	println!("5. Querying the database");
	let results = db_memory.query(&Query::new())?;
	for result in results {
		println!("   → {}", result.entry.expression);
	}
	println!();

	// The cool part: same API, different backends
	println!("6. Key benefits of backend abstraction:");
	println!("   ✓ Same API across SQLite, PostgreSQL, MySQL, etc.");
	println!("   ✓ Easy testing with in-memory database");
	println!("   ✓ Production deployment with persistent storage");
	println!("   ✓ Can switch backends without changing application code");
	println!();

	println!("7. Future backends (just implement StorageBackend trait):");
	println!("   • PostgresStorage - for production scale");
	println!("   • MySQLStorage - if you prefer MySQL");
	println!("   • MongoStorage - for document-oriented storage");
	println!("   • TiKVStorage - for distributed deployment");
	println!("   • CustomStorage - roll your own!");

	Ok(())
}

// Example of what a future PostgreSQL backend would look like:
/*
use contextdb::{StorageBackend, StorageResult, Entry, Query, QueryResult};
use uuid::Uuid;

pub struct PostgresStorage {
	pool: sqlx::PgPool,
}

impl PostgresStorage {
	pub async fn new(connection_string: &str) -> StorageResult<Self> {
		let pool = sqlx::PgPool::connect(connection_string)
			.await
			.map_err(|e| StorageError::Database(e.to_string()))?;
		Ok(Self { pool })
	}
}

impl StorageBackend for PostgresStorage {
	fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
		// Implement using PostgreSQL queries with pgvector extension
		todo!()
	}

	fn get(&self, id: Uuid) -> StorageResult<Entry> {
		todo!()
	}

	fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
		// Can use PostgreSQL's native vector operations!
		todo!()
	}

	fn update(&mut self, entry: &Entry) -> StorageResult<()> {
		todo!()
	}

	fn delete(&mut self, id: Uuid) -> StorageResult<()> {
		todo!()
	}

	fn count(&self) -> StorageResult<usize> {
		todo!()
	}

	fn backend_name(&self) -> &str {
		"PostgreSQL"
	}
}

// Then use it like:
let postgres = PostgresStorage::new("postgresql://localhost/mydb").await?;
let db = ContextDB::with_backend(postgres);
*/
