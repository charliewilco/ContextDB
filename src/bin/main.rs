use clap::{Parser, Subcommand};
use colored::*;
use contextdb::{ContextDB, EmbeddingProfile, Entry, ExpressionFilter, Query, QueryOrder};
use dialoguer::{theme::ColorfulTheme, Input};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tabled::{settings::Style, Table, Tabled};

#[derive(Parser)]
#[command(name = "contextdb")]
#[command(author, version, about = "A semantic database for LLM applications", long_about = None)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Create a new database
	Init {
		/// Path to the database file
		#[arg(default_value = "contextdb.db")]
		path: PathBuf,
	},

	/// Add an entry to an existing database
	Add {
		/// Path to the database file
		path: PathBuf,

		/// Human-readable expression
		#[arg(short, long)]
		expression: String,

		/// Comma-delimited embedding values
		#[arg(short, long, value_delimiter = ',', num_args = 1..)]
		meaning: Vec<f32>,

		/// JSON context metadata
		#[arg(short, long)]
		context: Option<String>,

		/// Related entry UUIDs
		#[arg(short, long, value_delimiter = ',')]
		relation: Vec<uuid::Uuid>,
	},

	/// Show database statistics
	Stats {
		/// Path to the database file
		path: PathBuf,
	},

	/// Search entries by text
	Search {
		/// Path to the database file
		path: PathBuf,

		/// Search query (text to find)
		query: String,

		/// Maximum results to return
		#[arg(short, long, default_value = "10")]
		limit: usize,

		/// Output format (table, json, plain)
		#[arg(short, long, default_value = "table")]
		format: String,
	},

	/// List all entries
	List {
		/// Path to the database file
		path: PathBuf,

		/// Maximum entries to show
		#[arg(short, long, default_value = "20")]
		limit: usize,

		/// Offset for pagination
		#[arg(short, long, default_value = "0")]
		offset: usize,

		/// Output format (table, json, plain)
		#[arg(short, long, default_value = "table")]
		format: String,
	},

	/// Show details of a specific entry
	Show {
		/// Path to the database file
		path: PathBuf,

		/// Entry ID (UUID)
		id: String,
	},

	/// Export database to JSON
	Export {
		/// Path to the database file
		path: PathBuf,

		/// Output file (stdout if not specified)
		#[arg(short, long)]
		output: Option<PathBuf>,
	},

	/// Import entries from JSON
	Import {
		/// Path to the database file
		path: PathBuf,

		/// Input JSON file
		input: PathBuf,
	},

	/// Delete an entry
	Delete {
		/// Path to the database file
		path: PathBuf,

		/// Entry ID (UUID)
		id: String,

		/// Skip confirmation
		#[arg(short, long)]
		force: bool,
	},

	/// Interactive REPL mode
	Repl {
		/// Path to the database file
		path: PathBuf,
	},

	/// Show recent entries
	Recent {
		/// Path to the database file
		path: PathBuf,

		/// Number of recent entries
		#[arg(short, long, default_value = "10")]
		count: usize,
	},

	/// Check database and index integrity
	Check {
		/// Path to the database file
		path: PathBuf,
	},

	/// Create a consistent SQLite backup
	Backup {
		/// Path to the database file
		path: PathBuf,
		/// New backup file
		output: PathBuf,
	},

	/// Restore a backup into a new database path
	Restore {
		/// Backup file
		backup: PathBuf,
		/// New destination database
		destination: PathBuf,
	},

	/// Show or configure embedding identity
	Profile {
		/// Path to the database file
		path: PathBuf,
		/// Embedding model identifier
		#[arg(long, requires = "dimensions")]
		model: Option<String>,
		/// Optional model revision
		#[arg(long, requires = "model")]
		version: Option<String>,
		/// Embedding dimensions
		#[arg(long, requires = "model")]
		dimensions: Option<usize>,
	},

	/// Print durable revision history for an entry
	Revisions {
		/// Path to the database file
		path: PathBuf,
		/// Entry UUID or unique prefix
		id: String,
	},
}

#[derive(Tabled)]
struct EntryRow {
	#[tabled(rename = "ID")]
	id: String,
	#[tabled(rename = "Expression")]
	expression: String,
	#[tabled(rename = "Vector Dim")]
	vector_dim: usize,
	#[tabled(rename = "Relations")]
	relations: usize,
	#[tabled(rename = "Created")]
	created: String,
}

impl From<&Entry> for EntryRow {
	fn from(entry: &Entry) -> Self {
		let expression = truncate(&entry.expression, 50);

		Self {
			id: entry.id.to_string()[..8].to_string(),
			expression,
			vector_dim: entry.meaning.len(),
			relations: entry.relations.len(),
			created: entry.created_at.format("%Y-%m-%d %H:%M").to_string(),
		}
	}
}

fn main() {
	let cli = Cli::parse();

	let result = match cli.command {
		Commands::Init { path } => cmd_init(path),
		Commands::Add {
			path,
			expression,
			meaning,
			context,
			relation,
		} => cmd_add(path, expression, meaning, context, relation),
		Commands::Stats { path } => cmd_stats(path),
		Commands::Search {
			path,
			query,
			limit,
			format,
		} => cmd_search(path, query, limit, format),
		Commands::List {
			path,
			limit,
			offset,
			format,
		} => cmd_list(path, limit, offset, format),
		Commands::Show { path, id } => cmd_show(path, id),
		Commands::Export { path, output } => cmd_export(path, output),
		Commands::Import { path, input } => cmd_import(path, input),
		Commands::Delete { path, id, force } => cmd_delete(path, id, force),
		Commands::Repl { path } => cmd_repl(path),
		Commands::Recent { path, count } => cmd_recent(path, count),
		Commands::Check { path } => cmd_check(path),
		Commands::Backup { path, output } => cmd_backup(path, output),
		Commands::Restore {
			backup,
			destination,
		} => cmd_restore(backup, destination),
		Commands::Profile {
			path,
			model,
			version,
			dimensions,
		} => cmd_profile(path, model, version, dimensions),
		Commands::Revisions { path, id } => cmd_revisions(path, id),
	};

	if let Err(e) = result {
		eprintln!("{} {}", "Error:".red().bold(), e);
		std::process::exit(1);
	}
}

fn cmd_check(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let report = db.integrity_check()?;
	if report.is_healthy() {
		println!("{} Database integrity check passed", "✓".green().bold());
		return Ok(());
	}
	for issue in report.issues {
		eprintln!("{}: {}", issue.area, issue.message);
	}
	Err("Database integrity check failed".into())
}

fn cmd_backup(path: PathBuf, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	db.backup_to(&output)?;
	println!(
		"{} Backed up database to {}",
		"✓".green().bold(),
		output.display()
	);
	Ok(())
}

fn cmd_restore(backup: PathBuf, destination: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	if !backup.exists() {
		return Err(format!("Backup not found: {}", backup.display()).into());
	}
	let db = ContextDB::restore(&backup, &destination)?;
	let report = db.integrity_check()?;
	if !report.is_healthy() {
		return Err("Restored database failed its integrity check".into());
	}
	println!(
		"{} Restored database to {}",
		"✓".green().bold(),
		destination.display()
	);
	Ok(())
}

fn cmd_profile(
	path: PathBuf,
	model: Option<String>,
	version: Option<String>,
	dimensions: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut db = open_db(&path)?;
	if let Some(model) = model {
		let profile = EmbeddingProfile {
			model,
			version,
			dimensions: dimensions.ok_or("--dimensions is required with --model")?,
		};
		db.set_embedding_profile(&profile)?;
		println!("{} Embedding profile configured", "✓".green().bold());
		return Ok(());
	}
	match db.embedding_profile()? {
		Some(profile) => println!("{}", serde_json::to_string_pretty(&profile)?),
		None => println!("No embedding profile configured."),
	}
	Ok(())
}

fn cmd_revisions(path: PathBuf, id: String) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let id = match uuid::Uuid::parse_str(&id) {
		Ok(id) => id,
		Err(_) => find_entry_by_partial_id(&db, &id)?.id,
	};
	println!("{}", serde_json::to_string_pretty(&db.revisions(id)?)?);
	Ok(())
}

fn cmd_add(
	path: PathBuf,
	expression: String,
	meaning: Vec<f32>,
	context: Option<String>,
	relations: Vec<uuid::Uuid>,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut db = open_db(&path)?;
	let context = context
		.map(|value| serde_json::from_str(&value))
		.transpose()?
		.unwrap_or(serde_json::Value::Null);
	let mut entry = Entry::new(meaning, expression).with_context(context);
	for relation in relations {
		entry = entry.add_relation(relation);
	}
	db.insert(&entry)?;
	println!("{} Added entry {}", "✓".green().bold(), entry.id);
	Ok(())
}

fn cmd_init(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	if path.exists() {
		return Err(format!("Database already exists at {}", path.display()).into());
	}

	let _db = ContextDB::new(&path)?;
	println!(
		"{} Created database at {}",
		"✓".green().bold(),
		path.display()
	);
	Ok(())
}

fn cmd_stats(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let count = db.count()?;

	println!("{}", "Database Statistics".cyan().bold());
	println!("{}", "─".repeat(40));
	println!("  {} {}", "Path:".bold(), path.display());
	println!("  {} {}", "Backend:".bold(), db.backend_name());
	println!("  {} {}", "Entries:".bold(), count);

	if count > 0 {
		// Get sample to show vector dimensions
		let results = db.query(&Query::new().with_limit(1))?;
		if let Some(first) = results.first() {
			println!(
				"  {} {}",
				"Vector dimensions:".bold(),
				first.entry.meaning.len()
			);
		}
	}

	println!("{}", "─".repeat(40));
	Ok(())
}

fn cmd_search(
	path: PathBuf,
	query: String,
	limit: usize,
	format: String,
) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;

	let results = db.query(
		&Query::new()
			.with_expression(ExpressionFilter::Contains(query.clone()))
			.with_limit(limit),
	)?;

	if results.is_empty() {
		println!("{}", "No entries found.".yellow());
		return Ok(());
	}

	println!(
		"{} {} results for \"{}\"",
		"Found".green(),
		results.len(),
		query
	);
	println!();

	match format.as_str() {
		"json" => {
			let entries: Vec<&Entry> = results.iter().map(|r| &r.entry).collect();
			println!("{}", serde_json::to_string_pretty(&entries)?);
		}
		"plain" => {
			for result in &results {
				println!("{}", result.entry.id);
				println!("  {}", result.entry.expression);
				println!();
			}
		}
		_ => {
			let rows: Vec<EntryRow> = results.iter().map(|r| EntryRow::from(&r.entry)).collect();
			let table = Table::new(rows).with(Style::rounded()).to_string();
			println!("{}", table);
		}
	}

	Ok(())
}

fn cmd_list(
	path: PathBuf,
	limit: usize,
	offset: usize,
	format: String,
) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let total = db.count()?;

	let results = db.query(&Query::new().with_offset(offset).with_limit(limit))?;

	println!(
		"{} {} of {} entries",
		"Showing".cyan(),
		results.len(),
		total
	);
	println!();

	match format.as_str() {
		"json" => {
			let entries: Vec<&Entry> = results.iter().map(|r| &r.entry).collect();
			println!("{}", serde_json::to_string_pretty(&entries)?);
		}
		"plain" => {
			for result in &results {
				println!(
					"{} | {}",
					&result.entry.id.to_string()[..8],
					result.entry.expression
				);
			}
		}
		_ => {
			let rows: Vec<EntryRow> = results.iter().map(|r| EntryRow::from(&r.entry)).collect();
			let table = Table::new(rows).with(Style::rounded()).to_string();
			println!("{}", table);
		}
	}

	Ok(())
}

fn cmd_show(path: PathBuf, id: String) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;

	// Try to find entry by partial ID match
	let entry = find_entry_by_partial_id(&db, &id)?;

	println!("{}", "Entry Details".cyan().bold());
	println!("{}", "─".repeat(60));
	println!("  {} {}", "ID:".bold(), entry.id);
	println!("  {} {}", "Expression:".bold(), entry.expression);
	println!("  {} {} dimensions", "Meaning:".bold(), entry.meaning.len());
	println!("  {} {}", "Context:".bold(), entry.context);
	println!("  {} {}", "Created:".bold(), entry.created_at);
	println!("  {} {}", "Updated:".bold(), entry.updated_at);

	if !entry.relations.is_empty() {
		println!("  {}", "Relations:".bold());
		for rel_id in &entry.relations {
			println!("    - {}", rel_id);
		}
	}

	// Show vector preview
	if !entry.meaning.is_empty() {
		let preview: Vec<String> = entry
			.meaning
			.iter()
			.take(5)
			.map(|v| format!("{:.4}", v))
			.collect();
		let suffix = if entry.meaning.len() > 5 {
			format!("... ({} more)", entry.meaning.len() - 5)
		} else {
			String::new()
		};
		println!("  {} [{}] {}", "Vector:".bold(), preview.join(", "), suffix);
	}

	println!("{}", "─".repeat(60));
	Ok(())
}

fn cmd_export(path: PathBuf, output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let count = db.count()?;

	let pb = ProgressBar::new(count as u64);
	pb.set_style(
		ProgressStyle::default_bar()
			.template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")?
			.progress_chars("#>-"),
	);

	let results = db.query(&Query::new())?;
	let entries: Vec<&Entry> = results.iter().map(|r| &r.entry).collect();

	pb.finish_with_message("done");

	let json = serde_json::to_string_pretty(&entries)?;

	match output {
		Some(out_path) => {
			std::fs::write(&out_path, json)?;
			println!(
				"{} Exported {} entries to {}",
				"✓".green().bold(),
				entries.len(),
				out_path.display()
			);
		}
		None => {
			println!("{}", json);
		}
	}

	Ok(())
}

fn cmd_import(path: PathBuf, input: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let content = std::fs::read_to_string(&input)?;
	let entries: Vec<Entry> = serde_json::from_str(&content)?;

	let pb = ProgressBar::new(entries.len() as u64);
	pb.set_style(
		ProgressStyle::default_bar()
			.template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")?
			.progress_chars("#>-"),
	);

	if path.exists() {
		let mut db = ContextDB::new(&path)?;
		db.insert_batch(&entries)?;
	} else {
		import_into_new_database(&path, &entries)?;
		println!("{} Created database at {}", "→".blue(), path.display());
	}
	pb.set_position(entries.len() as u64);

	pb.finish_with_message("done");

	println!(
		"{} Imported {} of {} entries",
		"✓".green().bold(),
		entries.len(),
		entries.len()
	);

	Ok(())
}

fn import_into_new_database(
	path: &std::path::Path,
	entries: &[Entry],
) -> Result<(), Box<dyn std::error::Error>> {
	let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
	let file_name = path
		.file_name()
		.ok_or_else(|| format!("Invalid database path: {}", path.display()))?;
	let mut temporary_name = std::ffi::OsString::from(".");
	temporary_name.push(file_name);
	temporary_name.push(format!(".{}.importing", uuid::Uuid::new_v4()));
	let temporary_path = parent.join(temporary_name);

	let result = (|| -> Result<(), Box<dyn std::error::Error>> {
		let mut db = ContextDB::new(&temporary_path)?;
		db.insert_batch(entries)?;
		drop(db);
		std::fs::hard_link(&temporary_path, path)?;
		std::fs::remove_file(&temporary_path)?;
		Ok(())
	})();

	if result.is_err() {
		let _ = std::fs::remove_file(&temporary_path);
		for suffix in ["-wal", "-shm"] {
			let mut sidecar = temporary_path.as_os_str().to_os_string();
			sidecar.push(suffix);
			let _ = std::fs::remove_file(PathBuf::from(sidecar));
		}
	}

	result
}

fn cmd_delete(path: PathBuf, id: String, force: bool) -> Result<(), Box<dyn std::error::Error>> {
	let mut db = open_db(&path)?;

	let entry = find_entry_by_partial_id(&db, &id)?;

	if !force {
		println!("{}", "Entry to delete:".yellow().bold());
		println!("  ID: {}", entry.id);
		println!("  Expression: {}", entry.expression);
		println!();

		let confirm: String = Input::with_theme(&ColorfulTheme::default())
			.with_prompt("Type 'delete' to confirm")
			.interact_text()?;

		if confirm != "delete" {
			println!("{}", "Cancelled.".yellow());
			return Ok(());
		}
	}

	db.delete(entry.id)?;
	println!("{} Deleted entry {}", "✓".green().bold(), entry.id);

	Ok(())
}

fn cmd_recent(path: PathBuf, count: usize) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let results = db.query(
		&Query::new()
			.with_order(QueryOrder::CreatedAtDesc)
			.with_limit(count),
	)?;

	if results.is_empty() {
		println!("{}", "No entries found.".yellow());
		return Ok(());
	}

	println!("{} {} most recent entries", "Showing".cyan(), results.len());
	println!();

	let rows: Vec<EntryRow> = results.iter().map(|r| EntryRow::from(&r.entry)).collect();
	let table = Table::new(rows).with(Style::rounded()).to_string();
	println!("{}", table);

	Ok(())
}

fn cmd_repl(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	let db = open_db(&path)?;
	let count = db.count()?;

	println!("{}", "ContextDB REPL".cyan().bold());
	println!("Database: {} ({} entries)", path.display(), count);
	println!("Type 'help' for commands, 'quit' to exit");
	println!();

	loop {
		let input: String = Input::with_theme(&ColorfulTheme::default())
			.with_prompt("contextdb>")
			.allow_empty(true)
			.interact_text()?;

		let input = input.trim();
		if input.is_empty() {
			continue;
		}

		let parts: Vec<&str> = input.splitn(2, ' ').collect();
		let cmd = parts[0].to_lowercase();
		let args = parts.get(1).copied().unwrap_or("");

		match cmd.as_str() {
			"help" | "h" | "?" => {
				println!("{}", "Commands:".bold());
				println!("  search <query>  - Search entries by text");
				println!("  list [n]        - List entries (default: 10)");
				println!("  show <id>       - Show entry details");
				println!("  stats           - Show database statistics");
				println!("  recent [n]      - Show recent entries");
				println!("  quit | exit     - Exit REPL");
			}
			"quit" | "exit" | "q" => {
				println!("Goodbye!");
				break;
			}
			"stats" => {
				let count = db.count()?;
				println!("Entries: {}", count);
			}
			"list" | "ls" => {
				let limit: usize = args.parse().unwrap_or(10);
				let results = db.query(&Query::new().with_limit(limit))?;
				for result in &results {
					println!(
						"{} | {}",
						&result.entry.id.to_string()[..8],
						truncate(&result.entry.expression, 60)
					);
				}
			}
			"search" | "find" | "s" => {
				if args.is_empty() {
					println!("{}", "Usage: search <query>".yellow());
					continue;
				}
				let results = db.query(
					&Query::new()
						.with_expression(ExpressionFilter::Contains(args.to_string()))
						.with_limit(10),
				)?;
				if results.is_empty() {
					println!("{}", "No results.".yellow());
				} else {
					for result in &results {
						println!(
							"{} | {}",
							&result.entry.id.to_string()[..8],
							truncate(&result.entry.expression, 60)
						);
					}
				}
			}
			"show" => {
				if args.is_empty() {
					println!("{}", "Usage: show <id>".yellow());
					continue;
				}
				match find_entry_by_partial_id(&db, args) {
					Ok(entry) => {
						println!("ID: {}", entry.id);
						println!("Expression: {}", entry.expression);
						println!("Context: {}", entry.context);
						println!("Created: {}", entry.created_at);
						println!("Vector: {} dimensions", entry.meaning.len());
					}
					Err(e) => println!("{} {}", "Error:".red(), e),
				}
			}
			"recent" => {
				let count: usize = args.parse().unwrap_or(10);
				let mut results = db.query(&Query::new())?;
				results.sort_by(|a, b| b.entry.created_at.cmp(&a.entry.created_at));
				results.truncate(count);
				for result in &results {
					println!(
						"{} | {} | {}",
						&result.entry.id.to_string()[..8],
						result.entry.created_at.format("%m-%d %H:%M"),
						truncate(&result.entry.expression, 50)
					);
				}
			}
			_ => {
				println!("{} Unknown command: {}", "?".yellow(), cmd);
				println!("Type 'help' for available commands");
			}
		}
	}

	Ok(())
}

// Helper functions

fn open_db(path: &PathBuf) -> Result<ContextDB, Box<dyn std::error::Error>> {
	if !path.exists() {
		return Err(format!("Database not found: {}", path.display()).into());
	}
	Ok(ContextDB::new(path)?)
}

fn find_entry_by_partial_id(
	db: &ContextDB,
	partial_id: &str,
) -> Result<Entry, Box<dyn std::error::Error>> {
	// Try full UUID first
	if let Ok(uuid) = uuid::Uuid::parse_str(partial_id) {
		return db.get(uuid).map_err(|e| e.into());
	}

	// Try partial match
	let results = db.query(&Query::new())?;
	let matches: Vec<_> = results
		.iter()
		.filter(|r| r.entry.id.to_string().starts_with(partial_id))
		.collect();

	match matches.len() {
		0 => Err(format!("No entry found matching '{}'", partial_id).into()),
		1 => Ok(matches[0].entry.clone()),
		n => {
			println!(
				"{} {} entries match '{}':",
				"Ambiguous:".yellow(),
				n,
				partial_id
			);
			for m in &matches {
				println!(
					"  {} - {}",
					&m.entry.id.to_string()[..12],
					truncate(&m.entry.expression, 40)
				);
			}
			Err("Please provide a more specific ID".into())
		}
	}
}

fn truncate(s: &str, max_len: usize) -> String {
	if s.chars().count() <= max_len {
		s.to_string()
	} else if max_len <= 3 {
		".".repeat(max_len)
	} else {
		format!("{}...", s.chars().take(max_len - 3).collect::<String>())
	}
}
