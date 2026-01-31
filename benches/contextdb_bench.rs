use contextdb::{ContextDB, ContextFilter, Entry, ExpressionFilter, Query};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use serde_json::json;

const DIMENSIONS: usize = 128;
const INSERT_COUNT: usize = 1_000;
const QUERY_COUNT: usize = 5_000;

fn make_vector(seed: usize, dim: usize) -> Vec<f32> {
	(0..dim)
		.map(|i| {
			let base = (seed.wrapping_add(i * 13) % 101) as f32;
			base / 101.0
		})
		.collect()
}

fn make_entry(index: usize, dim: usize) -> Entry {
	let expression = if index % 10 == 0 {
		format!("alpha entry {index}")
	} else {
		format!("beta entry {index}")
	};
	let context = if index % 2 == 0 {
		json!({"group": "a", "index": index})
	} else {
		json!({"group": "b", "index": index})
	};

	Entry::new(make_vector(index, dim), expression).with_context(context)
}

fn build_entries(count: usize, dim: usize) -> Vec<Entry> {
	(0..count).map(|i| make_entry(i, dim)).collect()
}

fn populate_db(count: usize, dim: usize) -> ContextDB {
	let mut db = ContextDB::in_memory().expect("in-memory db");
	let entries = build_entries(count, dim);
	for entry in &entries {
		db.insert(entry).expect("insert entry");
	}
	db
}

fn bench_insert_batch(c: &mut Criterion) {
	let entries = build_entries(INSERT_COUNT, DIMENSIONS);
	c.bench_function("insert_1k", |b| {
		b.iter_batched(
			|| ContextDB::in_memory().expect("in-memory db"),
			|mut db| {
				for entry in &entries {
					db.insert(entry).expect("insert entry");
				}
				black_box(db.count().expect("count entries"));
			},
			BatchSize::LargeInput,
		);
	});
}

fn bench_query_meaning(c: &mut Criterion) {
	let db = populate_db(QUERY_COUNT, DIMENSIONS);
	let query_vector = make_vector(QUERY_COUNT / 2, DIMENSIONS);
	let query = Query::new()
		.with_meaning(query_vector, Some(0.8))
		.with_limit(50);

	c.bench_function("query_meaning_5k", |b| {
		b.iter(|| {
			let results = db.query(&query).expect("query results");
			black_box(results.len());
		});
	});
}

fn bench_query_expression(c: &mut Criterion) {
	let db = populate_db(QUERY_COUNT, DIMENSIONS);
	let query = Query::new().with_expression(ExpressionFilter::Contains("alpha".to_string()));

	c.bench_function("query_expression_5k", |b| {
		b.iter(|| {
			let results = db.query(&query).expect("query results");
			black_box(results.len());
		});
	});
}

fn bench_query_context(c: &mut Criterion) {
	let db = populate_db(QUERY_COUNT, DIMENSIONS);
	let query =
		Query::new().with_context(ContextFilter::PathEquals("/group".to_string(), json!("a")));

	c.bench_function("query_context_5k", |b| {
		b.iter(|| {
			let results = db.query(&query).expect("query results");
			black_box(results.len());
		});
	});
}

criterion_group!(
	benches,
	bench_insert_batch,
	bench_query_meaning,
	bench_query_expression,
	bench_query_context
);
criterion_main!(benches);
