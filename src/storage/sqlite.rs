use crate::query::{
	ContextFilter, ExpressionFilter, Query, QueryResult, RelationFilter, TemporalFilter,
};
use crate::storage::{StorageBackend, StorageError, StorageResult};
use crate::types::Entry;
use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use uuid::Uuid;

/// SQLite-backed storage for ContextDB entries
pub struct SqliteStorage {
	conn: Connection,
}

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
		let mut storage = Self { conn };
		storage.initialize()?;
		Ok(storage)
	}

	/// Initialize the database schema
	fn initialize(&mut self) -> StorageResult<()> {
		self.conn
			.execute_batch(
				r#"
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
                FOREIGN KEY (from_id) REFERENCES entries(id),
                FOREIGN KEY (to_id) REFERENCES entries(id)
            );
            
            CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);
            CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(updated_at);
            CREATE INDEX IF NOT EXISTS idx_entries_expression ON entries(expression);
            CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_id);
            CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_id);
            "#,
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;
		Ok(())
	}

	/// Get all entries from the database
	fn get_all_entries(&self) -> StorageResult<Vec<Entry>> {
		let mut stmt = self
			.conn
			.prepare("SELECT id FROM entries")
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let entry_ids: Vec<Uuid> = stmt
			.query_map([], |row| {
				let id_str: String = row.get(0)?;
				Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)
			})
			.map_err(|e| StorageError::Database(e.to_string()))?
			.filter_map(Result::ok)
			.collect();

		entry_ids.iter().map(|id| self.get(*id)).collect()
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
			adjacency.entry(to_id).or_default().push(from_id);
			related_ids.insert(from_id);
			related_ids.insert(to_id);
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
	) -> String {
		let mut parts = Vec::new();

		if let Some(score) = similarity_score {
			parts.push(format!("Semantic similarity: {:.2}%", score * 100.0));
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

	fn get_entries_by_ids(&self, ids: &HashSet<Uuid>) -> StorageResult<Vec<Entry>> {
		let mut entries = Vec::with_capacity(ids.len());
		for id in ids {
			entries.push(self.get(*id)?);
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
				self.query_ids_with_params(
					"SELECT id FROM entries WHERE INSTR(expression, ?1) > 0",
					rusqlite::params![value],
				)
			}
		}
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

	fn query_relation_ids(&self, filter: &RelationFilter) -> StorageResult<HashSet<Uuid>> {
		match filter {
			RelationFilter::DirectlyRelatedTo(id) => {
				let id_str = id.to_string();
				self.query_ids_with_params(
					"SELECT to_id AS id FROM relations WHERE from_id = ?1
                     UNION
                     SELECT from_id AS id FROM relations WHERE to_id = ?1",
					rusqlite::params![id_str],
				)
			}
			RelationFilter::WithinDistance { from, max_hops } => {
				let index = self.load_relation_index()?;
				Ok(self.within_distance_relations(&index, *from, *max_hops))
			}
			RelationFilter::HasRelations => self.query_ids_with_params(
				"SELECT from_id AS id FROM relations
                 UNION
                 SELECT to_id AS id FROM relations",
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
}

impl StorageBackend for SqliteStorage {
	fn insert(&mut self, entry: &Entry) -> StorageResult<()> {
		let id = entry.id.to_string();
		let meaning_bytes = bincode::serialize(&entry.meaning)
			.map_err(|e| StorageError::Database(format!("Failed to serialize vector: {}", e)))?;
		let context_json = serde_json::to_string(&entry.context)?;

		self.conn
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
			self.conn
				.execute(
					"INSERT OR IGNORE INTO relations (from_id, to_id) VALUES (?1, ?2)",
					params![id, relation_id.to_string()],
				)
				.map_err(|e| StorageError::Database(e.to_string()))?;
		}

		Ok(())
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
				let meaning: Vec<f32> = bincode::deserialize(&meaning_bytes)
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
			.map_err(|_| StorageError::NotFound(id))?;

		// Get relations
		let mut rel_stmt = self
			.conn
			.prepare("SELECT to_id FROM relations WHERE from_id = ?1")
			.map_err(|e| StorageError::Database(e.to_string()))?;

		let relations: Vec<Uuid> = rel_stmt
			.query_map(params![id_str], |row| {
				let to_id_str: String = row.get(0)?;
				Uuid::parse_str(&to_id_str).map_err(|_| rusqlite::Error::InvalidQuery)
			})
			.map_err(|e| StorageError::Database(e.to_string()))?
			.filter_map(Result::ok)
			.collect();

		Ok(Entry { relations, ..entry })
	}

	fn query(&self, query: &Query) -> StorageResult<Vec<QueryResult>> {
		let mut candidate_ids: Option<HashSet<Uuid>> = None;

		if let Some(ref expr_filter) = query.expression {
			let ids = self.query_expression_ids(expr_filter)?;
			candidate_ids = Some(match candidate_ids {
				Some(existing) => existing.intersection(&ids).copied().collect(),
				None => ids,
			});
		}

		if let Some(ref temporal_filter) = query.temporal {
			let ids = self.query_temporal_ids(temporal_filter)?;
			candidate_ids = Some(match candidate_ids {
				Some(existing) => existing.intersection(&ids).copied().collect(),
				None => ids,
			});
		}

		if let Some(ref relation_filter) = query.relations {
			let ids = self.query_relation_ids(relation_filter)?;
			candidate_ids = Some(match candidate_ids {
				Some(existing) => existing.intersection(&ids).copied().collect(),
				None => ids,
			});
		}

		if matches!(candidate_ids, Some(ref ids) if ids.is_empty()) {
			return Ok(Vec::new());
		}

		// Start with filtered entries if possible
		let mut results = match candidate_ids {
			Some(ref ids) => self.get_entries_by_ids(ids)?,
			None => self.get_all_entries()?,
		};

		let relation_index = if query.relations.is_some() {
			Some(self.load_relation_index()?)
		} else {
			None
		};

		// Apply semantic filter (vector similarity)
		if let Some(ref meaning_filter) = query.meaning {
			results.sort_by(|a, b| {
				let sim_a = crate::types::cosine_similarity(&a.meaning, &meaning_filter.vector);
				let sim_b = crate::types::cosine_similarity(&b.meaning, &meaning_filter.vector);
				sim_b.partial_cmp(&sim_a).unwrap()
			});

			if let Some(threshold) = meaning_filter.threshold {
				results.retain(|e| {
					crate::types::cosine_similarity(&e.meaning, &meaning_filter.vector) >= threshold
				});
			}

			if let Some(top_k) = meaning_filter.top_k {
				results.truncate(top_k);
			}
		}

		// Apply expression filter
		if let Some(ref expr_filter) = query.expression {
			let mut filtered = Vec::with_capacity(results.len());
			for entry in results {
				if self.matches_expression(&entry.expression, expr_filter)? {
					filtered.push(entry);
				}
			}
			results = filtered;
		}

		// Apply context filter
		if let Some(ref ctx_filter) = query.context {
			results.retain(|e| self.matches_context(&e.context, ctx_filter));
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

		// Apply limit
		if let Some(limit) = query.limit {
			results.truncate(limit);
		}

		// Convert to QueryResults
		let query_results: Vec<QueryResult> = results
			.into_iter()
			.map(|entry| {
				let similarity_score = query
					.meaning
					.as_ref()
					.map(|m| crate::types::cosine_similarity(&entry.meaning, &m.vector));

				let explanation = if query.explain {
					Some(self.generate_explanation(&entry, query, similarity_score))
				} else {
					None
				};

				QueryResult {
					entry,
					similarity_score,
					explanation,
				}
			})
			.collect();

		Ok(query_results)
	}

	fn update(&mut self, entry: &Entry) -> StorageResult<()> {
		let id = entry.id.to_string();
		let meaning_bytes = bincode::serialize(&entry.meaning)
			.map_err(|e| StorageError::Database(format!("Failed to serialize vector: {}", e)))?;
		let context_json = serde_json::to_string(&entry.context)?;

		self.conn
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

		// Update relations (delete old, insert new)
		self.conn
			.execute("DELETE FROM relations WHERE from_id = ?1", params![id])
			.map_err(|e| StorageError::Database(e.to_string()))?;

		for relation_id in &entry.relations {
			self.conn
				.execute(
					"INSERT OR IGNORE INTO relations (from_id, to_id) VALUES (?1, ?2)",
					params![id, relation_id.to_string()],
				)
				.map_err(|e| StorageError::Database(e.to_string()))?;
		}

		Ok(())
	}

	fn delete(&mut self, id: Uuid) -> StorageResult<()> {
		let id_str = id.to_string();

		// Delete relations first
		self.conn
			.execute(
				"DELETE FROM relations WHERE from_id = ?1 OR to_id = ?1",
				params![id_str],
			)
			.map_err(|e| StorageError::Database(e.to_string()))?;

		// Delete entry
		let rows_affected = self
			.conn
			.execute("DELETE FROM entries WHERE id = ?1", params![id_str])
			.map_err(|e| StorageError::Database(e.to_string()))?;

		if rows_affected == 0 {
			return Err(StorageError::NotFound(id));
		}

		Ok(())
	}

	fn count(&self) -> StorageResult<usize> {
		let count: i64 = self
			.conn
			.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
			.map_err(|e| StorageError::Database(e.to_string()))?;
		Ok(count as usize)
	}

	fn backend_name(&self) -> &str {
		"SQLite"
	}
}

struct RelationIndex {
	adjacency: HashMap<Uuid, Vec<Uuid>>,
	related_ids: HashSet<Uuid>,
}

// Simple bincode serialize/deserialize for vectors
mod bincode {
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
		assert!(result.is_ok());
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
			explain: false,
		};
		let results_has = storage.query(&query_has).unwrap();
		let has_ids: HashSet<Uuid> = results_has.into_iter().map(|r| r.entry.id).collect();
		assert!(has_ids.contains(&entry1.id));
		assert!(has_ids.contains(&entry2.id));
		assert!(has_ids.contains(&entry3.id));
		assert!(!has_ids.contains(&entry4.id));

		let query_none = Query {
			meaning: None,
			expression: None,
			context: None,
			relations: Some(RelationFilter::NoRelations),
			temporal: None,
			limit: None,
			explain: false,
		};
		let results_none = storage.query(&query_none).unwrap();
		assert_eq!(results_none.len(), 1);
		assert_eq!(results_none[0].entry.id, entry4.id);
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

		let explanation = results[0].explanation.as_ref().unwrap();
		assert!(explanation.contains("Semantic similarity"));
		assert!(explanation.contains("expression filter"));
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
			explain: true,
		};

		let explanation = storage.generate_explanation(&entry, &query, Some(0.85));

		assert!(explanation.contains("Semantic similarity"));
		assert!(explanation.contains("expression filter"));
		assert!(explanation.contains("context filter"));
		assert!(explanation.contains("temporal filter"));
		assert!(explanation.contains("relation filter"));
	}

	#[test]
	fn test_bincode_roundtrip() {
		let vector = vec![0.1_f32, 0.2, 0.3];
		let encoded = bincode::serialize(&vector).unwrap();
		let decoded: Vec<f32> = bincode::deserialize(&encoded).unwrap();

		assert_eq!(decoded, vector);
	}

	#[test]
	fn test_bincode_deserialize_invalid_bytes() {
		let bytes = vec![0_u8, 159, 146, 150];
		let result: Result<Vec<f32>, String> = bincode::deserialize(&bytes);

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
