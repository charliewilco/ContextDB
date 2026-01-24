use contextdb::{ContextDB, ContextFilter, Entry, ExpressionFilter, Query};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("=== ContextDB Demo ===\n");

	// Create an in-memory database
	let mut db = ContextDB::in_memory()?;

	// Example 1: Dietary Preferences (the grocery app use case)
	println!("1. Inserting dietary preferences...");

	let entries = vec![
		Entry::new(
			vec![0.8, 0.1, 0.3], // Mock embedding for "doesn't like red onion"
			"User doesn't like red onions".to_string(),
		)
		.with_context(json!({
			"category": "dietary",
			"specificity": "item-level",
			"ingredient": "red onion",
			"sentiment": "dislike"
		})),
		Entry::new(
			vec![0.7, 0.2, 0.2],
			"User loves caramelized onions".to_string(),
		)
		.with_context(json!({
			"category": "dietary",
			"specificity": "preparation-level",
			"ingredient": "onion",
			"sentiment": "love",
			"preparation": "caramelized"
		})),
		Entry::new(vec![0.9, 0.1, 0.1], "User is gluten free".to_string()).with_context(json!({
			"category": "dietary",
			"specificity": "diet-level",
			"restriction": "gluten-free"
		})),
		Entry::new(vec![0.4, 0.6, 0.5], "User allergic to peanuts".to_string()).with_context(
			json!({
				"category": "dietary",
				"specificity": "allergy",
				"allergen": "peanuts",
				"severity": "high"
			}),
		),
	];

	for entry in &entries {
		db.insert(entry)?;
	}

	println!("✓ Inserted {} dietary preferences\n", entries.len());

	// Example 2: Semantic search (what an LLM would do)
	println!("2. LLM querying: Find preferences about onions");
	let onion_query = Query::new()
		.with_meaning(vec![0.75, 0.15, 0.25], Some(0.6)) // Mock query vector
		.with_limit(3)
		.with_explanation();

	let onion_results = db.query(&onion_query)?;
	for result in &onion_results {
		println!(
			"  → {} (similarity: {:.1}%)",
			result.entry.expression,
			result.similarity_score.unwrap() * 100.0
		);
		if let Some(exp) = &result.explanation {
			println!("    {}", exp);
		}
	}
	println!();

	// Example 3: Human inspection - text search
	println!("3. Human querying: Show all preferences containing 'onion'");
	let text_query = Query::new().with_expression(ExpressionFilter::Contains("onion".to_string()));

	let text_results = db.query(&text_query)?;
	for result in &text_results {
		println!("  → {}", result.entry.expression);
		if let Some(ctx) = result.entry.context.as_object() {
			if let Some(sentiment) = ctx.get("sentiment") {
				println!("    Sentiment: {}", sentiment);
			}
		}
	}
	println!();

	// Example 4: Structured context queries
	println!("4. Human querying: Show all high-severity dietary restrictions");
	let severity_query = Query::new().with_context(ContextFilter::PathEquals(
		"/severity".to_string(),
		json!("high"),
	));

	let severity_results = db.query(&severity_query)?;
	for result in &severity_results {
		println!("  → {}", result.entry.expression);
	}
	println!();

	// Example 5: Combined query (semantic + metadata)
	println!("5. Hybrid query: Dietary preferences that are item-level specific");
	let hybrid_query = Query::new()
		.with_context(ContextFilter::PathEquals(
			"/specificity".to_string(),
			json!("item-level"),
		))
		.with_expression(ExpressionFilter::Contains("like".to_string()));

	let hybrid_results = db.query(&hybrid_query)?;
	for result in &hybrid_results {
		println!("  → {}", result.entry.expression);
	}
	println!();

	// Example 6: Add more entries with temporal context
	println!("6. Adding work-related memories...");
	let work_entries = vec![
		Entry::new(
			vec![0.2, 0.8, 0.4],
			"User prefers TypeScript over JavaScript".to_string(),
		)
		.with_context(json!({
			"category": "work",
			"domain": "programming",
			"language": "typescript"
		})),
		Entry::new(
			vec![0.3, 0.7, 0.5],
			"User has 10 years of TypeScript experience".to_string(),
		)
		.with_context(json!({
			"category": "work",
			"domain": "experience",
			"technology": "typescript",
			"years": 10
		})),
	];

	for entry in &work_entries {
		db.insert(entry)?;
	}
	println!("✓ Inserted {} work memories\n", work_entries.len());

	// Example 7: Category filtering
	println!("7. Show all work-related memories");
	let work_query = Query::new().with_context(ContextFilter::PathEquals(
		"/category".to_string(),
		json!("work"),
	));

	let work_results = db.query(&work_query)?;
	for result in &work_results {
		println!("  → {}", result.entry.expression);
	}
	println!();

	// Example 8: Summary stats
	println!("=== Database Stats ===");
	println!("Total entries: {}", db.count()?);

	let dietary_count = db
		.query(&Query::new().with_context(ContextFilter::PathEquals(
			"/category".to_string(),
			json!("dietary"),
		)))?
		.len();

	let work_count = db
		.query(&Query::new().with_context(ContextFilter::PathEquals(
			"/category".to_string(),
			json!("work"),
		)))?
		.len();

	println!("  - Dietary preferences: {}", dietary_count);
	println!("  - Work memories: {}", work_count);
	println!();

	println!("=== Key Features Demonstrated ===");
	println!("✓ Dual representation: vectors (LLM) + text (human)");
	println!("✓ Semantic search by vector similarity");
	println!("✓ Text-based search on expressions");
	println!("✓ Structured metadata queries");
	println!("✓ Hybrid queries combining multiple filters");
	println!("✓ Explainable results");

	Ok(())
}
