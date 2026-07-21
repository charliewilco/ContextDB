use crate::query::{
	ContextFilter, ExpressionFilter, Query, QueryExecution, QueryFilterIdentity, QueryOrder,
	QueryPaginationPlan, QueryPlan, QueryPlanOrdering, QueryPlanStep, QueryPlanStrategy,
	QueryPrimaryOrder, QueryRankingMode, QueryResult, QueryTieBreaker, RelationFilter,
	TemporalFilter,
};
use crate::storage::{
	EmbeddingProfile, EntryRevision, IntegrityIssue, IntegrityReport, RevisionOperation,
	StorageBackend, StorageError, StorageResult,
};
use crate::types::Entry;
use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{params, Connection, OpenFlags, Transaction};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

/// SQLite-backed storage for ContextDB entries
pub struct SqliteStorage {
	conn: Connection,
}

const SCHEMA_VERSION: i64 = 2;

impl SqliteStorage {
	/// Create a new storage instance with an in-memory database
	pub fn in_memory() -> StorageResult<Self> {
		let conn =
			Connection::open_in_memory().map_err(|e| StorageError::Database(e.to_string()))?;
		let mut storage = Self { conn };
		storage.initialize()?;
		Ok(storage)
	}

	/// Create a new storage instance with a file-based database
	pub fn new<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
		let conn = Connection::open(path).map_err(|e| StorageError::Database(e.to_string()))?;
		conn.pragma_update(None, "journal_mode", "WAL")
			.map_err(|error| StorageError::Database(error.to_string()))?;
		conn.pragma_update(None, "synchronous", "NORMAL")
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut storage = Self { conn };
		storage.initialize()?;
		Ok(storage)
	}

	/// Restore a SQLite backup into a destination that does not yet exist.
	pub fn restore_from(backup: &Path, destination: &Path) -> StorageResult<()> {
		if destination.exists() {
			return Err(StorageError::Database(format!(
				"Restore destination already exists: {}",
				destination.display()
			)));
		}
		let source = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut destination_connection = Connection::open(destination)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let backup = rusqlite::backup::Backup::new(&source, &mut destination_connection)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		backup
			.run_to_completion(64, Duration::from_millis(10), None)
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	/// Initialize the database schema
	fn initialize(&mut self) -> StorageResult<()> {
		self.conn
			.execute_batch(
				r#"
			PRAGMA foreign_keys = ON;
			PRAGMA busy_timeout = 5000;

            CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY,
                meaning BLOB NOT NULL,
                expression TEXT NOT NULL,
                context TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS relations (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id),
				CHECK (from_id <> to_id),
				FOREIGN KEY (from_id) REFERENCES entries(id) ON DELETE CASCADE,
				FOREIGN KEY (to_id) REFERENCES entries(id) ON DELETE CASCADE
            );

			CREATE TABLE IF NOT EXISTS contextdb_metadata (
				key TEXT PRIMARY KEY,
				value TEXT NOT NULL
			);

			CREATE TABLE IF NOT EXISTS entry_revisions (
				revision_id TEXT PRIMARY KEY,
				entry_id TEXT NOT NULL,
				operation TEXT NOT NULL,
				snapshot TEXT NOT NULL,
				recorded_at TEXT NOT NULL
			);
			CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry
			ON entry_revisions(entry_id, recorded_at, revision_id);
            
            CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);
            CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(updated_at);
            CREATE INDEX IF NOT EXISTS idx_entries_expression ON entries(expression);
            CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_id);
            CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_id);
            "#,
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let version: i64 = self
			.conn
			.query_row("PRAGMA user_version", [], |row| row.get(0))
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if version > SCHEMA_VERSION {
			return Err(StorageError::Database(format!(
				"Database schema version {version} is newer than supported version {SCHEMA_VERSION}"
			)));
		}
		if version < SCHEMA_VERSION {
			self.migrate_legacy_schema()?;
		}
		self.initialize_search_index()?;
		Ok(())
	}

	fn initialize_search_index(&self) -> StorageResult<()> {
		self.conn
			.execute_batch(
				r#"
			CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(id UNINDEXED, expression);
			CREATE TRIGGER IF NOT EXISTS entries_fts_insert AFTER INSERT ON entries BEGIN
				INSERT INTO entries_fts(rowid, id, expression)
				VALUES (new.rowid, new.id, new.expression);
			END;
			CREATE TRIGGER IF NOT EXISTS entries_fts_update AFTER UPDATE OF expression ON entries BEGIN
				UPDATE entries_fts SET id = new.id, expression = new.expression WHERE rowid = old.rowid;
			END;
			CREATE TRIGGER IF NOT EXISTS entries_fts_delete AFTER DELETE ON entries BEGIN
				DELETE FROM entries_fts WHERE rowid = old.rowid;
			END;
			INSERT INTO entries_fts(rowid, id, expression)
			SELECT entry.rowid, entry.id, entry.expression
			FROM entries AS entry
			WHERE NOT EXISTS (SELECT 1 FROM entries_fts WHERE rowid = entry.rowid);
			"#,
			)
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn migrate_legacy_schema(&mut self) -> StorageResult<()> {
		let dimension = self.validate_existing_vectors()?;
		let existing_entries = self.get_all_entries()?;
		let invalid_relations: i64 = self
			.conn
			.query_row(
				"SELECT COUNT(*)
				 FROM relations AS relation
				 LEFT JOIN entries AS source ON source.id = relation.from_id
				 LEFT JOIN entries AS target ON target.id = relation.to_id
				 WHERE source.id IS NULL OR target.id IS NULL OR relation.from_id = relation.to_id",
				[],
				|row| row.get(0),
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if invalid_relations > 0 {
			return Err(StorageError::Database(format!(
				"Legacy database contains {invalid_relations} invalid relation(s)"
			)));
		}

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		transaction
			.execute_batch(
				r#"
			CREATE TABLE relations_v1 (
				from_id TEXT NOT NULL,
				to_id TEXT NOT NULL,
				PRIMARY KEY (from_id, to_id),
				CHECK (from_id <> to_id),
				FOREIGN KEY (from_id) REFERENCES entries(id) ON DELETE CASCADE,
				FOREIGN KEY (to_id) REFERENCES entries(id) ON DELETE CASCADE
			);
			INSERT INTO relations_v1 (from_id, to_id)
			SELECT from_id, to_id FROM relations;
			DROP TABLE relations;
			ALTER TABLE relations_v1 RENAME TO relations;
			CREATE INDEX idx_relations_from ON relations(from_id);
			CREATE INDEX idx_relations_to ON relations(to_id);
			"#,
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if let Some(dimension) = dimension {
			Self::set_vector_dimension(&transaction, dimension)?;
		}
		for entry in &existing_entries {
			Self::record_revision(&transaction, entry, RevisionOperation::Snapshot)?;
		}
		transaction
			.pragma_update(None, "user_version", SCHEMA_VERSION)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn validate_existing_vectors(&self) -> StorageResult<Option<usize>> {
		let mut statement = self
			.conn
			.prepare("SELECT id, meaning, context, created_at, updated_at FROM entries ORDER BY id")
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let rows = statement
			.query_map([], |row| {
				Ok((
					row.get::<_, String>(0)?,
					row.get::<_, Vec<u8>>(1)?,
					row.get::<_, String>(2)?,
					row.get::<_, String>(3)?,
					row.get::<_, String>(4)?,
				))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut expected_dimension = None;
		for row in rows {
			let (id, bytes, context, created_at, updated_at) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			Uuid::parse_str(&id).map_err(|error| {
				StorageError::Database(format!("Entry has invalid UUID {id}: {error}"))
			})?;
			let vector: Vec<f32> = vector_codec::deserialize(&bytes).map_err(|error| {
				StorageError::Database(format!("Entry {id} has invalid vector: {error}"))
			})?;
			Self::validate_vector(&vector)?;
			serde_json::from_str::<serde_json::Value>(&context).map_err(|error| {
				StorageError::Database(format!("Entry {id} has invalid context JSON: {error}"))
			})?;
			DateTime::parse_from_rfc3339(&created_at).map_err(|error| {
				StorageError::Database(format!("Entry {id} has invalid created_at: {error}"))
			})?;
			DateTime::parse_from_rfc3339(&updated_at).map_err(|error| {
				StorageError::Database(format!("Entry {id} has invalid updated_at: {error}"))
			})?;
			match expected_dimension {
				Some(dimension) if dimension != vector.len() => {
					return Err(StorageError::InvalidDimensions);
				}
				None => expected_dimension = Some(vector.len()),
				_ => {}
			}
		}
		Ok(expected_dimension)
	}

	fn set_vector_dimension(transaction: &Transaction<'_>, dimension: usize) -> StorageResult<()> {
		transaction
			.execute(
				"INSERT INTO contextdb_metadata (key, value) VALUES ('vector_dimension', ?1)
				 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
				params![dimension.to_string()],
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		Ok(())
	}

	fn metadata_value(&self, key: &str) -> StorageResult<Option<String>> {
		match self.conn.query_row(
			"SELECT value FROM contextdb_metadata WHERE key = ?1",
			params![key],
			|row| row.get(0),
		) {
			Ok(value) => Ok(Some(value)),
			Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
			Err(error) => Err(StorageError::Database(error.to_string())),
		}
	}

	fn record_revision(
		transaction: &Transaction<'_>,
		entry: &Entry,
		operation: RevisionOperation,
	) -> StorageResult<()> {
		let operation = match operation {
			RevisionOperation::Snapshot => "snapshot",
			RevisionOperation::Insert => "insert",
			RevisionOperation::Update => "update",
			RevisionOperation::Delete => "delete",
		};
		transaction
			.execute(
				"INSERT INTO entry_revisions
				 (revision_id, entry_id, operation, snapshot, recorded_at)
				 VALUES (?1, ?2, ?3, ?4, ?5)",
				params![
					Uuid::new_v4().to_string(),
					entry.id.to_string(),
					operation,
					serde_json::to_string(entry)?,
					Utc::now().to_rfc3339(),
				],
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		Ok(())
	}

	fn validate_revisions(&self) -> StorageResult<()> {
		let mut statement = self
			.conn
			.prepare(
				"SELECT revision_id, entry_id, operation, snapshot, recorded_at
				 FROM entry_revisions ORDER BY revision_id",
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let rows = statement
			.query_map([], |row| {
				Ok((
					row.get::<_, String>(0)?,
					row.get::<_, String>(1)?,
					row.get::<_, String>(2)?,
					row.get::<_, String>(3)?,
					row.get::<_, String>(4)?,
				))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for row in rows {
			let (revision_id, entry_id, operation, snapshot, recorded_at) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			Uuid::parse_str(&revision_id)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			Uuid::parse_str(&entry_id)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			if !matches!(
				operation.as_str(),
				"snapshot" | "insert" | "update" | "delete"
			) {
				return Err(StorageError::Database(format!(
					"Unknown revision operation: {operation}"
				)));
			}
			serde_json::from_str::<Entry>(&snapshot)?;
			DateTime::parse_from_rfc3339(&recorded_at)
				.map_err(|error| StorageError::Database(error.to_string()))?;
		}
		Ok(())
	}

	fn validate_vector(vector: &[f32]) -> StorageResult<()> {
		if vector.is_empty() || vector.iter().any(|value| !value.is_finite()) {
			return Err(StorageError::InvalidDimensions);
		}
		Ok(())
	}

	fn validate_embedding_profile(profile: &EmbeddingProfile) -> StorageResult<()> {
		if profile.model.trim().is_empty() || profile.dimensions == 0 {
			return Err(StorageError::InvalidDimensions);
		}
		if profile
			.version
			.as_ref()
			.is_some_and(|version| version.trim().is_empty())
		{
			return Err(StorageError::Database(
				"Embedding profile version cannot be empty".to_string(),
			));
		}
		Ok(())
	}

	fn inspect_embedding_metadata(
		&self,
	) -> StorageResult<(Option<EmbeddingProfile>, Option<usize>)> {
		let model = self.metadata_value("embedding_model")?;
		let version = self.metadata_value("embedding_model_version")?;
		let dimensions = self
			.metadata_value("vector_dimension")?
			.map(|value| {
				value.parse::<usize>().map_err(|error| {
					StorageError::Database(format!(
						"Invalid stored vector dimension {value:?}: {error}"
					))
				})
			})
			.transpose()?;
		if dimensions == Some(0) {
			return Err(StorageError::Database(
				"Stored vector dimension must be greater than zero".to_string(),
			));
		}
		if version
			.as_ref()
			.is_some_and(|value| value.trim().is_empty())
		{
			return Err(StorageError::Database(
				"Stored embedding model version is empty".to_string(),
			));
		}
		let Some(model) = model else {
			if version.is_some() {
				return Err(StorageError::Database(
					"Embedding model version is configured without a model".to_string(),
				));
			}
			return Ok((None, dimensions));
		};
		if model.trim().is_empty() {
			return Err(StorageError::Database(
				"Stored embedding model is empty".to_string(),
			));
		}
		let dimensions = dimensions.ok_or_else(|| {
			StorageError::Database(
				"Embedding model is configured without vector dimensions".to_string(),
			)
		})?;
		Ok((
			Some(EmbeddingProfile {
				model,
				version,
				dimensions,
			}),
			Some(dimensions),
		))
	}

	fn write_embedding_profile(
		transaction: &Transaction<'_>,
		profile: &EmbeddingProfile,
	) -> StorageResult<()> {
		Self::set_vector_dimension(transaction, profile.dimensions)?;
		transaction
			.execute(
				"INSERT INTO contextdb_metadata (key, value) VALUES ('embedding_model', ?1)
				 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
				params![&profile.model],
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		match &profile.version {
			Some(version) => {
				transaction
					.execute(
						"INSERT INTO contextdb_metadata (key, value)
						 VALUES ('embedding_model_version', ?1)
						 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
						params![version],
					)
					.map_err(|error| StorageError::Database(error.to_string()))?;
			}
			None => {
				transaction
					.execute(
						"DELETE FROM contextdb_metadata WHERE key = 'embedding_model_version'",
						[],
					)
					.map_err(|error| StorageError::Database(error.to_string()))?;
			}
		}
		Ok(())
	}

	fn stored_vector_dimension(&self) -> StorageResult<Option<usize>> {
		let metadata_result = self.conn.query_row(
			"SELECT value FROM contextdb_metadata WHERE key = 'vector_dimension'",
			[],
			|row| row.get::<_, String>(0),
		);
		match metadata_result {
			Ok(value) => {
				let dimension = value.parse::<usize>().map_err(|error| {
					StorageError::Database(format!("Invalid stored vector dimension: {error}"))
				})?;
				if dimension == 0 {
					return Err(StorageError::Database(
						"Stored vector dimension must be greater than zero".to_string(),
					));
				}
				return Ok(Some(dimension));
			}
			Err(rusqlite::Error::QueryReturnedNoRows) => {}
			Err(error) => return Err(StorageError::Database(error.to_string())),
		}

		let result = self.conn.query_row(
			"SELECT meaning FROM entries ORDER BY id LIMIT 1",
			[],
			|row| row.get::<_, Vec<u8>>(0),
		);

		match result {
			Ok(bytes) => {
				let vector: Vec<f32> = vector_codec::deserialize(&bytes).map_err(|error| {
					StorageError::Database(format!("Failed to deserialize vector: {error}"))
				})?;
				Self::validate_vector(&vector)?;
				Ok(Some(vector.len()))
			}
			Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
			Err(error) => Err(StorageError::Database(error.to_string())),
		}
	}

	fn validate_entry(&self, entry: &Entry) -> StorageResult<()> {
		Self::validate_vector(&entry.meaning)?;
		if self
			.stored_vector_dimension()?
			.is_some_and(|dimension| dimension != entry.meaning.len())
		{
			return Err(StorageError::InvalidDimensions);
		}
		if entry.relations.contains(&entry.id) {
			return Err(StorageError::Database(
				"An entry cannot relate to itself".to_string(),
			));
		}
		if entry
			.relations
			.iter()
			.copied()
			.collect::<HashSet<_>>()
			.len() != entry.relations.len()
		{
			return Err(StorageError::Database(
				"Entry contains duplicate relation IDs".to_string(),
			));
		}
		if entry.updated_at < entry.created_at {
			return Err(StorageError::Database(
				"updated_at cannot be earlier than created_at".to_string(),
			));
		}
		Ok(())
	}

	fn validate_query(&self, query: &Query) -> StorageResult<()> {
		if query.cursor.is_some() && query.offset > 0 {
			return Err(StorageError::Database(
				"A query cannot use both cursor and offset pagination".to_string(),
			));
		}
		if let Some(meaning) = &query.meaning {
			Self::validate_vector(&meaning.vector)?;
			if self
				.stored_vector_dimension()?
				.is_some_and(|dimension| dimension != meaning.vector.len())
			{
				return Err(StorageError::InvalidDimensions);
			}
			if meaning.threshold.is_some_and(|threshold| {
				!threshold.is_finite() || !(0.0..=1.0).contains(&threshold)
			}) {
				return Err(StorageError::Database(
					"Similarity threshold must be finite and between 0 and 1".to_string(),
				));
			}
			if meaning.top_k == Some(0) {
				return Err(StorageError::Database(
					"top_k must be greater than zero".to_string(),
				));
			}
		}
		if let Some(weights) = query.hybrid_weights {
			if !weights.semantic.is_finite()
				|| !weights.lexical.is_finite()
				|| weights.semantic < 0.0
				|| weights.lexical < 0.0
				|| weights.semantic + weights.lexical == 0.0
			{
				return Err(StorageError::Database(
					"Hybrid weights must be finite, non-negative, and have a positive sum"
						.to_string(),
				));
			}
			if query.meaning.is_none()
				|| !matches!(query.expression, Some(ExpressionFilter::FullText(_)))
			{
				return Err(StorageError::Database(
					"Hybrid weights require both meaning and full-text filters".to_string(),
				));
			}
		}
		if matches!(
			query.temporal,
			Some(TemporalFilter::CreatedBetween(start, end)) if start >= end
		) {
			return Err(StorageError::Database(
				"CreatedBetween start must be before end".to_string(),
			));
		}
		Ok(())
	}

	fn validate_relation_targets(&self, entry: &Entry) -> StorageResult<()> {
		for relation_id in &entry.relations {
			let exists: bool = self
				.conn
				.query_row(
					"SELECT EXISTS(SELECT 1 FROM entries WHERE id = ?1)",
					params![relation_id.to_string()],
					|row| row.get(0),
				)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			if !exists {
				return Err(StorageError::NotFound(*relation_id));
			}
		}
		Ok(())
	}

	/// Get all entries from the database
	fn get_all_entries(&self) -> StorageResult<Vec<Entry>> {
		let mut stmt = self
			.conn
			.prepare(
				"SELECT id, meaning, expression, context, created_at, updated_at
				 FROM entries ORDER BY id",
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let rows = stmt
			.query_map([], |row| {
				Ok((
					row.get::<_, String>(0)?,
					row.get::<_, Vec<u8>>(1)?,
					row.get::<_, String>(2)?,
					row.get::<_, String>(3)?,
					row.get::<_, String>(4)?,
					row.get::<_, String>(5)?,
				))
			})
			.map_err(|e| StorageError::Database(e.to_string()))?;
		let mut entries = Vec::new();
		for row in rows {
			let (id, meaning, expression, context, created_at, updated_at) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			let id = Uuid::parse_str(&id)
				.map_err(|error| StorageError::Database(format!("Invalid entry UUID: {error}")))?;
			entries.push(Entry {
				id,
				meaning: vector_codec::deserialize(&meaning).map_err(|error| {
					StorageError::Database(format!("Entry {id} has invalid vector: {error}"))
				})?,
				expression,
				context: serde_json::from_str(&context)?,
				created_at: DateTime::parse_from_rfc3339(&created_at)
					.map_err(|error| StorageError::Database(error.to_string()))?
					.with_timezone(&Utc),
				updated_at: DateTime::parse_from_rfc3339(&updated_at)
					.map_err(|error| StorageError::Database(error.to_string()))?
					.with_timezone(&Utc),
				relations: Vec::new(),
			});
		}

		let mut relation_statement = self
			.conn
			.prepare("SELECT from_id, to_id FROM relations ORDER BY from_id, to_id")
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let relation_rows = relation_statement
			.query_map([], |row| {
				Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut relations: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
		for row in relation_rows {
			let (from_id, to_id) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			let from_id = Uuid::parse_str(&from_id)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			let to_id = Uuid::parse_str(&to_id)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			relations.entry(from_id).or_default().push(to_id);
		}
		for entry in &mut entries {
			entry.relations = relations.remove(&entry.id).unwrap_or_default();
		}
		Ok(entries)
	}

	fn matches_expression(
		&self,
		expression: &str,
		filter: &ExpressionFilter,
	) -> StorageResult<bool> {
		match filter {
			ExpressionFilter::Equals(s) => Ok(expression == s),
			ExpressionFilter::Contains(s) => {
				Ok(expression.to_lowercase().contains(&s.to_lowercase()))
			}
			ExpressionFilter::StartsWith(s) => Ok(expression.starts_with(s)),
			ExpressionFilter::Matches(pattern) => {
				let regex = Regex::new(pattern)
					.map_err(|e| StorageError::Database(format!("Invalid regex: {}", e)))?;
				Ok(regex.is_match(expression))
			}
			ExpressionFilter::FullText(_) => Ok(true),
		}
	}

	fn matches_context(&self, context: &serde_json::Value, filter: &ContextFilter) -> bool {
		match filter {
			ContextFilter::PathExists(path) => {
				// Simple path checking - in production use jsonpath
				context.pointer(path).is_some()
			}
			ContextFilter::PathEquals(path, value) => context.pointer(path) == Some(value),
			ContextFilter::PathContains(path, value) => {
				if let Some(arr) = context.pointer(path).and_then(|v| v.as_array()) {
					arr.contains(value)
				} else {
					false
				}
			}
			ContextFilter::And(filters) => filters.iter().all(|f| self.matches_context(context, f)),
			ContextFilter::Or(filters) => filters.iter().any(|f| self.matches_context(context, f)),
		}
	}

	fn matches_temporal(&self, entry: &Entry, filter: &TemporalFilter) -> bool {
		match filter {
			TemporalFilter::CreatedAfter(dt) => entry.created_at > *dt,
			TemporalFilter::CreatedBefore(dt) => entry.created_at < *dt,
			TemporalFilter::CreatedBetween(start, end) => {
				entry.created_at > *start && entry.created_at < *end
			}
			TemporalFilter::UpdatedAfter(dt) => entry.updated_at > *dt,
			TemporalFilter::UpdatedBefore(dt) => entry.updated_at < *dt,
		}
	}

	fn load_relation_index(&self) -> StorageResult<RelationIndex> {
		let mut stmt = self
			.conn
			.prepare("SELECT from_id, to_id FROM relations")
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
		let mut related_ids: HashSet<Uuid> = HashSet::new();

		let rows = stmt
			.query_map([], |row| {
				let from_id_str: String = row.get(0)?;
				let to_id_str: String = row.get(1)?;
				let from_id =
					Uuid::parse_str(&from_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
				let to_id =
					Uuid::parse_str(&to_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
				Ok((from_id, to_id))
			})
			.map_err(|e| StorageError::Database(e.to_string()))?;

		for row in rows {
			let (from_id, to_id) = row.map_err(|e| StorageError::Database(e.to_string()))?;
			adjacency.entry(from_id).or_default().push(to_id);
			related_ids.insert(from_id);
		}

		Ok(RelationIndex {
			adjacency,
			related_ids,
		})
	}

	fn direct_relations(&self, index: &RelationIndex, id: Uuid) -> HashSet<Uuid> {
		index
			.adjacency
			.get(&id)
			.map(|ids| ids.iter().copied().collect())
			.unwrap_or_default()
	}

	fn within_distance_relations(
		&self,
		index: &RelationIndex,
		from: Uuid,
		max_hops: usize,
	) -> HashSet<Uuid> {
		if max_hops == 0 {
			return HashSet::new();
		}

		let mut visited: HashSet<Uuid> = HashSet::new();
		let mut results: HashSet<Uuid> = HashSet::new();
		let mut queue: VecDeque<(Uuid, usize)> = VecDeque::new();

		visited.insert(from);
		queue.push_back((from, 0));

		while let Some((current, hops)) = queue.pop_front() {
			if hops >= max_hops {
				continue;
			}

			if let Some(neighbors) = index.adjacency.get(&current) {
				for &neighbor in neighbors {
					if visited.insert(neighbor) {
						let next_hops = hops + 1;
						results.insert(neighbor);
						queue.push_back((neighbor, next_hops));
					}
				}
			}
		}

		results
	}

	fn generate_explanation(
		&self,
		_entry: &Entry,
		query: &Query,
		similarity_score: Option<f32>,
		lexical_score: Option<f32>,
		combined_score: Option<f32>,
	) -> String {
		let mut parts = vec!["Plan: SQLite candidate filtering".to_string()];

		if let Some(score) = similarity_score {
			parts.push(format!("Semantic similarity: {:.2}%", score * 100.0));
		}
		if let Some(score) = lexical_score {
			parts.push(format!("Normalized BM25 relevance: {:.2}%", score * 100.0));
		}
		if let Some(score) = combined_score {
			parts.push(format!("Combined relevance: {:.2}%", score * 100.0));
		}

		if query.expression.is_some() {
			parts.push("Matched expression filter".to_string());
		}

		if query.context.is_some() {
			parts.push("Matched context filter".to_string());
		}

		if query.temporal.is_some() {
			parts.push("Matched temporal filter".to_string());
		}

		if query.relations.is_some() {
			parts.push("Matched relation filter".to_string());
		}

		parts.join(", ")
	}

	fn get_entry_ids(&self) -> StorageResult<HashSet<Uuid>> {
		let mut stmt = self
			.conn
			.prepare("SELECT id FROM entries")
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let rows = stmt
			.query_map([], |row| {
				let id_str: String = row.get(0)?;
				Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)
			})
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let mut ids = HashSet::new();
		for row in rows {
			let id = row.map_err(|e| StorageError::Database(e.to_string()))?;
			ids.insert(id);
		}

		Ok(ids)
	}

	#[allow(clippy::manual_repeat_n)]
	fn get_entries_by_ids(&self, ids: &HashSet<Uuid>) -> StorageResult<Vec<Entry>> {
		if ids.is_empty() {
			return Ok(Vec::new());
		}
		let mut id_values: Vec<String> = ids.iter().map(Uuid::to_string).collect();
		id_values.sort();
		let mut entries = Vec::with_capacity(ids.len());
		for id_chunk in id_values.chunks(900) {
			let placeholders = std::iter::repeat("?")
				.take(id_chunk.len())
				.collect::<Vec<_>>()
				.join(",");
			let mut statement = self
				.conn
				.prepare(&format!(
					"SELECT id, meaning, expression, context, created_at, updated_at
					 FROM entries WHERE id IN ({placeholders}) ORDER BY id"
				))
				.map_err(|error| StorageError::Database(error.to_string()))?;
			let rows = statement
				.query_map(rusqlite::params_from_iter(id_chunk), |row| {
					Ok((
						row.get::<_, String>(0)?,
						row.get::<_, Vec<u8>>(1)?,
						row.get::<_, String>(2)?,
						row.get::<_, String>(3)?,
						row.get::<_, String>(4)?,
						row.get::<_, String>(5)?,
					))
				})
				.map_err(|error| StorageError::Database(error.to_string()))?;
			for row in rows {
				let (id, meaning, expression, context, created_at, updated_at) =
					row.map_err(|error| StorageError::Database(error.to_string()))?;
				let id = Uuid::parse_str(&id)
					.map_err(|error| StorageError::Database(error.to_string()))?;
				entries.push(Entry {
					id,
					meaning: vector_codec::deserialize(&meaning).map_err(|error| {
						StorageError::Database(format!("Entry {id} has invalid vector: {error}"))
					})?,
					expression,
					context: serde_json::from_str(&context)?,
					created_at: DateTime::parse_from_rfc3339(&created_at)
						.map_err(|error| StorageError::Database(error.to_string()))?
						.with_timezone(&Utc),
					updated_at: DateTime::parse_from_rfc3339(&updated_at)
						.map_err(|error| StorageError::Database(error.to_string()))?
						.with_timezone(&Utc),
					relations: Vec::new(),
				});
			}
		}

		let mut relations: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
		for id_chunk in id_values.chunks(900) {
			let placeholders = std::iter::repeat("?")
				.take(id_chunk.len())
				.collect::<Vec<_>>()
				.join(",");
			let mut relation_statement = self
				.conn
				.prepare(&format!(
					"SELECT from_id, to_id FROM relations
					 WHERE from_id IN ({placeholders}) ORDER BY from_id, to_id"
				))
				.map_err(|error| StorageError::Database(error.to_string()))?;
			let relation_rows = relation_statement
				.query_map(rusqlite::params_from_iter(id_chunk), |row| {
					Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
				})
				.map_err(|error| StorageError::Database(error.to_string()))?;
			for row in relation_rows {
				let (from_id, to_id) =
					row.map_err(|error| StorageError::Database(error.to_string()))?;
				let from_id = Uuid::parse_str(&from_id)
					.map_err(|error| StorageError::Database(error.to_string()))?;
				let to_id = Uuid::parse_str(&to_id)
					.map_err(|error| StorageError::Database(error.to_string()))?;
				relations.entry(from_id).or_default().push(to_id);
			}
		}
		for entry in &mut entries {
			entry.relations = relations.remove(&entry.id).unwrap_or_default();
		}
		Ok(entries)
	}

	fn query_expression_ids(&self, filter: &ExpressionFilter) -> StorageResult<HashSet<Uuid>> {
		match filter {
			ExpressionFilter::Equals(value) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE expression = ?1",
				rusqlite::params![value],
			),
			ExpressionFilter::Contains(value) => {
				let lowered = value.to_lowercase();
				self.query_ids_with_params(
					"SELECT id FROM entries WHERE INSTR(LOWER(expression), ?1) > 0",
					rusqlite::params![lowered],
				)
			}
			ExpressionFilter::StartsWith(value) => {
				let prefix_len = value.chars().count() as i64;
				self.query_ids_with_params(
					"SELECT id FROM entries WHERE SUBSTR(expression, 1, ?2) = ?1",
					rusqlite::params![value, prefix_len],
				)
			}
			ExpressionFilter::Matches(value) => {
				let _ = Regex::new(value)
					.map_err(|e| StorageError::Database(format!("Invalid regex: {}", e)))?;
				self.get_entry_ids()
			}
			ExpressionFilter::FullText(value) => {
				Ok(self.full_text_scores(value)?.into_keys().collect())
			}
		}
	}

	fn full_text_scores(&self, query: &str) -> StorageResult<HashMap<Uuid, f32>> {
		let mut statement = self
			.conn
			.prepare(
				"SELECT id, bm25(entries_fts) AS score
				 FROM entries_fts WHERE entries_fts MATCH ?1 ORDER BY score, id",
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let rows = statement
			.query_map(params![query], |row| {
				Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut raw_scores = Vec::new();
		for row in rows {
			let (id, score) = row.map_err(|error| StorageError::Database(error.to_string()))?;
			let id =
				Uuid::parse_str(&id).map_err(|error| StorageError::Database(error.to_string()))?;
			raw_scores.push((id, -score as f32));
		}
		let min = raw_scores
			.iter()
			.map(|(_, score)| *score)
			.reduce(f32::min)
			.unwrap_or(0.0);
		let max = raw_scores
			.iter()
			.map(|(_, score)| *score)
			.reduce(f32::max)
			.unwrap_or(0.0);
		Ok(raw_scores
			.into_iter()
			.map(|(id, score)| {
				let normalized = if max > min {
					(score - min) / (max - min)
				} else {
					1.0
				};
				(id, normalized)
			})
			.collect())
	}

	fn query_temporal_ids(&self, filter: &TemporalFilter) -> StorageResult<HashSet<Uuid>> {
		match filter {
			TemporalFilter::CreatedAfter(dt) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE created_at > ?1",
				rusqlite::params![dt.to_rfc3339()],
			),
			TemporalFilter::CreatedBefore(dt) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE created_at < ?1",
				rusqlite::params![dt.to_rfc3339()],
			),
			TemporalFilter::CreatedBetween(start, end) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE created_at > ?1 AND created_at < ?2",
				rusqlite::params![start.to_rfc3339(), end.to_rfc3339()],
			),
			TemporalFilter::UpdatedAfter(dt) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE updated_at > ?1",
				rusqlite::params![dt.to_rfc3339()],
			),
			TemporalFilter::UpdatedBefore(dt) => self.query_ids_with_params(
				"SELECT id FROM entries WHERE updated_at < ?1",
				rusqlite::params![dt.to_rfc3339()],
			),
		}
	}

	fn query_context_ids(&self, filter: &ContextFilter) -> StorageResult<HashSet<Uuid>> {
		match filter {
			ContextFilter::PathExists(path) => {
				let path = Self::sql_string_literal(&Self::json_pointer_to_sqlite_path(path)?);
				self.query_ids_with_params(
					&format!("SELECT id FROM entries WHERE json_type(context, {path}) IS NOT NULL"),
					[],
				)
			}
			ContextFilter::PathEquals(path, value) => {
				let path = Self::sql_string_literal(&Self::json_pointer_to_sqlite_path(path)?);
				self.query_ids_with_params(
					&format!(
						"SELECT id FROM entries
						 WHERE json_extract(context, {path}) = json_extract(?1, '$')"
					),
					params![serde_json::to_string(value)?],
				)
			}
			ContextFilter::PathContains(path, value) => self.query_ids_with_params(
				"SELECT entry.id FROM entries AS entry
				 WHERE EXISTS (
					SELECT 1 FROM json_each(entry.context, ?1)
					WHERE json_each.value = json_extract(?2, '$')
				 )",
				params![
					Self::json_pointer_to_sqlite_path(path)?,
					serde_json::to_string(value)?
				],
			),
			ContextFilter::And(filters) => {
				let mut ids = self.get_entry_ids()?;
				for filter in filters {
					let matching = self.query_context_ids(filter)?;
					ids = ids.intersection(&matching).copied().collect();
				}
				Ok(ids)
			}
			ContextFilter::Or(filters) => {
				let mut ids = HashSet::new();
				for filter in filters {
					ids.extend(self.query_context_ids(filter)?);
				}
				Ok(ids)
			}
		}
	}

	fn json_pointer_to_sqlite_path(pointer: &str) -> StorageResult<String> {
		if pointer.is_empty() {
			return Ok("$".to_string());
		}
		if !pointer.starts_with('/') {
			return Err(StorageError::Database(format!(
				"Invalid JSON Pointer: {pointer}"
			)));
		}
		let mut path = "$".to_string();
		for raw_segment in pointer[1..].split('/') {
			let segment = raw_segment.replace("~1", "/").replace("~0", "~");
			if let Ok(index) = segment.parse::<usize>() {
				path.push_str(&format!("[{index}]"));
			} else {
				path.push('.');
				path.push_str(&serde_json::to_string(&segment)?);
			}
		}
		Ok(path)
	}

	fn sql_string_literal(value: &str) -> String {
		format!("'{}'", value.replace('\'', "''"))
	}

	fn query_relation_ids(&self, filter: &RelationFilter) -> StorageResult<HashSet<Uuid>> {
		match filter {
			RelationFilter::DirectlyRelatedTo(id) => {
				let id_str = id.to_string();
				self.query_ids_with_params(
					"SELECT to_id AS id FROM relations WHERE from_id = ?1",
					rusqlite::params![id_str],
				)
			}
			RelationFilter::WithinDistance { from, max_hops } => {
				let index = self.load_relation_index()?;
				Ok(self.within_distance_relations(&index, *from, *max_hops))
			}
			RelationFilter::HasRelations => self.query_ids_with_params(
				"SELECT DISTINCT from_id AS id FROM relations",
				rusqlite::params![],
			),
			RelationFilter::NoRelations => {
				let all_ids = self.get_entry_ids()?;
				let related_ids = self.query_relation_ids(&RelationFilter::HasRelations)?;
				Ok(all_ids
					.difference(&related_ids)
					.copied()
					.collect::<HashSet<_>>())
			}
		}
	}

	fn query_ids_with_params<P>(&self, sql: &str, params: P) -> StorageResult<HashSet<Uuid>>
	where
		P: rusqlite::Params,
	{
		let mut stmt = self
			.conn
			.prepare(sql)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let rows = stmt
			.query_map(params, |row| {
				let id_str: String = row.get(0)?;
				Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)
			})
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let mut ids = HashSet::new();
		for row in rows {
			let id = row.map_err(|e| StorageError::Database(e.to_string()))?;
			ids.insert(id);
		}

		Ok(ids)
	}

	fn intersect_candidate_ids(
		candidate_ids: &mut Option<HashSet<Uuid>>,
		ids: HashSet<Uuid>,
		total_entries: usize,
	) -> (usize, usize) {
		let before = candidate_ids.as_ref().map_or(total_entries, HashSet::len);
		let narrowed = match candidate_ids.take() {
			Some(existing) => existing.intersection(&ids).copied().collect(),
			None => ids,
		};
		let after = narrowed.len();
		*candidate_ids = Some(narrowed);
		(before, after)
	}
}

impl StorageBackend for SqliteStorage {
	fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
		self.validate_entry(entry)?;
		self.validate_relation_targets(entry)?;
		let establishes_dimension = self.stored_vector_dimension()?.is_none();
		let id = entry.id.to_string();
		let meaning_bytes = vector_codec::serialize(&entry.meaning)
			.map_err(|e| StorageError::Database(format!("Failed to serialize vector: {}", e)))?;
		let context_json = serde_json::to_string(&entry.context)?;

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;

		transaction
			.execute(
				"INSERT INTO entries (id, meaning, expression, context, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
				params![
					id,
					meaning_bytes,
					&entry.expression,
					context_json,
					entry.created_at.to_rfc3339(),
					entry.updated_at.to_rfc3339(),
				],
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		// Insert relations
		for relation_id in &entry.relations {
			transaction
				.execute(
					"INSERT OR IGNORE INTO relations (from_id, to_id) VALUES (?1, ?2)",
					params![id, relation_id.to_string()],
				)
				.map_err(|e| StorageError::Database(e.to_string()))?;
		}
		Self::record_revision(&transaction, entry, RevisionOperation::Insert)?;
		if establishes_dimension {
			Self::set_vector_dimension(&transaction, entry.meaning.len())?;
		}

		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn insert_batch(&mut self, entries: &[Entry]) -> StorageResult<()> {
		if entries.is_empty() {
			return Ok(());
		}

		let stored_dimension = self.stored_vector_dimension()?;
		let expected_dimension = stored_dimension.unwrap_or(entries[0].meaning.len());
		let mut batch_ids = HashSet::with_capacity(entries.len());
		for entry in entries {
			self.validate_entry(entry)?;
			if entry.meaning.len() != expected_dimension {
				return Err(StorageError::InvalidDimensions);
			}
			if !batch_ids.insert(entry.id) {
				return Err(StorageError::Database(format!(
					"Duplicate entry ID in batch: {}",
					entry.id
				)));
			}
		}

		let existing_ids = self.get_entry_ids()?;
		for entry in entries {
			for relation_id in &entry.relations {
				if !existing_ids.contains(relation_id) && !batch_ids.contains(relation_id) {
					return Err(StorageError::NotFound(*relation_id));
				}
			}
		}

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for entry in entries {
			let meaning_bytes = vector_codec::serialize(&entry.meaning).map_err(|error| {
				StorageError::Database(format!("Failed to serialize vector: {error}"))
			})?;
			let context_json = serde_json::to_string(&entry.context)?;
			transaction
				.execute(
					"INSERT INTO entries (id, meaning, expression, context, created_at, updated_at)
					 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
					params![
						entry.id.to_string(),
						meaning_bytes,
						&entry.expression,
						context_json,
						entry.created_at.to_rfc3339(),
						entry.updated_at.to_rfc3339(),
					],
				)
				.map_err(|error| StorageError::Database(error.to_string()))?;
		}
		for entry in entries {
			for relation_id in &entry.relations {
				transaction
					.execute(
						"INSERT INTO relations (from_id, to_id) VALUES (?1, ?2)",
						params![entry.id.to_string(), relation_id.to_string()],
					)
					.map_err(|error| StorageError::Database(error.to_string()))?;
			}
			Self::record_revision(&transaction, entry, RevisionOperation::Insert)?;
		}
		if stored_dimension.is_none() {
			Self::set_vector_dimension(&transaction, expected_dimension)?;
		}
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn get(&self, id: Uuid) -> StorageResult<Entry> {
		let id_str = id.to_string();

		let mut stmt = self
			.conn
			.prepare(
				"SELECT id, meaning, expression, context, created_at, updated_at
             FROM entries WHERE id = ?1",
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let entry = stmt
			.query_row(params![id_str], |row| {
				let meaning_bytes: Vec<u8> = row.get(1)?;
				let meaning: Vec<f32> = vector_codec::deserialize(&meaning_bytes)
					.map_err(|_| rusqlite::Error::InvalidQuery)?;

				let context_json: String = row.get(3)?;
				let context: serde_json::Value = serde_json::from_str(&context_json)
					.map_err(|_| rusqlite::Error::InvalidQuery)?;

				let created_at_str: String = row.get(4)?;
				let created_at = DateTime::parse_from_rfc3339(&created_at_str)
					.map_err(|_| rusqlite::Error::InvalidQuery)?
					.with_timezone(&Utc);

				let updated_at_str: String = row.get(5)?;
				let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
					.map_err(|_| rusqlite::Error::InvalidQuery)?
					.with_timezone(&Utc);

				Ok(Entry {
					id,
					meaning,
					expression: row.get(2)?,
					context,
					created_at,
					updated_at,
					relations: Vec::new(), // Will be filled below
				})
			})
			.map_err(|error| match error {
				rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound(id),
				other => StorageError::Database(other.to_string()),
			})?;

		// Get relations
		let mut rel_stmt = self
			.conn
			.prepare("SELECT to_id FROM relations WHERE from_id = ?1 ORDER BY to_id")
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let relation_rows = rel_stmt
			.query_map(params![id_str], |row| {
				let to_id_str: String = row.get(0)?;
				Uuid::parse_str(&to_id_str).map_err(|_| rusqlite::Error::InvalidQuery)
			})
			.map_err(|e| StorageError::Database(e.to_string()))?;
		let mut relations = Vec::new();
		for row in relation_rows {
			relations.push(row.map_err(|error| StorageError::Database(error.to_string()))?);
		}

		Ok(Entry { relations, ..entry })
	}

	fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
		self.execute(query).map(|execution| execution.results)
	}

	fn execute(&self, query: &Query) -> StorageResult<QueryExecution> {
		self.validate_query(query)?;
		let lexical_scores = match &query.expression {
			Some(ExpressionFilter::FullText(value)) => self.full_text_scores(value)?,
			_ => HashMap::new(),
		};
		let total_entries = self.count()?;
		let mut candidate_ids: Option<HashSet<Uuid>> = None;
		let mut steps = Vec::new();
		let mut candidate_filters = Vec::new();

		if let Some(ref expr_filter) = query.expression {
			let ids = self.query_expression_ids(expr_filter)?;
			candidate_filters.push("expression".to_string());
			if !matches!(expr_filter, ExpressionFilter::Matches(_)) {
				let (before, after) =
					Self::intersect_candidate_ids(&mut candidate_ids, ids, total_entries);
				let (strategy, filter) = match expr_filter {
					ExpressionFilter::Equals(_) => (
						QueryPlanStrategy::SqlPredicate,
						QueryFilterIdentity::ExpressionEquals,
					),
					ExpressionFilter::Contains(_) => (
						QueryPlanStrategy::SqlPredicate,
						QueryFilterIdentity::ExpressionContains,
					),
					ExpressionFilter::StartsWith(_) => (
						QueryPlanStrategy::SqlPredicate,
						QueryFilterIdentity::ExpressionStartsWith,
					),
					ExpressionFilter::FullText(_) => (
						QueryPlanStrategy::Fts5,
						QueryFilterIdentity::ExpressionFullText,
					),
					ExpressionFilter::Matches(_) => unreachable!(),
				};
				steps.push(QueryPlanStep {
					strategy,
					filter: Some(filter),
					candidates_before: before,
					candidates_after: after,
				});
			}
		}

		if let Some(ref context_filter) = query.context {
			let ids = self.query_context_ids(context_filter)?;
			candidate_filters.push("context".to_string());
			let (before, after) =
				Self::intersect_candidate_ids(&mut candidate_ids, ids, total_entries);
			steps.push(QueryPlanStep {
				strategy: QueryPlanStrategy::JsonPredicate,
				filter: Some(QueryFilterIdentity::Context),
				candidates_before: before,
				candidates_after: after,
			});
		}

		if let Some(ref temporal_filter) = query.temporal {
			let ids = self.query_temporal_ids(temporal_filter)?;
			candidate_filters.push("temporal".to_string());
			let (before, after) =
				Self::intersect_candidate_ids(&mut candidate_ids, ids, total_entries);
			steps.push(QueryPlanStep {
				strategy: QueryPlanStrategy::SqlPredicate,
				filter: Some(QueryFilterIdentity::Temporal),
				candidates_before: before,
				candidates_after: after,
			});
		}

		if let Some(ref relation_filter) = query.relations {
			let ids = self.query_relation_ids(relation_filter)?;
			candidate_filters.push("relations".to_string());
			let (before, after) =
				Self::intersect_candidate_ids(&mut candidate_ids, ids, total_entries);
			steps.push(QueryPlanStep {
				strategy: QueryPlanStrategy::GraphTraversal,
				filter: Some(QueryFilterIdentity::Relations),
				candidates_before: before,
				candidates_after: after,
			});
		}

		// Start with filtered entries if possible
		let mut results = match candidate_ids {
			Some(ref ids) => self.get_entries_by_ids(ids)?,
			None => self.get_all_entries()?,
		};
		let candidates_loaded = results.len();

		let relation_index = if query.relations.is_some() {
			Some(self.load_relation_index()?)
		} else {
			None
		};

		// Context is not SQL-indexed yet, so narrow candidates before ranking.
		if let Some(ref ctx_filter) = query.context {
			results.retain(|entry| self.matches_context(&entry.context, ctx_filter));
		}

		// Apply semantic filter (vector similarity)
		if let Some(ref meaning_filter) = query.meaning {
			let before = results.len();
			let weights = query.hybrid_weights.unwrap_or(crate::query::HybridWeights {
				semantic: 1.0,
				lexical: 1.0,
			});
			let weight_sum = weights.semantic + weights.lexical;
			results.sort_by(|a, b| {
				let sim_a = crate::types::cosine_similarity(&a.meaning, &meaning_filter.vector);
				let sim_b = crate::types::cosine_similarity(&b.meaning, &meaning_filter.vector);
				let score_a = lexical_scores.get(&a.id).map_or(sim_a, |lexical| {
					(weights.semantic * ((sim_a + 1.0) / 2.0) + weights.lexical * lexical)
						/ weight_sum
				});
				let score_b = lexical_scores.get(&b.id).map_or(sim_b, |lexical| {
					(weights.semantic * ((sim_b + 1.0) / 2.0) + weights.lexical * lexical)
						/ weight_sum
				});
				score_b.total_cmp(&score_a).then_with(|| a.id.cmp(&b.id))
			});

			if let Some(threshold) = meaning_filter.threshold {
				results.retain(|e| {
					crate::types::cosine_similarity(&e.meaning, &meaning_filter.vector) >= threshold
				});
			}
			steps.push(QueryPlanStep {
				strategy: QueryPlanStrategy::LinearVectorScan,
				filter: Some(QueryFilterIdentity::Meaning),
				candidates_before: before,
				candidates_after: results.len(),
			});
		}

		// Apply expression filter
		if let Some(ref expr_filter) = query.expression {
			let before = results.len();
			let mut filtered = Vec::with_capacity(results.len());
			for entry in results {
				if self.matches_expression(&entry.expression, expr_filter)? {
					filtered.push(entry);
				}
			}
			results = filtered;
			if matches!(expr_filter, ExpressionFilter::Matches(_)) {
				steps.push(QueryPlanStep {
					strategy: QueryPlanStrategy::RustRegexScan,
					filter: Some(QueryFilterIdentity::ExpressionRegex),
					candidates_before: before,
					candidates_after: results.len(),
				});
			}
		}

		// Apply temporal filter
		if let Some(ref temporal_filter) = query.temporal {
			results.retain(|e| self.matches_temporal(e, temporal_filter));
		}

		// Apply relation filter
		if let Some(ref relation_filter) = query.relations {
			let index = relation_index
				.as_ref()
				.expect("relation index must be initialized when relations filter is set");
			match relation_filter {
				RelationFilter::DirectlyRelatedTo(id) => {
					let related = self.direct_relations(index, *id);
					results.retain(|e| related.contains(&e.id));
				}
				RelationFilter::WithinDistance { from, max_hops } => {
					let related = self.within_distance_relations(index, *from, *max_hops);
					results.retain(|e| related.contains(&e.id));
				}
				RelationFilter::HasRelations => {
					results.retain(|e| index.related_ids.contains(&e.id));
				}
				RelationFilter::NoRelations => {
					results.retain(|e| !index.related_ids.contains(&e.id));
				}
			}
		}

		if let Some(top_k) = query.meaning.as_ref().and_then(|meaning| meaning.top_k) {
			let before = results.len();
			results.truncate(top_k);
			steps.push(QueryPlanStep {
				strategy: QueryPlanStrategy::TopK,
				filter: Some(QueryFilterIdentity::Meaning),
				candidates_before: before,
				candidates_after: results.len(),
			});
		}

		if query.meaning.is_none() && !lexical_scores.is_empty() {
			results.sort_by(|left, right| {
				lexical_scores[&right.id]
					.total_cmp(&lexical_scores[&left.id])
					.then_with(|| left.id.cmp(&right.id))
			});
		} else if query.meaning.is_none() {
			results.sort_by(|left, right| {
				let ordering = match query.order {
					QueryOrder::CreatedAtAsc => left.created_at.cmp(&right.created_at),
					QueryOrder::CreatedAtDesc => right.created_at.cmp(&left.created_at),
					QueryOrder::UpdatedAtAsc => left.updated_at.cmp(&right.updated_at),
					QueryOrder::UpdatedAtDesc => right.updated_at.cmp(&left.updated_at),
					QueryOrder::ExpressionAsc => left.expression.cmp(&right.expression),
					QueryOrder::ExpressionDesc => right.expression.cmp(&left.expression),
				};
				ordering.then_with(|| left.id.cmp(&right.id))
			});
		}
		steps.push(QueryPlanStep {
			strategy: QueryPlanStrategy::DeterministicSort,
			filter: Some(QueryFilterIdentity::Ordering),
			candidates_before: results.len(),
			candidates_after: results.len(),
		});

		let has_full_text = matches!(
			query.expression.as_ref(),
			Some(ExpressionFilter::FullText(_))
		);
		let ranking_mode = if query.meaning.is_some() && has_full_text {
			let weights = query.hybrid_weights.unwrap_or(crate::query::HybridWeights {
				semantic: 1.0,
				lexical: 1.0,
			});
			QueryRankingMode::Hybrid {
				semantic_weight: weights.semantic,
				lexical_weight: weights.lexical,
			}
		} else if query.meaning.is_some() {
			QueryRankingMode::CosineSimilarity
		} else if has_full_text {
			QueryRankingMode::Bm25
		} else {
			QueryRankingMode::None
		};
		let primary = match ranking_mode {
			QueryRankingMode::Hybrid { .. } => QueryPrimaryOrder::CombinedScoreDescending,
			QueryRankingMode::CosineSimilarity => QueryPrimaryOrder::SimilarityDescending,
			QueryRankingMode::Bm25 => QueryPrimaryOrder::Bm25Descending,
			QueryRankingMode::None => QueryPrimaryOrder::Configured(query.order),
		};
		let ranking = match ranking_mode {
			QueryRankingMode::Hybrid { .. } => "weighted semantic and BM25".to_string(),
			QueryRankingMode::CosineSimilarity => "cosine similarity".to_string(),
			QueryRankingMode::Bm25 => "BM25".to_string(),
			QueryRankingMode::None => format!("{:?} with UUID tie-breaker", query.order),
		};
		let matches_before_pagination = results.len();

		// Apply pagination after filtering and ordering.
		if let Some(cursor) = query.cursor {
			let position = results
				.iter()
				.position(|entry| entry.id == cursor.after)
				.ok_or_else(|| {
					StorageError::Database(
						"Query cursor is not present in the ordered result set".to_string(),
					)
				})?;
			results = results.into_iter().skip(position + 1).collect();
		}
		if query.offset > 0 {
			results = results.into_iter().skip(query.offset).collect();
		}
		if let Some(limit) = query.limit {
			results.truncate(limit);
		}
		let results_returned = results.len();
		steps.push(QueryPlanStep {
			strategy: QueryPlanStrategy::Pagination,
			filter: Some(QueryFilterIdentity::Pagination),
			candidates_before: matches_before_pagination,
			candidates_after: results_returned,
		});
		let plan = QueryPlan {
			backend: "SQLite".to_string(),
			candidate_filters,
			ranking,
			candidates_loaded,
			matches_before_pagination,
			steps,
			ranking_mode,
			ordering: QueryPlanOrdering {
				primary,
				tie_breaker: QueryTieBreaker::UuidAscending,
			},
			pagination: QueryPaginationPlan {
				cursor: query.cursor,
				offset: query.offset,
				limit: query.limit,
				candidates_before: matches_before_pagination,
				candidates_after: results_returned,
			},
			results_returned,
		};
		let result_plan = query.explain.then(|| plan.clone());

		// Convert to QueryResults
		let query_results: Vec<QueryResult> = results
			.into_iter()
			.map(|entry| {
				let similarity_score = query
					.meaning
					.as_ref()
					.map(|m| crate::types::cosine_similarity(&entry.meaning, &m.vector));
				let lexical_score = lexical_scores.get(&entry.id).copied();
				let combined_score =
					similarity_score
						.zip(lexical_score)
						.map(|(semantic, lexical)| {
							let weights =
								query.hybrid_weights.unwrap_or(crate::query::HybridWeights {
									semantic: 1.0,
									lexical: 1.0,
								});
							(weights.semantic * ((semantic + 1.0) / 2.0)
								+ weights.lexical * lexical)
								/ (weights.semantic + weights.lexical)
						});

				let explanation = if query.explain {
					Some(self.generate_explanation(
						&entry,
						query,
						similarity_score,
						lexical_score,
						combined_score,
					))
				} else {
					None
				};

				QueryResult {
					entry,
					similarity_score,
					lexical_score,
					combined_score,
					explanation,
					plan: result_plan.clone(),
				}
			})
			.collect();

		Ok(QueryExecution {
			results: query_results,
			plan,
		})
	}

	fn update(&mut self, entry: &Entry) -> StorageResult<()> {
		self.validate_entry(entry)?;
		self.validate_relation_targets(entry)?;
		let existing = self.get(entry.id)?;
		if entry.created_at != existing.created_at || entry.updated_at < existing.updated_at {
			return Err(StorageError::Database(
				"Updates must preserve created_at and advance updated_at monotonically".to_string(),
			));
		}
		let id = entry.id.to_string();
		let meaning_bytes = vector_codec::serialize(&entry.meaning)
			.map_err(|e| StorageError::Database(format!("Failed to serialize vector: {}", e)))?;
		let context_json = serde_json::to_string(&entry.context)?;

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let rows_affected = transaction
			.execute(
				"UPDATE entries 
             SET meaning = ?1, expression = ?2, context = ?3, updated_at = ?4
             WHERE id = ?5",
				params![
					meaning_bytes,
					&entry.expression,
					context_json,
					entry.updated_at.to_rfc3339(),
					id,
				],
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;
		if rows_affected == 0 {
			return Err(StorageError::NotFound(entry.id));
		}

		// Entry relations are directed outgoing edges.
		transaction
			.execute("DELETE FROM relations WHERE from_id = ?1", params![id])
			.map_err(|e| StorageError::Database(e.to_string()))?;

		for relation_id in &entry.relations {
			transaction
				.execute(
					"INSERT OR IGNORE INTO relations (from_id, to_id) VALUES (?1, ?2)",
					params![id, relation_id.to_string()],
				)
				.map_err(|e| StorageError::Database(e.to_string()))?;
		}
		Self::record_revision(&transaction, entry, RevisionOperation::Update)?;

		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn update_batch(&mut self, entries: &[Entry]) -> StorageResult<()> {
		if entries.is_empty() {
			return Ok(());
		}
		let mut ids = HashSet::with_capacity(entries.len());
		for entry in entries {
			self.validate_entry(entry)?;
			self.validate_relation_targets(entry)?;
			if !ids.insert(entry.id) {
				return Err(StorageError::Database(format!(
					"Duplicate entry ID in batch: {}",
					entry.id
				)));
			}
		}
		let existing_ids = self.get_entry_ids()?;
		if let Some(missing) = ids.difference(&existing_ids).next() {
			return Err(StorageError::NotFound(*missing));
		}
		for entry in entries {
			let existing = self.get(entry.id)?;
			if entry.created_at != existing.created_at || entry.updated_at < existing.updated_at {
				return Err(StorageError::Database(
					"Updates must preserve created_at and advance updated_at monotonically"
						.to_string(),
				));
			}
		}

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for entry in entries {
			let meaning_bytes = vector_codec::serialize(&entry.meaning).map_err(|error| {
				StorageError::Database(format!("Failed to serialize vector: {error}"))
			})?;
			let context_json = serde_json::to_string(&entry.context)?;
			transaction
				.execute(
					"UPDATE entries
					 SET meaning = ?1, expression = ?2, context = ?3, updated_at = ?4
					 WHERE id = ?5",
					params![
						meaning_bytes,
						&entry.expression,
						context_json,
						entry.updated_at.to_rfc3339(),
						entry.id.to_string(),
					],
				)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			transaction
				.execute(
					"DELETE FROM relations WHERE from_id = ?1",
					params![entry.id.to_string()],
				)
				.map_err(|error| StorageError::Database(error.to_string()))?;
		}
		for entry in entries {
			for relation_id in &entry.relations {
				transaction
					.execute(
						"INSERT INTO relations (from_id, to_id) VALUES (?1, ?2)",
						params![entry.id.to_string(), relation_id.to_string()],
					)
					.map_err(|error| StorageError::Database(error.to_string()))?;
			}
			Self::record_revision(&transaction, entry, RevisionOperation::Update)?;
		}
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn delete(&mut self, id: Uuid) -> StorageResult<()> {
		let snapshot = self.get(id)?;
		let id_str = id.to_string();
		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;

		// Delete relations first
		transaction
			.execute(
				"DELETE FROM relations WHERE from_id = ?1 OR to_id = ?1",
				params![id_str],
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		// Delete entry
		let rows_affected = transaction
			.execute("DELETE FROM entries WHERE id = ?1", params![id_str])
			.map_err(|e| StorageError::Database(e.to_string()))?;

		if rows_affected == 0 {
			return Err(StorageError::NotFound(id));
		}
		Self::record_revision(&transaction, &snapshot, RevisionOperation::Delete)?;

		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn delete_batch(&mut self, ids: &[Uuid]) -> StorageResult<()> {
		if ids.is_empty() {
			return Ok(());
		}
		let unique_ids: HashSet<Uuid> = ids.iter().copied().collect();
		if unique_ids.len() != ids.len() {
			return Err(StorageError::Database(
				"Duplicate entry ID in delete batch".to_string(),
			));
		}
		let existing_ids = self.get_entry_ids()?;
		if let Some(missing) = unique_ids.difference(&existing_ids).next() {
			return Err(StorageError::NotFound(*missing));
		}
		let snapshots: Vec<Entry> = ids
			.iter()
			.map(|id| self.get(*id))
			.collect::<StorageResult<_>>()?;

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for (id, snapshot) in ids.iter().zip(&snapshots) {
			transaction
				.execute("DELETE FROM entries WHERE id = ?1", params![id.to_string()])
				.map_err(|error| StorageError::Database(error.to_string()))?;
			Self::record_revision(&transaction, snapshot, RevisionOperation::Delete)?;
		}
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn count(&self) -> StorageResult<usize> {
		let count: i64 = self
			.conn
			.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
			.map_err(|e| StorageError::Database(e.to_string()))?;
		Ok(count as usize)
	}

	fn integrity_check(&self) -> StorageResult<IntegrityReport> {
		let mut report = IntegrityReport::default();
		let quick_check: String = self
			.conn
			.query_row("PRAGMA quick_check", [], |row| row.get(0))
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if quick_check != "ok" {
			report.issues.push(IntegrityIssue {
				area: "sqlite".to_string(),
				message: quick_check,
			});
		}

		let mut foreign_key_statement = self
			.conn
			.prepare("PRAGMA foreign_key_check")
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let foreign_key_rows = foreign_key_statement
			.query_map([], |row| {
				Ok((
					row.get::<_, String>(0)?,
					row.get::<_, i64>(1)?,
					row.get::<_, String>(2)?,
				))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for row in foreign_key_rows {
			let (table, row_id, parent) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			report.issues.push(IntegrityIssue {
				area: "foreign_key".to_string(),
				message: format!("{table} row {row_id} references missing {parent}"),
			});
		}

		let actual_dimension = match self.validate_existing_vectors() {
			Ok(dimension) => dimension,
			Err(error) => {
				report.issues.push(IntegrityIssue {
					area: "entries".to_string(),
					message: error.to_string(),
				});
				None
			}
		};
		if let Err(error) = self.validate_revisions() {
			report.issues.push(IntegrityIssue {
				area: "revisions".to_string(),
				message: error.to_string(),
			});
		}
		if let Err(error) = self.conn.execute(
			"INSERT INTO entries_fts(entries_fts) VALUES ('integrity-check')",
			[],
		) {
			report.issues.push(IntegrityIssue {
				area: "full_text".to_string(),
				message: format!("FTS5 integrity check failed: {error}"),
			});
		}
		let entry_count = self.count()?;
		let search_count: usize = self
			.conn
			.query_row("SELECT COUNT(*) FROM entries_fts", [], |row| row.get(0))
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if entry_count != search_count {
			report.issues.push(IntegrityIssue {
				area: "full_text".to_string(),
				message: format!(
					"Search index contains {search_count} rows for {entry_count} entries"
				),
			});
		}
		let search_mismatch_count: usize = self
			.conn
			.query_row(
				"SELECT
					(SELECT COUNT(*)
					 FROM entries AS entry
					 LEFT JOIN entries_fts AS search ON search.rowid = entry.rowid
					 WHERE search.rowid IS NULL
						OR search.id <> entry.id
						OR search.expression <> entry.expression)
					+
					(SELECT COUNT(*)
					 FROM entries_fts AS search
					 LEFT JOIN entries AS entry ON entry.rowid = search.rowid
					 WHERE entry.rowid IS NULL)",
				[],
				|row| row.get(0),
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		if search_mismatch_count > 0 {
			report.issues.push(IntegrityIssue {
				area: "full_text".to_string(),
				message: format!(
					"Search index contains {search_mismatch_count} stale or mismatched rows"
				),
			});
		}
		match self.inspect_embedding_metadata() {
			Err(error) => report.issues.push(IntegrityIssue {
				area: "metadata".to_string(),
				message: error.to_string(),
			}),
			Ok((_, metadata_dimension)) => {
				if entry_count > 0 && metadata_dimension.is_none() {
					report.issues.push(IntegrityIssue {
						area: "metadata".to_string(),
						message: "Stored entries are missing vector-dimension metadata".to_string(),
					});
				}
				if let (Some(metadata_dimension), Some(actual_dimension)) =
					(metadata_dimension, actual_dimension)
				{
					if metadata_dimension != actual_dimension {
						report.issues.push(IntegrityIssue {
							area: "metadata".to_string(),
							message: format!(
								"Stored vector dimension {metadata_dimension} does not match data dimension {actual_dimension}"
							),
						});
					}
				}
			}
		}

		Ok(report)
	}

	fn backup_to(&self, destination: &Path) -> StorageResult<()> {
		if destination.exists() {
			return Err(StorageError::Database(format!(
				"Backup destination already exists: {}",
				destination.display()
			)));
		}
		let mut destination_connection = Connection::open(destination)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let backup = rusqlite::backup::Backup::new(&self.conn, &mut destination_connection)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		backup
			.run_to_completion(64, Duration::from_millis(10), None)
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn embedding_profile(&self) -> StorageResult<Option<EmbeddingProfile>> {
		self.inspect_embedding_metadata()
			.map(|(profile, _)| profile)
	}

	fn set_embedding_profile(&mut self, profile: &EmbeddingProfile) -> StorageResult<()> {
		Self::validate_embedding_profile(profile)?;
		if self
			.stored_vector_dimension()?
			.is_some_and(|dimension| dimension != profile.dimensions)
		{
			return Err(StorageError::InvalidDimensions);
		}
		if self.count()? > 0 {
			match self.embedding_profile()? {
				Some(current) if current == *profile => return Ok(()),
				Some(_) => {
					return Err(StorageError::Database(
						"Changing the embedding profile requires an explicit embedding migration"
							.to_string(),
					));
				}
				None => {
					return Err(StorageError::Database(
						"Populated legacy data must be explicitly adopted before assigning an embedding profile"
							.to_string(),
					));
				}
			}
		}

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		Self::write_embedding_profile(&transaction, profile)?;
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn adopt_legacy_embedding_profile(&mut self, profile: &EmbeddingProfile) -> StorageResult<()> {
		Self::validate_embedding_profile(profile)?;
		if self.count()? == 0 {
			return Err(StorageError::Database(
				"There is no legacy embedding data to adopt".to_string(),
			));
		}
		if self.embedding_profile()?.is_some() {
			return Err(StorageError::Database(
				"The database already has an embedding profile".to_string(),
			));
		}
		if self.validate_existing_vectors()? != Some(profile.dimensions) {
			return Err(StorageError::InvalidDimensions);
		}

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		Self::write_embedding_profile(&transaction, profile)?;
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn migrate_embeddings(
		&mut self,
		profile: &EmbeddingProfile,
		replacements: &[(Uuid, Vec<f32>)],
	) -> StorageResult<()> {
		Self::validate_embedding_profile(profile)?;
		let entries = self.get_all_entries()?;
		if replacements.len() != entries.len() {
			return Err(StorageError::Database(format!(
				"Embedding migration requires {} replacement vectors, received {}",
				entries.len(),
				replacements.len()
			)));
		}

		let entry_ids: HashSet<Uuid> = entries.iter().map(|entry| entry.id).collect();
		let mut replacement_vectors = HashMap::with_capacity(replacements.len());
		for (id, vector) in replacements {
			if !entry_ids.contains(id) {
				return Err(StorageError::NotFound(*id));
			}
			Self::validate_vector(vector)?;
			if vector.len() != profile.dimensions {
				return Err(StorageError::InvalidDimensions);
			}
			if replacement_vectors.insert(*id, vector.clone()).is_some() {
				return Err(StorageError::Database(format!(
					"Duplicate replacement vector for entry {id}"
				)));
			}
		}
		if let Some(missing) = entry_ids
			.iter()
			.find(|id| !replacement_vectors.contains_key(id))
		{
			return Err(StorageError::Database(format!(
				"Missing replacement vector for entry {missing}"
			)));
		}

		let migration_time = Utc::now();
		let updated_entries: Vec<Entry> = entries
			.into_iter()
			.map(|mut entry| {
				entry.meaning = replacement_vectors
					.remove(&entry.id)
					.expect("replacement coverage was validated");
				if entry.updated_at < migration_time {
					entry.updated_at = migration_time;
				}
				entry
			})
			.collect();

		let transaction = self
			.conn
			.transaction()
			.map_err(|error| StorageError::Database(error.to_string()))?;
		for entry in &updated_entries {
			let meaning = vector_codec::serialize(&entry.meaning).map_err(|error| {
				StorageError::Database(format!("Failed to serialize vector: {error}"))
			})?;
			let updated = transaction
				.execute(
					"UPDATE entries SET meaning = ?1, updated_at = ?2 WHERE id = ?3",
					params![meaning, entry.updated_at.to_rfc3339(), entry.id.to_string()],
				)
				.map_err(|error| StorageError::Database(error.to_string()))?;
			if updated != 1 {
				return Err(StorageError::NotFound(entry.id));
			}
			Self::record_revision(&transaction, entry, RevisionOperation::Update)?;
		}
		Self::write_embedding_profile(&transaction, profile)?;
		transaction
			.commit()
			.map_err(|error| StorageError::Database(error.to_string()))
	}

	fn revisions(&self, id: Uuid) -> StorageResult<Vec<EntryRevision>> {
		let mut statement = self
			.conn
			.prepare(
				"SELECT revision_id, operation, snapshot, recorded_at
				 FROM entry_revisions
				 WHERE entry_id = ?1
				 ORDER BY recorded_at, revision_id",
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let rows = statement
			.query_map(params![id.to_string()], |row| {
				Ok((
					row.get::<_, String>(0)?,
					row.get::<_, String>(1)?,
					row.get::<_, String>(2)?,
					row.get::<_, String>(3)?,
				))
			})
			.map_err(|error| StorageError::Database(error.to_string()))?;
		let mut revisions = Vec::new();
		for row in rows {
			let (revision_id, operation, snapshot, recorded_at) =
				row.map_err(|error| StorageError::Database(error.to_string()))?;
			let operation = match operation.as_str() {
				"snapshot" => RevisionOperation::Snapshot,
				"insert" => RevisionOperation::Insert,
				"update" => RevisionOperation::Update,
				"delete" => RevisionOperation::Delete,
				other => {
					return Err(StorageError::Database(format!(
						"Unknown revision operation: {other}"
					)))
				}
			};
			revisions.push(EntryRevision {
				revision_id: Uuid::parse_str(&revision_id)
					.map_err(|error| StorageError::Database(error.to_string()))?,
				entry_id: id,
				operation,
				snapshot: serde_json::from_str(&snapshot)?,
				recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
					.map_err(|error| StorageError::Database(error.to_string()))?
					.with_timezone(&Utc),
			});
		}
		Ok(revisions)
	}

	fn create_context_index(&mut self, path: &str) -> StorageResult<String> {
		let sqlite_path = Self::json_pointer_to_sqlite_path(path)?;
		let mut hash = 0xcbf29ce484222325_u64;
		for byte in path.as_bytes() {
			hash ^= u64::from(*byte);
			hash = hash.wrapping_mul(0x100000001b3);
		}
		let name = format!("idx_entries_context_{hash:016x}");
		let path_literal = Self::sql_string_literal(&sqlite_path);
		self.conn
			.execute(
				&format!("CREATE INDEX IF NOT EXISTS {name} ON entries(json_extract(context, {path_literal}))"),
				[],
			)
			.map_err(|error| StorageError::Database(error.to_string()))?;
		Ok(name)
	}

	fn backend_name(&self) -> &str {
		"SQLite"
	}
}

struct RelationIndex {
	adjacency: HashMap<Uuid, Vec<Uuid>>,
	related_ids: HashSet<Uuid>,
}

// Vectors are JSON-encoded inside the BLOB column for portable decoding.
mod vector_codec {
	use serde::{Deserialize, Serialize};

	pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
		serde_json::to_vec(value).map_err(|e| e.to_string())
	}

	pub fn deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
		serde_json::from_slice(bytes).map_err(|e| e.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::query::{ContextFilter, MeaningFilter, RelationFilter, TemporalFilter};
	use chrono::TimeZone;
	use std::collections::HashSet;

	fn create_test_storage() -> SqliteStorage {
		SqliteStorage::in_memory().unwrap()
	}

	fn create_test_entry(meaning: Vec<f32>, expression: &str) -> Entry {
		Entry::new(meaning, expression.to_string())
	}

	// ==================== Storage Initialization Tests ====================

	#[test]
	fn test_storage_in_memory_creation() {
		let storage = SqliteStorage::in_memory();
		assert!(storage.is_ok());
	}

	#[test]
	fn test_storage_backend_name() {
		let storage = create_test_storage();
		assert_eq!(storage.backend_name(), "SQLite");
	}

	#[test]
	fn test_storage_initial_count_is_zero() {
		let storage = create_test_storage();
		assert_eq!(storage.count().unwrap(), 0);
	}

	#[test]
	fn test_storage_initializes_schema_and_foreign_keys() {
		let storage = create_test_storage();
		let version: i64 = storage
			.conn
			.query_row("PRAGMA user_version", [], |row| row.get(0))
			.unwrap();
		let foreign_keys: i64 = storage
			.conn
			.query_row("PRAGMA foreign_keys", [], |row| row.get(0))
			.unwrap();

		assert_eq!(version, SCHEMA_VERSION);
		assert_eq!(foreign_keys, 1);
	}

	#[test]
	fn test_file_storage_configures_durability_and_contention() {
		let directory = tempfile::TempDir::new().unwrap();
		let storage = SqliteStorage::new(directory.path().join("configured.db")).unwrap();
		let journal_mode: String = storage
			.conn
			.query_row("PRAGMA journal_mode", [], |row| row.get(0))
			.unwrap();
		let synchronous: i64 = storage
			.conn
			.query_row("PRAGMA synchronous", [], |row| row.get(0))
			.unwrap();
		let busy_timeout: i64 = storage
			.conn
			.query_row("PRAGMA busy_timeout", [], |row| row.get(0))
			.unwrap();

		assert_eq!(journal_mode, "wal");
		assert_eq!(synchronous, 1);
		assert_eq!(busy_timeout, 5_000);
	}

	#[test]
	fn test_legacy_database_migrates_with_snapshot_history() {
		let directory = tempfile::TempDir::new().unwrap();
		let path = directory.path().join("legacy.db");
		let entry = create_test_entry(vec![0.1, 0.2], "Legacy");
		let connection = Connection::open(&path).unwrap();
		connection
			.execute_batch(
				"CREATE TABLE entries (
					id TEXT PRIMARY KEY, meaning BLOB NOT NULL, expression TEXT NOT NULL,
					context TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
				);
				CREATE TABLE relations (
					from_id TEXT NOT NULL, to_id TEXT NOT NULL,
					PRIMARY KEY (from_id, to_id)
				);",
			)
			.unwrap();
		connection
			.execute(
				"INSERT INTO entries VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
				params![
					entry.id.to_string(),
					vector_codec::serialize(&entry.meaning).unwrap(),
					entry.expression,
					serde_json::to_string(&entry.context).unwrap(),
					entry.created_at.to_rfc3339(),
					entry.updated_at.to_rfc3339(),
				],
			)
			.unwrap();
		drop(connection);

		let storage = SqliteStorage::new(&path).unwrap();

		assert_eq!(storage.get(entry.id).unwrap().expression, "Legacy");
		assert_eq!(
			storage.revisions(entry.id).unwrap()[0].operation,
			RevisionOperation::Snapshot
		);
	}

	#[test]
	fn test_legacy_database_with_orphan_relation_is_rejected() {
		let directory = tempfile::TempDir::new().unwrap();
		let path = directory.path().join("orphan.db");
		let connection = Connection::open(&path).unwrap();
		connection
			.execute_batch(
				"CREATE TABLE entries (
					id TEXT PRIMARY KEY, meaning BLOB NOT NULL, expression TEXT NOT NULL,
					context TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
				);
				CREATE TABLE relations (
					from_id TEXT NOT NULL, to_id TEXT NOT NULL,
					PRIMARY KEY (from_id, to_id)
				);",
			)
			.unwrap();
		connection
			.execute(
				"INSERT INTO relations VALUES (?1, ?2)",
				params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string()],
			)
			.unwrap();
		drop(connection);

		let result = SqliteStorage::new(&path);

		assert!(matches!(
			result,
			Err(StorageError::Database(message)) if message.contains("invalid relation")
		));
	}

	// ==================== Insert Tests ====================

	#[test]
	fn test_insert_single_entry() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1, 0.2, 0.3], "Test entry");

		let result = storage.insert(&entry);
		assert!(result.is_ok());
		assert_eq!(storage.count().unwrap(), 1);
	}

	#[test]
	fn test_insert_multiple_entries() {
		let mut storage = create_test_storage();

		for i in 0..10 {
			let entry = create_test_entry(vec![i as f32], &format!("Entry {}", i));
			storage.insert(&entry).unwrap();
		}

		assert_eq!(storage.count().unwrap(), 10);
	}

	#[test]
	fn test_insert_batch_supports_internal_relations() {
		let mut storage = create_test_storage();
		let target = create_test_entry(vec![0.1], "Target");
		let source = create_test_entry(vec![0.2], "Source").add_relation(target.id);

		storage.insert_batch(&[source.clone(), target]).unwrap();

		assert_eq!(storage.count().unwrap(), 2);
		assert_eq!(storage.get(source.id).unwrap().relations.len(), 1);
	}

	#[test]
	fn test_insert_batch_rolls_back_on_duplicate() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Duplicate");

		assert!(storage.insert_batch(&[entry.clone(), entry]).is_err());
		assert_eq!(storage.count().unwrap(), 0);
	}

	#[test]
	fn test_insert_batch_rejects_invalid_timestamps_atomically() {
		let mut storage = create_test_storage();
		let valid = create_test_entry(vec![0.1], "Valid");
		let mut invalid = create_test_entry(vec![0.2], "Invalid");
		invalid.updated_at = invalid.created_at - chrono::Duration::seconds(1);

		assert!(storage.insert_batch(&[valid, invalid]).is_err());
		assert_eq!(storage.count().unwrap(), 0);
	}

	#[test]
	fn test_insert_entry_with_context() {
		let mut storage = create_test_storage();
		let context = serde_json::json!({
			"source": "test",
			"priority": 1,
			"tags": ["a", "b", "c"]
		});
		let entry = create_test_entry(vec![0.1], "With context").with_context(context);

		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.context["source"], "test");
		assert_eq!(retrieved.context["priority"], 1);
	}

	#[test]
	fn test_insert_entry_with_relations() {
		let mut storage = create_test_storage();

		// Insert two entries
		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");
		storage.insert(&entry1).unwrap();
		storage.insert(&entry2).unwrap();

		// Insert entry with relation
		let entry3 = create_test_entry(vec![0.3], "Entry 3")
			.add_relation(entry1.id)
			.add_relation(entry2.id);
		storage.insert(&entry3).unwrap();

		let retrieved = storage.get(entry3.id).unwrap();
		assert_eq!(retrieved.relations.len(), 2);
		assert!(retrieved.relations.contains(&entry1.id));
		assert!(retrieved.relations.contains(&entry2.id));
	}

	#[test]
	fn test_insert_entry_with_empty_meaning() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![], "No embedding");

		let result = storage.insert(&entry);
		assert!(matches!(result, Err(StorageError::InvalidDimensions)));
	}

	#[test]
	fn test_insert_rejects_non_finite_meaning() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![f32::NAN], "Invalid embedding");

		assert!(matches!(
			storage.insert(&entry),
			Err(StorageError::InvalidDimensions)
		));
	}

	#[test]
	fn test_insert_rejects_mismatched_dimensions() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1, 0.2], "First"))
			.unwrap();

		assert!(matches!(
			storage.insert(&create_test_entry(vec![0.1], "Second")),
			Err(StorageError::InvalidDimensions)
		));
	}

	#[test]
	fn test_insert_entry_with_large_vector() {
		let mut storage = create_test_storage();
		let large_vector: Vec<f32> = (0..1536).map(|i| i as f32 / 1536.0).collect();
		let entry = create_test_entry(large_vector.clone(), "Large vector");

		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.meaning.len(), 1536);
		assert!((retrieved.meaning[0] - large_vector[0]).abs() < 0.0001);
	}

	// ==================== Get Tests ====================

	#[test]
	fn test_get_existing_entry() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1, 0.2, 0.3], "Test entry");
		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();

		assert_eq!(retrieved.id, entry.id);
		assert_eq!(retrieved.expression, entry.expression);
		assert_eq!(retrieved.meaning, entry.meaning);
	}

	#[test]
	fn test_get_nonexistent_entry() {
		let storage = create_test_storage();
		let fake_id = Uuid::new_v4();

		let result = storage.get(fake_id);
		assert!(result.is_err());

		match result {
			Err(StorageError::NotFound(id)) => assert_eq!(id, fake_id),
			_ => panic!("Expected NotFound error"),
		}
	}

	#[test]
	fn test_get_preserves_timestamps() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let original_created = entry.created_at;
		let original_updated = entry.updated_at;
		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();

		// Timestamps should be close (within 1 second due to serialization)
		assert!(
			(retrieved.created_at - original_created)
				.num_seconds()
				.abs() < 1
		);
		assert!(
			(retrieved.updated_at - original_updated)
				.num_seconds()
				.abs() < 1
		);
	}

	// ==================== Update Tests ====================

	#[test]
	fn test_update_entry_expression() {
		let mut storage = create_test_storage();
		let mut entry = create_test_entry(vec![0.1], "Original");
		storage.insert(&entry).unwrap();

		entry.expression = "Updated".to_string();
		entry.updated_at = Utc::now();
		storage.update(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.expression, "Updated");
	}

	#[test]
	fn test_update_entry_meaning() {
		let mut storage = create_test_storage();
		let mut entry = create_test_entry(vec![0.1, 0.2], "Test");
		storage.insert(&entry).unwrap();

		entry.meaning = vec![0.9, 0.8];
		storage.update(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.meaning, vec![0.9, 0.8]);
	}

	#[test]
	fn test_update_entry_context() {
		let mut storage = create_test_storage();
		let mut entry =
			create_test_entry(vec![0.1], "Test").with_context(serde_json::json!({"version": 1}));
		storage.insert(&entry).unwrap();

		entry.context = serde_json::json!({"version": 2, "new_field": "added"});
		storage.update(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.context["version"], 2);
		assert_eq!(retrieved.context["new_field"], "added");
	}

	#[test]
	fn test_update_entry_relations() {
		let mut storage = create_test_storage();

		let target1 = create_test_entry(vec![0.1], "Target 1");
		let target2 = create_test_entry(vec![0.2], "Target 2");
		storage.insert(&target1).unwrap();
		storage.insert(&target2).unwrap();

		let mut entry = create_test_entry(vec![0.3], "Entry").add_relation(target1.id);
		storage.insert(&entry).unwrap();

		// Update relations
		entry.relations = vec![target2.id];
		storage.update(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.relations.len(), 1);
		assert!(retrieved.relations.contains(&target2.id));
		assert!(!retrieved.relations.contains(&target1.id));
	}

	#[test]
	fn test_update_nonexistent_entry_returns_not_found() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Missing");

		assert!(matches!(
			storage.update(&entry),
			Err(StorageError::NotFound(id)) if id == entry.id
		));
	}

	#[test]
	fn test_update_batch_is_atomic() {
		let mut storage = create_test_storage();
		let mut first = create_test_entry(vec![0.1], "First");
		let second = create_test_entry(vec![0.2], "Second");
		storage.insert(&first).unwrap();
		first.expression = "Changed".to_string();

		assert!(storage.update_batch(&[first.clone(), second]).is_err());
		assert_eq!(storage.get(first.id).unwrap().expression, "First");
	}

	// ==================== Delete Tests ====================

	#[test]
	fn test_delete_entry() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		storage.insert(&entry).unwrap();

		let result = storage.delete(entry.id);
		assert!(result.is_ok());
		assert_eq!(storage.count().unwrap(), 0);
	}

	#[test]
	fn test_delete_nonexistent_entry() {
		let mut storage = create_test_storage();
		let fake_id = Uuid::new_v4();

		let result = storage.delete(fake_id);
		assert!(result.is_err());

		match result {
			Err(StorageError::NotFound(id)) => assert_eq!(id, fake_id),
			_ => panic!("Expected NotFound error"),
		}
	}

	#[test]
	fn test_delete_batch_is_atomic() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Existing");
		storage.insert(&entry).unwrap();

		assert!(storage.delete_batch(&[entry.id, Uuid::new_v4()]).is_err());
		assert_eq!(storage.count().unwrap(), 1);
	}

	#[test]
	fn test_integrity_check_reports_healthy_database() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Healthy"))
			.unwrap();

		assert!(storage.integrity_check().unwrap().is_healthy());
	}

	#[test]
	fn test_integrity_check_detects_equal_count_full_text_drift() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Canonical expression");
		storage.insert(&entry).unwrap();
		storage
			.conn
			.execute(
				"UPDATE entries_fts SET expression = 'stale expression' WHERE id = ?1",
				params![entry.id.to_string()],
			)
			.unwrap();

		let report = storage.integrity_check().unwrap();

		assert!(report.issues.iter().any(|issue| {
			issue.area == "full_text" && issue.message.contains("stale or mismatched")
		}));
	}

	#[test]
	fn test_embedding_profile_is_persisted_and_locked_for_existing_data() {
		let mut storage = create_test_storage();
		let profile = EmbeddingProfile {
			model: "text-embedding-model".to_string(),
			version: Some("2026-07".to_string()),
			dimensions: 2,
		};
		storage.set_embedding_profile(&profile).unwrap();
		storage
			.insert(&create_test_entry(vec![0.1, 0.2], "Profiled"))
			.unwrap();

		assert_eq!(storage.embedding_profile().unwrap(), Some(profile));
		assert!(storage
			.set_embedding_profile(&EmbeddingProfile {
				model: "different-model".to_string(),
				version: None,
				dimensions: 2,
			})
			.is_err());
	}

	#[test]
	fn test_populated_legacy_profile_requires_explicit_adoption() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1, 0.2], "Legacy"))
			.unwrap();
		let profile = EmbeddingProfile {
			model: "legacy-model".to_string(),
			version: Some("v1".to_string()),
			dimensions: 2,
		};

		assert!(storage.set_embedding_profile(&profile).is_err());
		assert_eq!(storage.embedding_profile().unwrap(), None);
		storage.adopt_legacy_embedding_profile(&profile).unwrap();
		assert_eq!(storage.embedding_profile().unwrap(), Some(profile));
	}

	#[test]
	fn test_legacy_profile_adoption_rejects_wrong_dimensions() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1, 0.2], "Legacy"))
			.unwrap();
		let profile = EmbeddingProfile {
			model: "wrong-model".to_string(),
			version: None,
			dimensions: 3,
		};

		assert!(matches!(
			storage.adopt_legacy_embedding_profile(&profile),
			Err(StorageError::InvalidDimensions)
		));
		assert_eq!(storage.embedding_profile().unwrap(), None);
	}

	#[test]
	fn test_migrate_embeddings_replaces_every_vector_and_records_revisions() {
		let mut storage = create_test_storage();
		let old_profile = EmbeddingProfile {
			model: "old-model".to_string(),
			version: Some("v1".to_string()),
			dimensions: 2,
		};
		storage.set_embedding_profile(&old_profile).unwrap();
		let first = create_test_entry(vec![0.1, 0.2], "First");
		let second = create_test_entry(vec![0.3, 0.4], "Second");
		storage
			.insert_batch(&[first.clone(), second.clone()])
			.unwrap();
		let new_profile = EmbeddingProfile {
			model: "new-model".to_string(),
			version: Some("v2".to_string()),
			dimensions: 3,
		};

		storage
			.migrate_embeddings(
				&new_profile,
				&[
					(first.id, vec![1.0, 0.0, 0.0]),
					(second.id, vec![0.0, 1.0, 0.0]),
				],
			)
			.unwrap();

		assert_eq!(storage.embedding_profile().unwrap(), Some(new_profile));
		assert_eq!(storage.get(first.id).unwrap().meaning, vec![1.0, 0.0, 0.0]);
		assert_eq!(storage.get(second.id).unwrap().meaning, vec![0.0, 1.0, 0.0]);
		let first_revisions = storage.revisions(first.id).unwrap();
		assert_eq!(first_revisions.len(), 2);
		assert_eq!(first_revisions[1].operation, RevisionOperation::Update);
		assert_eq!(first_revisions[1].snapshot.meaning, vec![1.0, 0.0, 0.0]);
	}

	#[test]
	fn test_migrate_embeddings_rolls_back_vectors_profile_and_revisions() {
		let mut storage = create_test_storage();
		let old_profile = EmbeddingProfile {
			model: "old-model".to_string(),
			version: None,
			dimensions: 2,
		};
		storage.set_embedding_profile(&old_profile).unwrap();
		let first = create_test_entry(vec![0.1, 0.2], "First");
		let second = create_test_entry(vec![0.3, 0.4], "Second");
		storage
			.insert_batch(&[first.clone(), second.clone()])
			.unwrap();
		let failing_id = [first.id, second.id].into_iter().max().unwrap();
		storage
			.conn
			.execute_batch(&format!(
				"CREATE TRIGGER fail_embedding_migration
				 BEFORE UPDATE OF meaning ON entries
				 WHEN old.id = '{failing_id}'
				 BEGIN SELECT RAISE(ABORT, 'forced migration failure'); END;"
			))
			.unwrap();
		let new_profile = EmbeddingProfile {
			model: "new-model".to_string(),
			version: None,
			dimensions: 3,
		};

		assert!(storage
			.migrate_embeddings(
				&new_profile,
				&[
					(first.id, vec![1.0, 0.0, 0.0]),
					(second.id, vec![0.0, 1.0, 0.0])
				],
			)
			.is_err());

		assert_eq!(storage.embedding_profile().unwrap(), Some(old_profile));
		assert_eq!(storage.get(first.id).unwrap().meaning, first.meaning);
		assert_eq!(storage.get(second.id).unwrap().meaning, second.meaning);
		assert_eq!(storage.revisions(first.id).unwrap().len(), 1);
		assert_eq!(storage.revisions(second.id).unwrap().len(), 1);
	}

	#[test]
	fn test_migrate_embeddings_requires_exact_valid_replacements() {
		let mut storage = create_test_storage();
		let first = create_test_entry(vec![0.1], "First");
		let second = create_test_entry(vec![0.2], "Second");
		storage
			.insert_batch(&[first.clone(), second.clone()])
			.unwrap();
		let profile = EmbeddingProfile {
			model: "new-model".to_string(),
			version: None,
			dimensions: 2,
		};

		assert!(storage
			.migrate_embeddings(&profile, &[(first.id, vec![1.0, 0.0])])
			.is_err());
		assert!(storage
			.migrate_embeddings(
				&profile,
				&[(first.id, vec![1.0, 0.0]), (first.id, vec![0.0, 1.0])],
			)
			.is_err());
		assert!(storage
			.migrate_embeddings(
				&profile,
				&[(first.id, vec![f32::NAN, 0.0]), (second.id, vec![0.0, 1.0])],
			)
			.is_err());
		assert_eq!(storage.get(first.id).unwrap().meaning, first.meaning);
		assert_eq!(storage.get(second.id).unwrap().meaning, second.meaning);
	}

	#[test]
	fn test_integrity_check_reports_partial_and_malformed_profile_metadata() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1, 0.2], "Entry"))
			.unwrap();
		storage
			.conn
			.execute(
				"INSERT INTO contextdb_metadata (key, value) VALUES ('embedding_model_version', 'v1')",
				[],
			)
			.unwrap();

		let partial = storage.integrity_check().unwrap();
		assert!(partial.issues.iter().any(|issue| issue.area == "metadata"));

		storage
			.conn
			.execute(
				"UPDATE contextdb_metadata SET value = 'invalid' WHERE key = 'vector_dimension'",
				[],
			)
			.unwrap();
		let malformed = storage.integrity_check().unwrap();
		assert!(malformed
			.issues
			.iter()
			.any(|issue| issue.area == "metadata" && issue.message.contains("vector dimension")));
	}

	#[test]
	fn test_backup_and_restore_preserve_data() {
		let directory = tempfile::TempDir::new().unwrap();
		let source_path = directory.path().join("source.db");
		let backup_path = directory.path().join("backup.db");
		let restored_path = directory.path().join("restored.db");
		let mut source = SqliteStorage::new(&source_path).unwrap();
		let entry = create_test_entry(vec![0.1], "Backed up");
		source.insert(&entry).unwrap();
		source.backup_to(&backup_path).unwrap();

		SqliteStorage::restore_from(&backup_path, &restored_path).unwrap();
		let restored = SqliteStorage::new(&restored_path).unwrap();

		assert_eq!(restored.get(entry.id).unwrap().expression, "Backed up");
		assert!(restored.integrity_check().unwrap().is_healthy());
	}

	#[test]
	fn test_revision_history_survives_deletion() {
		let mut storage = create_test_storage();
		let mut entry = create_test_entry(vec![0.1], "Original");
		storage.insert(&entry).unwrap();
		entry.expression = "Updated".to_string();
		entry.updated_at = Utc::now();
		storage.update(&entry).unwrap();
		storage.delete(entry.id).unwrap();

		let revisions = storage.revisions(entry.id).unwrap();
		assert_eq!(revisions.len(), 3);
		assert_eq!(revisions[0].operation, RevisionOperation::Insert);
		assert_eq!(revisions[0].snapshot.expression, "Original");
		assert_eq!(revisions[1].operation, RevisionOperation::Update);
		assert_eq!(revisions[1].snapshot.expression, "Updated");
		assert_eq!(revisions[2].operation, RevisionOperation::Delete);
		assert_eq!(revisions[2].snapshot.expression, "Updated");
	}

	#[test]
	fn test_delete_entry_with_relations() {
		let mut storage = create_test_storage();

		let target = create_test_entry(vec![0.1], "Target");
		storage.insert(&target).unwrap();

		let entry = create_test_entry(vec![0.2], "Entry").add_relation(target.id);
		storage.insert(&entry).unwrap();

		// Delete entry with relations
		storage.delete(entry.id).unwrap();
		assert_eq!(storage.count().unwrap(), 1);

		// Target should still exist
		assert!(storage.get(target.id).is_ok());
	}

	#[test]
	fn test_delete_target_of_relation() {
		let mut storage = create_test_storage();

		let target = create_test_entry(vec![0.1], "Target");
		storage.insert(&target).unwrap();

		let entry = create_test_entry(vec![0.2], "Entry").add_relation(target.id);
		storage.insert(&entry).unwrap();

		// Delete target (should clean up relation)
		storage.delete(target.id).unwrap();

		// Source entry should still exist but relation should be gone
		// Note: This tests the DELETE cascade on relations
		assert_eq!(storage.count().unwrap(), 1);
	}

	// ==================== Expression Filter Tests ====================

	#[test]
	fn test_matches_expression_equals() {
		let storage = create_test_storage();

		let filter = ExpressionFilter::Equals("exact match".to_string());
		assert!(storage.matches_expression("exact match", &filter).unwrap());
		assert!(!storage.matches_expression("Exact Match", &filter).unwrap());
		assert!(!storage.matches_expression("exact match ", &filter).unwrap());
	}

	#[test]
	fn test_matches_expression_contains_case_insensitive() {
		let storage = create_test_storage();

		let filter = ExpressionFilter::Contains("test".to_string());
		assert!(storage
			.matches_expression("This is a test", &filter)
			.unwrap());
		assert!(storage.matches_expression("TEST", &filter).unwrap());
		assert!(storage.matches_expression("Testing", &filter).unwrap());
		assert!(!storage
			.matches_expression("No match here", &filter)
			.unwrap());
	}

	#[test]
	fn test_matches_expression_starts_with() {
		let storage = create_test_storage();

		let filter = ExpressionFilter::StartsWith("Hello".to_string());
		assert!(storage.matches_expression("Hello World", &filter).unwrap());
		assert!(storage.matches_expression("Hello", &filter).unwrap());
		assert!(!storage.matches_expression("hello world", &filter).unwrap()); // case sensitive
		assert!(!storage.matches_expression("Say Hello", &filter).unwrap());
	}

	#[test]
	fn test_matches_expression_matches_pattern() {
		let storage = create_test_storage();

		let filter = ExpressionFilter::Matches("error".to_string());
		assert!(storage
			.matches_expression("An error occurred", &filter)
			.unwrap());
		assert!(storage.matches_expression("error", &filter).unwrap());
		assert!(!storage
			.matches_expression("An Error occurred", &filter)
			.unwrap()); // case sensitive
	}

	#[test]
	fn test_matches_expression_invalid_regex() {
		let storage = create_test_storage();

		let filter = ExpressionFilter::Matches("[".to_string());
		let result = storage.matches_expression("anything", &filter);
		assert!(result.is_err());
	}

	// ==================== Context Filter Tests ====================

	#[test]
	fn test_matches_context_path_exists() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"foo": {
				"bar": "value"
			}
		});

		let filter = ContextFilter::PathExists("/foo/bar".to_string());
		assert!(storage.matches_context(&context, &filter));

		let filter_missing = ContextFilter::PathExists("/foo/baz".to_string());
		assert!(!storage.matches_context(&context, &filter_missing));
	}

	#[test]
	fn test_matches_context_path_equals() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"status": "active",
			"count": 42
		});

		let filter = ContextFilter::PathEquals("/status".to_string(), serde_json::json!("active"));
		assert!(storage.matches_context(&context, &filter));

		let filter_wrong =
			ContextFilter::PathEquals("/status".to_string(), serde_json::json!("inactive"));
		assert!(!storage.matches_context(&context, &filter_wrong));

		let filter_int = ContextFilter::PathEquals("/count".to_string(), serde_json::json!(42));
		assert!(storage.matches_context(&context, &filter_int));
	}

	#[test]
	fn test_matches_context_path_contains() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"tags": ["rust", "database", "embedded"]
		});

		let filter = ContextFilter::PathContains("/tags".to_string(), serde_json::json!("rust"));
		assert!(storage.matches_context(&context, &filter));

		let filter_missing =
			ContextFilter::PathContains("/tags".to_string(), serde_json::json!("python"));
		assert!(!storage.matches_context(&context, &filter_missing));
	}

	#[test]
	fn test_matches_context_path_contains_non_array() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"name": "test"
		});

		let filter = ContextFilter::PathContains("/name".to_string(), serde_json::json!("test"));
		assert!(!storage.matches_context(&context, &filter)); // Not an array
	}

	#[test]
	fn test_matches_context_and() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"a": 1,
			"b": 2
		});

		let filter = ContextFilter::And(vec![
			ContextFilter::PathExists("/a".to_string()),
			ContextFilter::PathExists("/b".to_string()),
		]);
		assert!(storage.matches_context(&context, &filter));

		let filter_partial = ContextFilter::And(vec![
			ContextFilter::PathExists("/a".to_string()),
			ContextFilter::PathExists("/c".to_string()),
		]);
		assert!(!storage.matches_context(&context, &filter_partial));
	}

	#[test]
	fn test_matches_context_or() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"a": 1
		});

		let filter = ContextFilter::Or(vec![
			ContextFilter::PathExists("/a".to_string()),
			ContextFilter::PathExists("/b".to_string()),
		]);
		assert!(storage.matches_context(&context, &filter));

		let filter_none = ContextFilter::Or(vec![
			ContextFilter::PathExists("/x".to_string()),
			ContextFilter::PathExists("/y".to_string()),
		]);
		assert!(!storage.matches_context(&context, &filter_none));
	}

	#[test]
	fn test_matches_context_nested_and_or() {
		let storage = create_test_storage();
		let context = serde_json::json!({
			"type": "user",
			"status": "active"
		});

		// (type exists AND (status = active OR status = pending))
		let filter = ContextFilter::And(vec![
			ContextFilter::PathExists("/type".to_string()),
			ContextFilter::Or(vec![
				ContextFilter::PathEquals("/status".to_string(), serde_json::json!("active")),
				ContextFilter::PathEquals("/status".to_string(), serde_json::json!("pending")),
			]),
		]);
		assert!(storage.matches_context(&context, &filter));
	}

	// ==================== Temporal Filter Tests ====================

	#[test]
	fn test_matches_temporal_created_after() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let future = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

		assert!(storage.matches_temporal(&entry, &TemporalFilter::CreatedAfter(past)));
		assert!(!storage.matches_temporal(&entry, &TemporalFilter::CreatedAfter(future)));
	}

	#[test]
	fn test_matches_temporal_created_before() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let future = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

		assert!(!storage.matches_temporal(&entry, &TemporalFilter::CreatedBefore(past)));
		assert!(storage.matches_temporal(&entry, &TemporalFilter::CreatedBefore(future)));
	}

	#[test]
	fn test_matches_temporal_created_between() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let future = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

		assert!(storage.matches_temporal(&entry, &TemporalFilter::CreatedBetween(past, future)));

		let narrow_start = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let narrow_end = Utc.with_ymd_and_hms(2020, 1, 2, 0, 0, 0).unwrap();
		assert!(!storage.matches_temporal(
			&entry,
			&TemporalFilter::CreatedBetween(narrow_start, narrow_end)
		));
	}

	#[test]
	fn test_matches_temporal_updated_after() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();

		assert!(storage.matches_temporal(&entry, &TemporalFilter::UpdatedAfter(past)));
	}

	#[test]
	fn test_matches_temporal_updated_before() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let future = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

		assert!(storage.matches_temporal(&entry, &TemporalFilter::UpdatedBefore(future)));
	}

	// ==================== Query Tests ====================

	#[test]
	fn test_query_empty_database() {
		let storage = create_test_storage();
		let query = Query::new();

		let results = storage.query(&query).unwrap();
		assert!(results.is_empty());
	}

	#[test]
	fn test_query_all_entries() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Entry 1"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "Entry 2"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.3], "Entry 3"))
			.unwrap();

		let query = Query::new();
		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn test_query_with_limit() {
		let mut storage = create_test_storage();
		for i in 0..10 {
			storage
				.insert(&create_test_entry(vec![i as f32], &format!("Entry {}", i)))
				.unwrap();
		}

		let query = Query::new().with_limit(5);
		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 5);
	}

	#[test]
	fn test_query_cursor_continues_after_previous_page() {
		let mut storage = create_test_storage();
		let mut entries = [
			create_test_entry(vec![0.1], "First"),
			create_test_entry(vec![0.2], "Second"),
			create_test_entry(vec![0.3], "Third"),
		];
		let base = Utc::now();
		for (index, entry) in entries.iter_mut().enumerate() {
			entry.created_at = base + chrono::Duration::seconds(index as i64);
			entry.updated_at = entry.created_at;
			storage.insert(entry).unwrap();
		}

		let first_page = storage.query(&Query::new().with_limit(2)).unwrap();
		let second_page = storage
			.query(
				&Query::new()
					.with_cursor_after(first_page[1].entry.id)
					.with_limit(2),
			)
			.unwrap();

		assert_eq!(first_page.len(), 2);
		assert_eq!(second_page.len(), 1);
		assert_eq!(second_page[0].entry.expression, "Third");
	}

	#[test]
	fn test_query_with_limit_zero() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Entry"))
			.unwrap();

		let query = Query::new().with_limit(0);
		let results = storage.query(&query).unwrap();
		assert!(results.is_empty());
	}

	#[test]
	fn test_query_by_expression_equals() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Target"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "Other"))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::Equals("Target".to_string()));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "Target");
	}

	#[test]
	fn test_large_filtered_candidate_set_does_not_load_unrelated_entries() {
		let mut storage = create_test_storage();
		let mut entries: Vec<Entry> = (0..901)
			.map(|index| create_test_entry(vec![index as f32], "Match"))
			.collect();
		let unrelated = create_test_entry(vec![1.0], "Unrelated");
		entries.push(unrelated.clone());
		storage.insert_batch(&entries).unwrap();
		storage
			.conn
			.execute(
				"UPDATE entries SET meaning = x'00' WHERE id = ?1",
				params![unrelated.id.to_string()],
			)
			.unwrap();

		let results = storage
			.query(&Query::new().with_expression(ExpressionFilter::Equals("Match".to_string())))
			.unwrap();

		assert_eq!(results.len(), 901);
	}

	#[test]
	fn test_query_by_expression_contains() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Hello World"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "World Hello"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.3], "Goodbye"))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::Contains("world".to_string()));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_query_by_meaning_similarity() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0, 0.0], "X axis"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.0, 1.0, 0.0], "Y axis"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.0, 0.0, 1.0], "Z axis"))
			.unwrap();

		// Query for vectors similar to X axis
		let query = Query::new().with_meaning(vec![1.0, 0.0, 0.0], Some(0.9));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "X axis");
		assert!(results[0].similarity_score.unwrap() > 0.99);
	}

	#[test]
	fn test_query_by_meaning_top_k() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "Very similar"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.9, 0.1], "Similar"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.0, 1.0], "Different"))
			.unwrap();

		let query = Query {
			meaning: Some(MeaningFilter {
				vector: vec![1.0, 0.0],
				threshold: None,
				top_k: Some(2),
			}),
			expression: None,
			context: None,
			relations: None,
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};

		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 2);
		// Should be ordered by similarity
		assert!(results[0].similarity_score.unwrap() >= results[1].similarity_score.unwrap());
	}

	#[test]
	fn test_query_by_context() {
		let mut storage = create_test_storage();
		storage
			.insert(
				&create_test_entry(vec![0.1], "Entry 1")
					.with_context(serde_json::json!({"type": "user"})),
			)
			.unwrap();
		storage
			.insert(
				&create_test_entry(vec![0.2], "Entry 2")
					.with_context(serde_json::json!({"type": "system"})),
			)
			.unwrap();

		let query = Query::new().with_context(ContextFilter::PathEquals(
			"/type".to_string(),
			serde_json::json!("user"),
		));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "Entry 1");
	}

	#[test]
	fn test_context_index_is_created_and_used_by_queries() {
		let mut storage = create_test_storage();
		let index = storage.create_context_index("/type").unwrap();
		storage
			.insert(
				&create_test_entry(vec![0.1], "Indexed")
					.with_context(serde_json::json!({"type": "user"})),
			)
			.unwrap();

		let exists: bool = storage
			.conn
			.query_row(
				"SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
				params![index],
				|row| row.get(0),
			)
			.unwrap();
		let results = storage
			.query(&Query::new().with_context(ContextFilter::PathEquals(
				"/type".to_string(),
				serde_json::json!("user"),
			)))
			.unwrap();

		assert!(exists);
		assert_eq!(results.len(), 1);
	}

	#[test]
	fn test_query_by_temporal() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Entry"))
			.unwrap();

		let past = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
		let query = Query::new().with_temporal(TemporalFilter::CreatedAfter(past));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
	}

	#[test]
	fn test_query_by_relations_directly_related() {
		let mut storage = create_test_storage();
		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");
		let entry3 = create_test_entry(vec![0.3], "Entry 3");

		storage.insert(&entry1).unwrap();
		storage.insert(&entry2).unwrap();
		storage.insert(&entry3).unwrap();

		let entry1 = entry1.add_relation(entry2.id);
		let entry2 = entry2.add_relation(entry3.id);

		storage.update(&entry1).unwrap();
		storage.update(&entry2).unwrap();

		let query = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::DirectlyRelatedTo(entry1.id)),
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};

		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.id, entry2.id);
	}

	#[test]
	fn test_query_by_relations_within_distance() {
		let mut storage = create_test_storage();
		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");
		let entry3 = create_test_entry(vec![0.3], "Entry 3");
		let entry4 = create_test_entry(vec![0.4], "Entry 4");

		storage.insert(&entry1).unwrap();
		storage.insert(&entry2).unwrap();
		storage.insert(&entry3).unwrap();
		storage.insert(&entry4).unwrap();

		let entry1 = entry1.add_relation(entry2.id);
		let entry2 = entry2.add_relation(entry3.id);

		storage.update(&entry1).unwrap();
		storage.update(&entry2).unwrap();

		let query = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::WithinDistance {
				from: entry1.id,
				max_hops: 2,
			}),
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};

		let mut results = storage.query(&query).unwrap();
		results.sort_by_key(|result| result.entry.expression.clone());
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].entry.id, entry2.id);
		assert_eq!(results[1].entry.id, entry3.id);
	}

	#[test]
	fn test_query_by_relations_has_and_no_relations() {
		let mut storage = create_test_storage();
		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");
		let entry3 = create_test_entry(vec![0.3], "Entry 3");
		let entry4 = create_test_entry(vec![0.4], "Entry 4");

		storage.insert(&entry1).unwrap();
		storage.insert(&entry2).unwrap();
		storage.insert(&entry3).unwrap();
		storage.insert(&entry4).unwrap();

		let entry1 = entry1.add_relation(entry2.id);
		let entry2 = entry2.add_relation(entry3.id);

		storage.update(&entry1).unwrap();
		storage.update(&entry2).unwrap();

		let query_has = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::HasRelations),
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};
		let results_has = storage.query(&query_has).unwrap();
		let has_ids: HashSet<Uuid> = results_has.into_iter().map(|r| r.entry.id).collect();
		assert_eq!(has_ids, HashSet::from([entry1.id, entry2.id]));

		let query_none = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::NoRelations),
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};
		let results_none = storage.query(&query_none).unwrap();
		let no_relation_ids: HashSet<Uuid> = results_none
			.into_iter()
			.map(|result| result.entry.id)
			.collect();
		assert_eq!(no_relation_ids, HashSet::from([entry3.id, entry4.id]));
	}

	#[test]
	fn test_has_relations_means_outgoing_relations() {
		let mut storage = create_test_storage();
		let target = create_test_entry(vec![0.1], "Target");
		let source = create_test_entry(vec![0.2], "Source");
		storage.insert(&target).unwrap();
		storage.insert(&source).unwrap();
		storage
			.update(&source.clone().add_relation(target.id))
			.unwrap();

		let has_relations = storage
			.query(&Query::new().with_relations(RelationFilter::HasRelations))
			.unwrap();
		let no_relations = storage
			.query(&Query::new().with_relations(RelationFilter::NoRelations))
			.unwrap();

		assert_eq!(has_relations.len(), 1);
		assert_eq!(has_relations[0].entry.id, source.id);
		assert_eq!(no_relations.len(), 1);
		assert_eq!(no_relations[0].entry.id, target.id);
	}

	#[test]
	fn test_query_combined_filters() {
		let mut storage = create_test_storage();
		storage
			.insert(
				&create_test_entry(vec![1.0, 0.0], "Hello World")
					.with_context(serde_json::json!({"type": "greeting"})),
			)
			.unwrap();
		storage
			.insert(
				&create_test_entry(vec![1.0, 0.0], "Hello There")
					.with_context(serde_json::json!({"type": "greeting"})),
			)
			.unwrap();
		storage
			.insert(
				&create_test_entry(vec![0.0, 1.0], "Hello Different")
					.with_context(serde_json::json!({"type": "greeting"})),
			)
			.unwrap();
		storage
			.insert(
				&create_test_entry(vec![1.0, 0.0], "Goodbye World")
					.with_context(serde_json::json!({"type": "farewell"})),
			)
			.unwrap();

		// Semantic + Expression + Context
		let query = Query::new()
			.with_meaning(vec![1.0, 0.0], Some(0.9))
			.with_expression(ExpressionFilter::Contains("hello".to_string()))
			.with_context(ContextFilter::PathEquals(
				"/type".to_string(),
				serde_json::json!("greeting"),
			));

		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_query_with_explanation() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "Test"))
			.unwrap();

		let query = Query::new()
			.with_meaning(vec![1.0, 0.0], Some(0.9))
			.with_expression(ExpressionFilter::Contains("test".to_string()))
			.with_explanation();

		let results = storage.query(&query).unwrap();
		assert_eq!(results.len(), 1);
		assert!(results[0].explanation.is_some());
		let plan = results[0].plan.as_ref().unwrap();
		assert_eq!(plan.backend, "SQLite");
		assert_eq!(plan.ranking, "cosine similarity");
		assert_eq!(plan.candidates_loaded, 1);

		let explanation = results[0].explanation.as_ref().unwrap();
		assert!(explanation.contains("Semantic similarity"));
		assert!(explanation.contains("expression filter"));
	}

	#[test]
	fn test_execute_records_regex_scan_provenance() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "match 42"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "other"))
			.unwrap();
		let execution = storage
			.execute(
				&Query::new()
					.with_expression(ExpressionFilter::Matches(r"^match \d+$".to_string()))
					.with_explanation(),
			)
			.unwrap();

		assert_eq!(execution.results.len(), 1);
		let step = execution
			.plan
			.steps
			.iter()
			.find(|step| step.strategy == QueryPlanStrategy::RustRegexScan)
			.unwrap();
		assert_eq!(step.filter, Some(QueryFilterIdentity::ExpressionRegex));
		assert_eq!((step.candidates_before, step.candidates_after), (2, 1));
		assert!(execution.results[0].plan.is_some());
	}

	#[test]
	fn test_execute_records_fts_and_semantic_strategies() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "rust database"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.0, 1.0], "swift interface"))
			.unwrap();

		let fts = storage
			.execute(&Query::new().with_expression(ExpressionFilter::FullText("rust".to_string())))
			.unwrap();
		assert_eq!(fts.plan.ranking_mode, QueryRankingMode::Bm25);
		assert!(fts
			.plan
			.steps
			.iter()
			.any(|step| step.strategy == QueryPlanStrategy::Fts5));

		let semantic = storage
			.execute(&Query::new().with_meaning(vec![1.0, 0.0], Some(0.9)))
			.unwrap();
		assert_eq!(
			semantic.plan.ranking_mode,
			QueryRankingMode::CosineSimilarity
		);
		assert!(semantic
			.plan
			.steps
			.iter()
			.any(|step| step.strategy == QueryPlanStrategy::LinearVectorScan));
		assert!(semantic.results[0].plan.is_none());
	}

	#[test]
	fn test_execute_preserves_exact_hybrid_weights() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "rust database"))
			.unwrap();
		let execution = storage
			.execute(
				&Query::new()
					.with_meaning(vec![1.0, 0.0], None)
					.with_expression(ExpressionFilter::FullText("rust".to_string()))
					.with_hybrid_weights(0.75, 0.25),
			)
			.unwrap();

		assert_eq!(
			execution.plan.ranking_mode,
			QueryRankingMode::Hybrid {
				semantic_weight: 0.75,
				lexical_weight: 0.25,
			}
		);
		assert_eq!(
			execution.plan.ordering.primary,
			QueryPrimaryOrder::CombinedScoreDescending
		);
	}

	#[test]
	fn test_execute_returns_plan_for_explained_zero_result_query() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "present"))
			.unwrap();
		let execution = storage
			.execute(
				&Query::new()
					.with_expression(ExpressionFilter::FullText("absent".to_string()))
					.with_explanation(),
			)
			.unwrap();

		assert!(execution.results.is_empty());
		assert_eq!(execution.plan.ranking_mode, QueryRankingMode::Bm25);
		assert_eq!(execution.plan.results_returned, 0);
		assert!(execution
			.plan
			.steps
			.iter()
			.any(|step| step.strategy == QueryPlanStrategy::Fts5));
	}

	#[test]
	fn test_execute_reconciles_candidate_and_pagination_counts() {
		let mut storage = create_test_storage();
		for expression in ["match one", "match two", "other"] {
			storage
				.insert(&create_test_entry(vec![0.1], expression))
				.unwrap();
		}
		let execution = storage
			.execute(
				&Query::new()
					.with_expression(ExpressionFilter::Contains("match".to_string()))
					.with_offset(1)
					.with_limit(1),
			)
			.unwrap();

		assert_eq!(execution.plan.candidates_loaded, 2);
		assert_eq!(execution.plan.matches_before_pagination, 2);
		assert_eq!(execution.plan.pagination.candidates_before, 2);
		assert_eq!(execution.plan.pagination.candidates_after, 1);
		assert_eq!(execution.plan.results_returned, execution.results.len());
		let sql_step = execution
			.plan
			.steps
			.iter()
			.find(|step| step.strategy == QueryPlanStrategy::SqlPredicate)
			.unwrap();
		assert_eq!(
			(sql_step.candidates_before, sql_step.candidates_after),
			(3, 2)
		);
	}

	#[test]
	fn test_query_with_invalid_regex_returns_error() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Test"))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::Matches("[".to_string()));
		let result = storage.query(&query);

		match result {
			Err(StorageError::Database(message)) => {
				assert!(message.contains("Invalid regex"));
			}
			_ => panic!("Expected invalid regex error"),
		}
	}

	#[test]
	fn test_query_regex_uses_pattern_semantics() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "foo 42"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "prefix foo 42"))
			.unwrap();

		let query =
			Query::new().with_expression(ExpressionFilter::Matches(r"^foo\s+\d+$".to_string()));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "foo 42");
	}

	#[test]
	fn test_semantic_top_k_is_applied_after_regex_filtering() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "nonmatching"))
			.unwrap();
		let matching = create_test_entry(vec![0.0, 1.0], "matching");
		storage.insert(&matching).unwrap();

		let results = storage
			.query(
				&Query::new()
					.with_meaning(vec![1.0, 0.0], None)
					.with_top_k(1)
					.with_expression(ExpressionFilter::Matches("^matching$".to_string())),
			)
			.unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.id, matching.id);
	}

	#[test]
	fn test_full_text_query_returns_bm25_relevance() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Rust database internals"))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![0.2], "Swift application UI"))
			.unwrap();

		let query = Query::new().with_expression(ExpressionFilter::FullText("rust".to_string()));
		let results = storage.query(&query).unwrap();

		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "Rust database internals");
		assert_eq!(results[0].lexical_score, Some(1.0));
	}

	#[test]
	fn test_hybrid_weights_change_result_order() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(
				vec![0.0, 1.0],
				"rust rust rust database",
			))
			.unwrap();
		storage
			.insert(&create_test_entry(vec![1.0, 0.0], "rust application"))
			.unwrap();

		let semantic_first = storage
			.query(
				&Query::new()
					.with_meaning(vec![1.0, 0.0], None)
					.with_expression(ExpressionFilter::FullText("rust".to_string()))
					.with_hybrid_weights(1.0, 0.0),
			)
			.unwrap();
		let lexical_first = storage
			.query(
				&Query::new()
					.with_meaning(vec![1.0, 0.0], None)
					.with_expression(ExpressionFilter::FullText("rust".to_string()))
					.with_hybrid_weights(0.0, 1.0),
			)
			.unwrap();

		assert_eq!(semantic_first[0].entry.expression, "rust application");
		assert_eq!(lexical_first[0].entry.expression, "rust rust rust database");
		assert!(semantic_first[0].combined_score.is_some());
	}

	#[test]
	fn test_query_within_distance_zero_hops() {
		let mut storage = create_test_storage();
		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");

		storage.insert(&entry1).unwrap();
		storage.insert(&entry2).unwrap();

		let entry1 = entry1.add_relation(entry2.id);
		storage.update(&entry1).unwrap();

		let query = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::WithinDistance {
				from: entry1.id,
				max_hops: 0,
			}),
			temporal: None,
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: false,
		};

		let results = storage.query(&query).unwrap();
		assert!(results.is_empty());
	}

	#[test]
	fn test_generate_explanation_includes_all_filters() {
		let storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		let query = Query {
			meaning: Some(MeaningFilter {
				vector: vec![0.1],
				threshold: Some(0.8),
				top_k: None,
			}),
			expression: Some(ExpressionFilter::Contains("test".to_string())),
			context: Some(ContextFilter::PathExists("/meta".to_string())),
			relations: Some(RelationFilter::HasRelations),
			temporal: Some(TemporalFilter::CreatedAfter(Utc::now())),
			limit: None,
			offset: 0,
			cursor: None,
			order: QueryOrder::default(),
			hybrid_weights: None,
			explain: true,
		};

		let explanation = storage.generate_explanation(&entry, &query, Some(0.85), None, None);

		assert!(explanation.contains("Semantic similarity"));
		assert!(explanation.contains("expression filter"));
		assert!(explanation.contains("context filter"));
		assert!(explanation.contains("temporal filter"));
		assert!(explanation.contains("relation filter"));
	}

	#[test]
	fn test_vector_codec_roundtrip() {
		let vector = vec![0.1_f32, 0.2, 0.3];
		let encoded = vector_codec::serialize(&vector).unwrap();
		let decoded: Vec<f32> = vector_codec::deserialize(&encoded).unwrap();

		assert_eq!(decoded, vector);
	}

	#[test]
	fn test_vector_codec_deserialize_invalid_bytes() {
		let bytes = vec![0_u8, 159, 146, 150];
		let result: Result<Vec<f32>, String> = vector_codec::deserialize(&bytes);

		assert!(result.is_err());
	}

	// ==================== Edge Cases ====================

	#[test]
	fn test_insert_and_retrieve_unicode_content() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Hello 世界 🌍")
			.with_context(serde_json::json!({"greeting": "你好"}));
		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.expression, "Hello 世界 🌍");
		assert_eq!(retrieved.context["greeting"], "你好");
	}

	#[test]
	fn test_insert_and_retrieve_empty_string() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "");
		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert!(retrieved.expression.is_empty());
	}

	#[test]
	fn test_insert_and_retrieve_null_context() {
		let mut storage = create_test_storage();
		let entry = create_test_entry(vec![0.1], "Test");
		// Context defaults to Null
		storage.insert(&entry).unwrap();

		let retrieved = storage.get(entry.id).unwrap();
		assert_eq!(retrieved.context, serde_json::Value::Null);
	}

	#[test]
	fn test_query_no_matches() {
		let mut storage = create_test_storage();
		storage
			.insert(&create_test_entry(vec![0.1], "Entry"))
			.unwrap();

		let query =
			Query::new().with_expression(ExpressionFilter::Equals("Nonexistent".to_string()));
		let results = storage.query(&query).unwrap();

		assert!(results.is_empty());
	}

	#[test]
	fn test_count_after_operations() {
		let mut storage = create_test_storage();
		assert_eq!(storage.count().unwrap(), 0);

		let entry1 = create_test_entry(vec![0.1], "Entry 1");
		let entry2 = create_test_entry(vec![0.2], "Entry 2");

		storage.insert(&entry1).unwrap();
		assert_eq!(storage.count().unwrap(), 1);

		storage.insert(&entry2).unwrap();
		assert_eq!(storage.count().unwrap(), 2);

		storage.delete(entry1.id).unwrap();
		assert_eq!(storage.count().unwrap(), 1);

		storage.delete(entry2.id).unwrap();
		assert_eq!(storage.count().unwrap(), 0);
	}
}
