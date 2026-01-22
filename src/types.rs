use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The fundamental unit of ContextDB: an entry with both semantic meaning and human expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
	/// Unique identifier
	pub id: Uuid,

	/// Semantic representation (vector embedding)
	pub meaning: Vec<f32>,

	/// Human-readable form of the entry
	pub expression: String,

	/// Flexible metadata for domain-specific information
	pub context: serde_json::Value,

	/// When this entry was created
	pub created_at: DateTime<Utc>,

	/// When this entry was last updated
	pub updated_at: DateTime<Utc>,

	/// IDs of related entries (for graph relationships)
	pub relations: Vec<Uuid>,
}

impl Entry {
	/// Create a new entry with the given meaning and expression
	pub fn new(meaning: Vec<f32>, expression: String) -> Self {
		let now = Utc::now();
		Self {
			id: Uuid::new_v4(),
			meaning,
			expression,
			context: serde_json::Value::Null,
			created_at: now,
			updated_at: now,
			relations: Vec::new(),
		}
	}

	/// Create an entry with additional context metadata
	pub fn with_context(mut self, context: serde_json::Value) -> Self {
		self.context = context;
		self
	}

	/// Add a relation to another entry
	pub fn add_relation(mut self, entry_id: Uuid) -> Self {
		if !self.relations.contains(&entry_id) {
			self.relations.push(entry_id);
			self.updated_at = Utc::now();
		}
		self
	}

	/// Calculate cosine similarity with another entry's meaning
	pub fn similarity(&self, other: &Entry) -> f32 {
		cosine_similarity(&self.meaning, &other.meaning)
	}
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
	if a.len() != b.len() {
		return 0.0;
	}

	let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
	let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
	let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

	if magnitude_a == 0.0 || magnitude_b == 0.0 {
		return 0.0;
	}

	dot_product / (magnitude_a * magnitude_b)
}

#[cfg(test)]
mod tests {
	use super::*;

	// ==================== Cosine Similarity Tests ====================

	#[test]
	fn test_cosine_similarity_identical_vectors() {
		let a = vec![1.0, 0.0, 0.0];
		let b = vec![1.0, 0.0, 0.0];
		assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_orthogonal_vectors() {
		let a = vec![1.0, 0.0, 0.0];
		let b = vec![0.0, 1.0, 0.0];
		assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_opposite_vectors() {
		let a = vec![1.0, 0.0, 0.0];
		let b = vec![-1.0, 0.0, 0.0];
		assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_different_lengths() {
		let a = vec![1.0, 0.0, 0.0];
		let b = vec![1.0, 0.0];
		assert_eq!(cosine_similarity(&a, &b), 0.0);
	}

	#[test]
	fn test_cosine_similarity_empty_vectors() {
		let a: Vec<f32> = vec![];
		let b: Vec<f32> = vec![];
		assert_eq!(cosine_similarity(&a, &b), 0.0);
	}

	#[test]
	fn test_cosine_similarity_zero_magnitude_vector() {
		let a = vec![0.0, 0.0, 0.0];
		let b = vec![1.0, 0.0, 0.0];
		assert_eq!(cosine_similarity(&a, &b), 0.0);
	}

	#[test]
	fn test_cosine_similarity_both_zero_magnitude() {
		let a = vec![0.0, 0.0, 0.0];
		let b = vec![0.0, 0.0, 0.0];
		assert_eq!(cosine_similarity(&a, &b), 0.0);
	}

	#[test]
	fn test_cosine_similarity_partial_overlap() {
		// 45-degree angle should give ~0.707
		let a = vec![1.0, 0.0];
		let b = vec![1.0, 1.0];
		let expected = 1.0 / (2.0_f32).sqrt(); // cos(45°) ≈ 0.707
		assert!((cosine_similarity(&a, &b) - expected).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_scaled_vectors() {
		// Scaling shouldn't affect cosine similarity
		let a = vec![1.0, 2.0, 3.0];
		let b = vec![2.0, 4.0, 6.0]; // 2x scale of a
		assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_negative_values() {
		let a = vec![-1.0, -2.0, 3.0];
		let b = vec![1.0, 2.0, -3.0];
		// Opposite directions
		assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 0.001);
	}

	#[test]
	fn test_cosine_similarity_high_dimensional() {
		let a: Vec<f32> = (0..128).map(|i| i as f32).collect();
		let b: Vec<f32> = (0..128).map(|i| i as f32).collect();
		assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
	}

	// ==================== Entry Creation Tests ====================

	#[test]
	fn test_entry_creation() {
		let entry = Entry::new(vec![0.1, 0.2, 0.3], "Test entry".to_string());

		assert_eq!(entry.expression, "Test entry");
		assert_eq!(entry.meaning.len(), 3);
		assert_eq!(entry.context, serde_json::Value::Null);
		assert!(entry.relations.is_empty());
	}

	#[test]
	fn test_entry_has_unique_id() {
		let entry1 = Entry::new(vec![0.1], "Entry 1".to_string());
		let entry2 = Entry::new(vec![0.1], "Entry 2".to_string());
		assert_ne!(entry1.id, entry2.id);
	}

	#[test]
	fn test_entry_timestamps_initialized() {
		let before = Utc::now();
		let entry = Entry::new(vec![0.1], "Test".to_string());
		let after = Utc::now();

		assert!(entry.created_at >= before && entry.created_at <= after);
		assert!(entry.updated_at >= before && entry.updated_at <= after);
		assert_eq!(entry.created_at, entry.updated_at);
	}

	#[test]
	fn test_entry_with_empty_expression() {
		let entry = Entry::new(vec![0.1], String::new());
		assert!(entry.expression.is_empty());
	}

	#[test]
	fn test_entry_with_empty_meaning() {
		let entry = Entry::new(vec![], "No embedding".to_string());
		assert!(entry.meaning.is_empty());
	}

	// ==================== Entry Context Tests ====================

	#[test]
	fn test_entry_with_context() {
		let context = serde_json::json!({
			"source": "user",
			"priority": 1
		});

		let entry = Entry::new(vec![0.1], "Test".to_string()).with_context(context.clone());

		assert_eq!(entry.context, context);
	}

	#[test]
	fn test_entry_context_can_be_replaced() {
		let context1 = serde_json::json!({"version": 1});
		let context2 = serde_json::json!({"version": 2});

		let entry = Entry::new(vec![0.1], "Test".to_string())
			.with_context(context1)
			.with_context(context2.clone());

		assert_eq!(entry.context, context2);
	}

	#[test]
	fn test_entry_with_nested_context() {
		let context = serde_json::json!({
			"metadata": {
				"tags": ["tag1", "tag2"],
				"nested": {
					"deep": true
				}
			}
		});

		let entry = Entry::new(vec![0.1], "Test".to_string()).with_context(context.clone());

		assert_eq!(entry.context["metadata"]["tags"][0], "tag1");
		assert_eq!(entry.context["metadata"]["nested"]["deep"], true);
	}

	// ==================== Entry Relations Tests ====================

	#[test]
	fn test_entry_add_relation() {
		let other_id = Uuid::new_v4();
		let entry = Entry::new(vec![0.1], "Test".to_string()).add_relation(other_id);

		assert_eq!(entry.relations.len(), 1);
		assert!(entry.relations.contains(&other_id));
	}

	#[test]
	fn test_entry_add_multiple_relations() {
		let id1 = Uuid::new_v4();
		let id2 = Uuid::new_v4();
		let id3 = Uuid::new_v4();

		let entry = Entry::new(vec![0.1], "Test".to_string())
			.add_relation(id1)
			.add_relation(id2)
			.add_relation(id3);

		assert_eq!(entry.relations.len(), 3);
		assert!(entry.relations.contains(&id1));
		assert!(entry.relations.contains(&id2));
		assert!(entry.relations.contains(&id3));
	}

	#[test]
	fn test_entry_add_duplicate_relation_ignored() {
		let other_id = Uuid::new_v4();
		let entry = Entry::new(vec![0.1], "Test".to_string())
			.add_relation(other_id)
			.add_relation(other_id);

		assert_eq!(entry.relations.len(), 1);
	}

	#[test]
	fn test_entry_add_relation_updates_timestamp() {
		let entry = Entry::new(vec![0.1], "Test".to_string());
		let original_updated = entry.updated_at;

		// Small delay to ensure timestamp difference
		std::thread::sleep(std::time::Duration::from_millis(10));

		let other_id = Uuid::new_v4();
		let entry = entry.add_relation(other_id);

		assert!(entry.updated_at > original_updated);
	}

	#[test]
	fn test_entry_add_duplicate_relation_no_timestamp_update() {
		let other_id = Uuid::new_v4();
		let entry = Entry::new(vec![0.1], "Test".to_string()).add_relation(other_id);
		let updated_at_after_first = entry.updated_at;

		// Small delay
		std::thread::sleep(std::time::Duration::from_millis(10));

		let entry = entry.add_relation(other_id);

		// Timestamp should not change for duplicate
		assert_eq!(entry.updated_at, updated_at_after_first);
	}

	// ==================== Entry Similarity Tests ====================

	#[test]
	fn test_entry_similarity_identical() {
		let entry1 = Entry::new(vec![1.0, 0.0, 0.0], "Entry 1".to_string());
		let entry2 = Entry::new(vec![1.0, 0.0, 0.0], "Entry 2".to_string());

		assert!((entry1.similarity(&entry2) - 1.0).abs() < 0.001);
	}

	#[test]
	fn test_entry_similarity_orthogonal() {
		let entry1 = Entry::new(vec![1.0, 0.0, 0.0], "Entry 1".to_string());
		let entry2 = Entry::new(vec![0.0, 1.0, 0.0], "Entry 2".to_string());

		assert!((entry1.similarity(&entry2) - 0.0).abs() < 0.001);
	}

	#[test]
	fn test_entry_similarity_is_symmetric() {
		let entry1 = Entry::new(vec![1.0, 2.0, 3.0], "Entry 1".to_string());
		let entry2 = Entry::new(vec![4.0, 5.0, 6.0], "Entry 2".to_string());

		assert!((entry1.similarity(&entry2) - entry2.similarity(&entry1)).abs() < 0.001);
	}

	// ==================== Serialization Tests ====================

	#[test]
	fn test_entry_serialization_roundtrip() {
		let context = serde_json::json!({"key": "value"});
		let entry = Entry::new(vec![0.1, 0.2, 0.3], "Test".to_string()).with_context(context);

		let json = serde_json::to_string(&entry).unwrap();
		let deserialized: Entry = serde_json::from_str(&json).unwrap();

		assert_eq!(entry.id, deserialized.id);
		assert_eq!(entry.meaning, deserialized.meaning);
		assert_eq!(entry.expression, deserialized.expression);
		assert_eq!(entry.context, deserialized.context);
	}

	#[test]
	fn test_entry_with_relations_serialization() {
		let relation_id = Uuid::new_v4();
		let entry = Entry::new(vec![0.1], "Test".to_string()).add_relation(relation_id);

		let json = serde_json::to_string(&entry).unwrap();
		let deserialized: Entry = serde_json::from_str(&json).unwrap();

		assert_eq!(entry.relations, deserialized.relations);
	}
}
