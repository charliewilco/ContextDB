use contextdb::{ContextDB, Entry, Query, RelationFilter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("=== ContextDB Relations Example ===\n");

	let mut db = ContextDB::in_memory()?;

	let root = Entry::new(vec![0.1, 0.1, 0.1], "Project kickoff note".to_string());
	let child = Entry::new(vec![0.2, 0.2, 0.2], "Decision: use Rust".to_string());
	let grandchild = Entry::new(vec![0.3, 0.3, 0.3], "Follow-up: add benchmarks".to_string());
	let orphan = Entry::new(vec![0.4, 0.4, 0.4], "Unlinked note".to_string());

	let root = root.add_relation(child.id);
	let child = child.add_relation(grandchild.id);

	db.insert(&root)?;
	db.insert(&child)?;
	db.insert(&grandchild)?;
	db.insert(&orphan)?;

	println!("Directly related to root:");
	let direct_query = Query {
		relations: Some(RelationFilter::DirectlyRelatedTo(root.id)),
		..Query::new()
	};
	for result in db.query(&direct_query)? {
		println!("  - {}", result.entry.expression);
	}

	println!("\nWithin 2 hops of root:");
	let hops_query = Query {
		relations: Some(RelationFilter::WithinDistance {
			from: root.id,
			max_hops: 2,
		}),
		..Query::new()
	};
	for result in db.query(&hops_query)? {
		println!("  - {}", result.entry.expression);
	}

	println!("\nEntries with no relations:");
	let orphan_query = Query {
		relations: Some(RelationFilter::NoRelations),
		..Query::new()
	};
	for result in db.query(&orphan_query)? {
		println!("  - {}", result.entry.expression);
	}

	Ok(())
}
