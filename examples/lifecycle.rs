use chrono::Utc;
use contextdb::{ContextDB, Entry};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("=== ContextDB Entry Lifecycle Example ===\n");

	let mut db = ContextDB::in_memory()?;

	let entry = Entry::new(vec![0.2, 0.3, 0.4], "Initial note".to_string())
		.with_context(json!({"category": "note", "status": "draft"}));

	db.insert(&entry)?;
	println!("Inserted: {}", entry.id);

	let mut updated = db.get(entry.id)?;
	updated.expression = "Revised note".to_string();
	updated.context = json!({"category": "note", "status": "published"});
	updated.updated_at = Utc::now();

	db.update(&updated)?;
	println!("Updated: {}", updated.id);

	let fetched = db.get(entry.id)?;
	println!("Fetched: {}", fetched.expression);

	db.delete(entry.id)?;
	println!("Deleted: {}", entry.id);

	Ok(())
}
