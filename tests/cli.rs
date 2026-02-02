#![cfg(feature = "cli")]

use assert_cmd::Command;
use contextdb::{ContextDB, Entry};
use predicates::prelude::*;
use tempfile::TempDir;

fn temp_db_path() -> (TempDir, std::path::PathBuf) {
	let temp_dir = TempDir::new().expect("tempdir created");
	let db_path = temp_dir.path().join("contextdb.db");
	(temp_dir, db_path)
}

fn cmd_bin() -> Command {
	let mut cmd = Command::cargo_bin("contextdb").expect("binary built");
	cmd.env("NO_COLOR", "1").env("CLICOLOR", "0");
	cmd
}

#[test]
fn cli_init_creates_db() {
	let (_temp_dir, db_path) = temp_db_path();

	cmd_bin()
		.args(["init", db_path.to_str().unwrap()])
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
		.args([
			"list",
			db_path.to_str().unwrap(),
			"--limit",
			"5",
			"--format",
			"plain",
		])
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
		.args([
			"search",
			db_path.to_str().unwrap(),
			"onion",
			"--format",
			"json",
		])
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
		.args(["show", db_path.to_str().unwrap(), &entry_prefix])
		.assert()
		.success()
		.stdout(predicate::str::contains("Entry Details"))
		.stdout(predicate::str::contains(&entry_id))
		.stdout(predicate::str::contains("Partial ID entry"));
}
