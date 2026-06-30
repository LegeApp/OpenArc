//! File-level tracking & deduplication system
//!
//! Central SQLite DB in AppData tracking every file processed by OpenArc.
//! Duplicates are logged (not skipped). Per-run log files saved.
//! Batch DB queries only (before/after processing, never per-file).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Returns the platform-specific OpenArc data directory.
pub fn openarc_data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        // %APPDATA%/OpenArc
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("OpenArc")
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/OpenArc")
    } else {
        // Linux / other
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/share/openarc")
    }
}

/// A record of a previously processed file found in the database.
#[derive(Debug, Clone)]
pub struct DuplicateRecord {
    pub file_name: String,
    pub file_hash: String,
    pub file_size: i64,
    pub processed_at: String,
    pub run_id: String,
    pub archive_name: Option<String>,
    pub processing_mode: String,
}

/// A record to insert after processing.
#[derive(Debug, Clone)]
pub struct ProcessedFileRecord {
    pub file_name: String,
    pub file_hash: String,
    pub file_size: i64,
    pub processed_at: String,
    pub run_id: String,
    pub archive_name: Option<String>,
    pub archive_hash: Option<String>,
    pub output_path: String,
    pub processing_mode: String,
}

/// Central file tracker backed by SQLite.
pub struct FileTracker {
    conn: Connection,
    run_id: String,
}

impl FileTracker {
    /// Open or create the tracking database.
    pub fn new() -> Result<Self> {
        let data_dir = openarc_data_dir();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data dir: {}", data_dir.display()))?;

        let db_path = data_dir.join("tracking.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open tracking DB: {}", db_path.display()))?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS processed_files (
                id INTEGER PRIMARY KEY,
                file_name TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                processed_at TEXT NOT NULL,
                run_id TEXT NOT NULL,
                archive_name TEXT,
                archive_hash TEXT,
                output_path TEXT,
                processing_mode TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_file_hash ON processed_files(file_hash);",
        )?;

        let run_id = generate_run_id();

        Ok(Self { conn, run_id })
    }

    /// Get the run ID for this session.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Batch lookup: given a slice of SHA-256 hashes, return all previously
    /// processed records grouped by hash.
    pub fn find_duplicates(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, Vec<DuplicateRecord>>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result: HashMap<String, Vec<DuplicateRecord>> = HashMap::new();

        // Process in chunks to stay within SQLite variable limits
        for chunk in hashes.chunks(500) {
            let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT file_name, file_hash, file_size, processed_at, run_id, archive_name, processing_mode
                 FROM processed_files WHERE file_hash IN ({})",
                placeholders
            );

            let mut stmt = self.conn.prepare(&sql)?;

            let params: Vec<&dyn rusqlite::types::ToSql> = chunk
                .iter()
                .map(|h| h as &dyn rusqlite::types::ToSql)
                .collect();

            let rows = stmt.query_map(params.as_slice(), |row| {
                Ok(DuplicateRecord {
                    file_name: row.get(0)?,
                    file_hash: row.get(1)?,
                    file_size: row.get(2)?,
                    processed_at: row.get(3)?,
                    run_id: row.get(4)?,
                    archive_name: row.get(5)?,
                    processing_mode: row.get(6)?,
                })
            })?;

            for row in rows {
                let record = row?;
                result
                    .entry(record.file_hash.clone())
                    .or_default()
                    .push(record);
            }
        }

        Ok(result)
    }

    /// Batch insert processed file records.
    pub fn record_batch(&self, records: &[ProcessedFileRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let tx = self.conn.unchecked_transaction()?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO processed_files
                 (file_name, file_hash, file_size, processed_at, run_id, archive_name, archive_hash, output_path, processing_mode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            )?;

            for r in records {
                stmt.execute(params![
                    r.file_name,
                    r.file_hash,
                    r.file_size,
                    r.processed_at,
                    r.run_id,
                    r.archive_name,
                    r.archive_hash,
                    r.output_path,
                    r.processing_mode,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Generate a formatted run log string.
    pub fn generate_run_log(
        &self,
        duplicates: &HashMap<String, Vec<DuplicateRecord>>,
        processed_count: usize,
        processing_mode: &str,
    ) -> String {
        let mut log = String::new();
        let now = iso8601_now();

        log.push_str(&format!("OpenArc Run Log\n"));
        log.push_str(&format!("===============\n"));
        log.push_str(&format!("Run ID: {}\n", self.run_id));
        log.push_str(&format!("Date: {}\n", now));
        log.push_str(&format!("Mode: {}\n", processing_mode));
        log.push_str(&format!("Files processed: {}\n", processed_count));
        log.push_str(&format!("Duplicate files found: {}\n\n", duplicates.len()));

        if !duplicates.is_empty() {
            log.push_str("Duplicate Files\n");
            log.push_str("---------------\n");
            for (hash, records) in duplicates {
                log.push_str(&format!("Hash: {}\n", hash));
                for r in records {
                    log.push_str(&format!(
                        "  - {} (processed {} in run {}, mode: {})\n",
                        r.file_name, r.processed_at, r.run_id, r.processing_mode
                    ));
                    if let Some(ref archive) = r.archive_name {
                        log.push_str(&format!("    Archive: {}\n", archive));
                    }
                }
                log.push_str("\n");
            }
        }

        log
    }

    /// Write the run log to disk.
    pub fn write_run_log(&self, content: &str) -> Result<PathBuf> {
        let logs_dir = openarc_data_dir().join("logs");
        fs::create_dir_all(&logs_dir)?;

        let log_path = logs_dir.join(format!("run_{}.log", self.run_id));
        fs::write(&log_path, content)?;
        Ok(log_path)
    }

    /// Print a concise duplicate report to the console.
    pub fn print_duplicate_report(duplicates: &HashMap<String, Vec<DuplicateRecord>>) {
        if duplicates.is_empty() {
            return;
        }

        println!(
            "\n\x1b[93m⚠ Duplicate files detected ({} hashes matched previous runs):\x1b[0m",
            duplicates.len()
        );

        let mut shown = 0;
        for (hash, records) in duplicates {
            if shown >= 10 {
                let remaining = duplicates.len() - shown;
                println!("  ... and {} more duplicate groups", remaining);
                break;
            }
            let short_hash = &hash[..12.min(hash.len())];
            let names: Vec<&str> = records.iter().map(|r| r.file_name.as_str()).collect();
            println!(
                "  [{}...] previously processed as: {}",
                short_hash,
                names.join(", ")
            );
            shown += 1;
        }
        println!();
    }
}

/// Generate a simple unique run ID (timestamp + random suffix).
fn generate_run_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand_part: u32 = (ts as u32) ^ (std::process::id());
    format!("{:x}-{:04x}", ts, rand_part & 0xFFFF)
}

/// Return current time as ISO 8601 string (UTC).
pub fn iso8601_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple UTC breakdown
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since 1970-01-01 to Y-M-D (simplified)
    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
