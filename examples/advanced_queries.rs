use chrono::{Duration, Utc};
use contextdb::{
	ContextDB, ContextFilter, Entry, ExpressionFilter, MeaningFilter, Query, TemporalFilter,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("=== ContextDB Advanced Queries Example ===\n");

	let mut db = ContextDB::in_memory()?;

	let mut older = Entry::new(vec![0.1, 0.2, 0.3], "User likes cold brew".to_string())
		.with_context(json!({"category": "dietary", "confidence": 0.9}));
	older.created_at = Utc::now() - Duration::days(10);
	older.updated_at = older.created_at;

	let recent = Entry::new(vec![0.12, 0.22, 0.32], "User switched to green tea".to_string())
		.with_context(json!({"category": "dietary", "confidence": 0.8}));

	let work = Entry::new(vec![0.8, 0.1, 0.1], "User prefers TypeScript".to_string())
		.with_context(json!({"category": "work"}));

	db.insert(&older)?;
	db.insert(&recent)?;
	db.insert(&work)?;

	println!("Top-k semantic results for beverages:");
	let semantic_query = Query {
		meaning: Some(MeaningFilter {
			vector: vec![0.11, 0.21, 0.31],
			threshold: Some(0.7),
			top_k: Some(2),
		}),
		..Query::new()
	};

	for result in db.query(&semantic_query)? {
		println!("  - {}", result.entry.expression);
	}

	println!("\nRecent dietary entries (last 3 days):");
	let recent_query = Query::new()
		.with_context(ContextFilter::PathEquals(
			"/category".to_string(),
			json!("dietary"),
		))
		.with_temporal(TemporalFilter::CreatedAfter(Utc::now() - Duration::days(3)));

	for result in db.query(&recent_query)? {
		println!("  - {}", result.entry.expression);
	}

	println!("\nHybrid: text + context filter:");
	let hybrid_query = Query::new()
		.with_expression(ExpressionFilter::Contains("prefer".to_string()))
		.with_context(ContextFilter::PathEquals(
			"/category".to_string(),
			json!("work"),
		));

	for result in db.query(&hybrid_query)? {
		println!("  - {}", result.entry.expression);
	}

	Ok(())
}
