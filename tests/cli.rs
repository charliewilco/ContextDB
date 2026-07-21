#![cfg(feature = "cli")]

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use chrono::{Duration, Utc};
use contextdb::{ContextDB, Entry};
use predicates::prelude::*;
use tempfile::TempDir;

fn temp_db_path() -> (TempDir, std::path::PathBuf) {
	let temp_dir = TempDir::new().expect("tempdir created");
	let db_path = temp_dir.path().join("contextdb.db");
	(temp_dir, db_path)
}

fn cmd_bin() -> Command {
	let mut cmd = cargo_bin_cmd!("contextdb");
	cmd.env("NO_COLOR", "1").env("CLICOLOR", "0");
	cmd
}

#[test]
fn cli_init_creates_db() {
	let (_temp_dir, db_path) = temp_db_path();

	cmd_bin()
		.arg("init")
		.arg(&db_path)
		.assert()
		.success()
		.stdout(predicate::str::contains("Created database"));

	assert!(db_path.exists(), "database file should exist");
}

#[test]
fn cli_list_shows_entries_plain() {
	let (_temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	let entry = Entry::new(vec![0.1, 0.2, 0.3], "CLI list entry".to_string());
	let entry_id_prefix = entry.id.to_string()[..8].to_string();
	db.insert(&entry).expect("entry inserted");

	cmd_bin()
		.arg("list")
		.arg(&db_path)
		.args(["--limit", "5", "--format", "plain"])
		.assert()
		.success()
		.stdout(predicate::str::contains(&entry_id_prefix))
		.stdout(predicate::str::contains("CLI list entry"));
}

#[test]
fn cli_search_finds_entries_json() {
	let (_temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	let entry = Entry::new(vec![0.3, 0.4, 0.5], "Searchable onion note".to_string());
	let entry_id = entry.id.to_string();
	db.insert(&entry).expect("entry inserted");

	cmd_bin()
		.arg("search")
		.arg(&db_path)
		.arg("onion")
		.args(["--format", "json"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Searchable onion note"))
		.stdout(predicate::str::contains(&entry_id));
}

#[test]
fn cli_show_accepts_partial_id() {
	let (_temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	let entry = Entry::new(vec![0.9, 0.1, 0.2], "Partial ID entry".to_string());
	let entry_id = entry.id.to_string();
	let entry_prefix = entry_id[..8].to_string();
	db.insert(&entry).expect("entry inserted");

	cmd_bin()
		.arg("show")
		.arg(&db_path)
		.arg(&entry_prefix)
		.assert()
		.success()
		.stdout(predicate::str::contains("Entry Details"))
		.stdout(predicate::str::contains(&entry_id))
		.stdout(predicate::str::contains("Partial ID entry"));
}

#[test]
fn cli_add_inserts_an_entry() {
	let (_temp_dir, db_path) = temp_db_path();
	ContextDB::new(&db_path).expect("db created");

	cmd_bin()
		.arg("add")
		.arg(&db_path)
		.args([
			"--expression",
			"Added from CLI",
			"--meaning",
			"0.1,0.2,0.3",
			"--context",
			r#"{"source":"cli"}"#,
		])
		.assert()
		.success()
		.stdout(predicate::str::contains("Added entry"));

	let db = ContextDB::new(&db_path).expect("db reopened");
	let results = db.query(&contextdb::Query::new()).expect("query succeeds");
	assert_eq!(results.len(), 1);
	assert_eq!(results[0].entry.expression, "Added from CLI");
	assert_eq!(results[0].entry.context["source"], "cli");
}

#[test]
fn cli_list_applies_offset_after_stable_ordering() {
	let (_temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	let base = Utc::now();
	for (index, expression) in ["first", "second", "third"].into_iter().enumerate() {
		let mut entry = Entry::new(vec![index as f32], expression.to_string());
		entry.created_at = base + Duration::seconds(index as i64);
		entry.updated_at = entry.created_at;
		db.insert(&entry).expect("entry inserted");
	}

	cmd_bin()
		.arg("list")
		.arg(&db_path)
		.args(["--offset", "1", "--limit", "1", "--format", "plain"])
		.assert()
		.success()
		.stdout(predicate::str::contains("second"))
		.stdout(predicate::str::contains("first").not())
		.stdout(predicate::str::contains("third").not());
}

#[test]
fn cli_list_handles_long_unicode_expression() {
	let (_temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	db.insert(&Entry::new(vec![0.1], "🧠".repeat(80)))
		.expect("entry inserted");

	cmd_bin().arg("list").arg(&db_path).assert().success();
}

#[test]
fn cli_delete_missing_database_does_not_create_it() {
	let (_temp_dir, db_path) = temp_db_path();

	cmd_bin()
		.arg("delete")
		.arg(&db_path)
		.arg(uuid::Uuid::new_v4().to_string())
		.arg("--force")
		.assert()
		.failure()
		.stderr(predicate::str::contains("Database not found"));
	assert!(!db_path.exists());
}

#[test]
fn cli_import_rolls_back_the_entire_batch() {
	let (temp_dir, db_path) = temp_db_path();
	let mut db = ContextDB::new(&db_path).expect("db created");
	let existing = Entry::new(vec![0.1], "existing".to_string());
	db.insert(&existing).expect("existing entry inserted");
	let new_entry = Entry::new(vec![0.2], "new".to_string());
	let input = temp_dir.path().join("entries.json");
	std::fs::write(
		&input,
		serde_json::to_string(&vec![new_entry, existing]).expect("entries serialize"),
	)
	.expect("input written");

	cmd_bin()
		.arg("import")
		.arg(&db_path)
		.arg(&input)
		.assert()
		.failure();

	let db = ContextDB::new(&db_path).expect("db reopened");
	assert_eq!(db.count().expect("count succeeds"), 1);
}

#[test]
fn cli_failed_import_does_not_create_destination() {
	let (temp_dir, db_path) = temp_db_path();
	let entry = Entry::new(vec![0.2], "duplicate".to_string());
	let input = temp_dir.path().join("invalid-entries.json");
	std::fs::write(
		&input,
		serde_json::to_string(&vec![entry.clone(), entry]).expect("entries serialize"),
	)
	.expect("input written");

	cmd_bin()
		.arg("import")
		.arg(&db_path)
		.arg(&input)
		.assert()
		.failure();

	assert!(
		!db_path.exists(),
		"failed import must not create destination"
	);
}

#[test]
fn cli_check_backup_and_restore_round_trip() {
	let (temp_dir, db_path) = temp_db_path();
	let backup_path = temp_dir.path().join("backup.db");
	let restored_path = temp_dir.path().join("restored.db");
	let mut db = ContextDB::new(&db_path).expect("db created");
	db.insert(&Entry::new(vec![0.1], "Persistent".to_string()))
		.expect("entry inserted");

	cmd_bin()
		.arg("check")
		.arg(&db_path)
		.assert()
		.success()
		.stdout(predicate::str::contains("integrity check passed"));
	cmd_bin()
		.arg("backup")
		.arg(&db_path)
		.arg(&backup_path)
		.assert()
		.success();
	cmd_bin()
		.arg("restore")
		.arg(&backup_path)
		.arg(&restored_path)
		.assert()
		.success();

	let restored = ContextDB::new(&restored_path).expect("restored db opens");
	assert_eq!(restored.count().expect("count succeeds"), 1);
}

#[test]
fn cli_profile_and_revisions_are_inspectable() {
	let (_temp_dir, db_path) = temp_db_path();
	ContextDB::new(&db_path).expect("db created");

	cmd_bin()
		.arg("profile")
		.arg(&db_path)
		.args([
			"--model",
			"test-model",
			"--version",
			"v1",
			"--dimensions",
			"2",
		])
		.assert()
		.success();
	cmd_bin()
		.arg("add")
		.arg(&db_path)
		.args(["--expression", "Versioned", "--meaning", "0.1,0.2"])
		.assert()
		.success();
	let db = ContextDB::new(&db_path).expect("db reopened");
	let id = db.query(&contextdb::Query::new()).unwrap()[0].entry.id;

	cmd_bin()
		.arg("profile")
		.arg(&db_path)
		.assert()
		.success()
		.stdout(predicate::str::contains("test-model"));
	cmd_bin()
		.arg("revisions")
		.arg(&db_path)
		.arg(id.to_string())
		.assert()
		.success()
		.stdout(predicate::str::contains("Insert"));
}
