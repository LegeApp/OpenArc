use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A single file entry in the backup catalog (what was already archived).
#[derive(Clone, Debug)]
pub struct BackupEntry {
    /// Normalized file path (for comparison across runs).
    pub path: String,
    /// File size in bytes (quick check for "did it change?").
    pub size: u64,
    /// Modification time as seconds since UNIX_EPOCH (mtime).
    pub mtime_secs: u64,
    /// Optional SHA-256 hash of file contents (for stronger verification).
    pub sha256: Option<String>,
    /// Timestamp when this file was last backed up (seconds since UNIX_EPOCH).
    pub backed_up_at: u64,
    /// Which archive (filename or ID) this file is stored in (optional, for tracking).
    pub archive_id: Option<String>,
}

/// Manages the SQLite catalog of backed-up files.
pub struct BackupCatalog {
    conn: Connection,
    db_path: PathBuf,
}

impl BackupCatalog {
    /// Open or create a catalog at `db_path`.
    /// If the file doesn't exist, a fresh database is created.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open catalog DB at {}", db_path.display()))?;

        // Enable Write-Ahead Logging for robustness.
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to enable WAL mode")?;

        let mut catalog = Self { conn, db_path };
        catalog.init_schema().context("Failed to initialize schema")?;
        Ok(catalog)
    }

    /// Initialize the schema if it doesn't already exist.
    fn init_schema(&mut self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS backed_up_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                size INTEGER NOT NULL,
                mtime_secs INTEGER NOT NULL,
                sha256 TEXT,
                backed_up_at INTEGER NOT NULL,
                archive_id TEXT
            );
            
            CREATE INDEX IF NOT EXISTS idx_path ON backed_up_files (path);
            CREATE INDEX IF NOT EXISTS idx_backed_up_at ON backed_up_files (backed_up_at);
        "#,
            )
            .context("Failed to create schema")?;
        Ok(())
    }

    /// Record a file as backed up. Overwrites if it already exists.
    pub fn record_backup(&mut self, entry: BackupEntry) -> Result<()> {
        let now = now_secs();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO backed_up_files 
                 (path, size, mtime_secs, sha256, backed_up_at, archive_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &entry.path,
                    entry.size as i64,
                    entry.mtime_secs as i64,
                    &entry.sha256,
                    now as i64,
                    &entry.archive_id,
                ],
            )
            .context("Failed to record backup entry")?;
        Ok(())
    }

    /// Record multiple files as backed up in a single transaction.
    pub fn record_backups(&mut self, entries: Vec<BackupEntry>) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("Failed to start transaction")?;
        let now = now_secs();

        for entry in entries {
            tx.execute(
                "INSERT OR REPLACE INTO backed_up_files 
                 (path, size, mtime_secs, sha256, backed_up_at, archive_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &entry.path,
                    entry.size as i64,
                    entry.mtime_secs as i64,
                    &entry.sha256,
                    now as i64,
                    &entry.archive_id,
                ],
            )
            .context("Failed to record backup entry")?;
        }

        tx.commit().context("Failed to commit transaction")?;
        Ok(())
    }

    /// Check if a file has already been backed up and matches current state.
    /// Returns:
    /// - `None` if not in catalog (always backup).
    /// - `Some(true)` if in catalog and unchanged (skip backup).
    /// - `Some(false)` if in catalog but changed (backup again).
    pub fn should_skip_file(&self, file_path: impl AsRef<Path>) -> Result<Option<bool>> {
        let path_str = normalize_path(file_path.as_ref());

        // Get the on-disk file metadata.
        let metadata = fs::metadata(file_path.as_ref()).context("Failed to read file metadata")?;
        let current_size = metadata.len();
        let current_mtime = get_mtime_secs(&metadata)?;

        // Look up in catalog.
        let entry: Option<(u64, u64)> = self
            .conn
            .query_row(
                "SELECT size, mtime_secs FROM backed_up_files WHERE path = ?1",
                params![&path_str],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)),
            )
            .optional()
            .context("Failed to query catalog")?;

        Ok(entry.map(|(cat_size, cat_mtime)| {
            // If size and mtime both match, skip this file (it hasn't changed).
            cat_size == current_size && cat_mtime == current_mtime
        }))
    }

    /// Batch check multiple files and return two lists: (skip, backup).
    /// Files are grouped by whether they should be skipped or backed up.
    pub fn filter_files_to_backup(&self, file_paths: Vec<PathBuf>) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
        let mut skip = Vec::new();
        let mut backup = Vec::new();

        for path in file_paths {
            match self.should_skip_file(&path) {
                Ok(Some(true)) => skip.push(path),
                Ok(Some(false)) => backup.push(path),
                Ok(None) => backup.push(path), // New file, must backup
                Err(e) => {
                    // On error (e.g., file deleted, unreadable), skip it and log.
                    eprintln!("Warning: Failed to check {}: {}", path.display(), e);
                    skip.push(path);
                }
            }
        }

        Ok((skip, backup))
    }

    /// Get all entries from the catalog for inspection/debugging.
    pub fn list_all(&self) -> Result<Vec<BackupEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, size, mtime_secs, sha256, backed_up_at, archive_id FROM backed_up_files ORDER BY backed_up_at DESC")
            .context("Failed to prepare query")?;

        let entries = stmt
            .query_map([], |row| {
                Ok(BackupEntry {
                    path: row.get(0)?,
                    size: row.get::<_, u64>(1)?,
                    mtime_secs: row.get::<_, u64>(2)?,
                    sha256: row.get(3)?,
                    backed_up_at: row.get::<_, u64>(4)?,
                    archive_id: row.get(5)?,
                })
            })
            .context("Failed to execute query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect results")?;

        Ok(entries)
    }

    /// Get entries backed up since a certain time (seconds since UNIX_EPOCH).
    pub fn list_since(&self, since_secs: u64) -> Result<Vec<BackupEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT path, size, mtime_secs, sha256, backed_up_at, archive_id 
                 FROM backed_up_files 
                 WHERE backed_up_at >= ?1 
                 ORDER BY backed_up_at DESC",
            )
            .context("Failed to prepare query")?;

        let entries = stmt
            .query_map(params![since_secs as i64], |row| {
                Ok(BackupEntry {
                    path: row.get(0)?,
                    size: row.get::<_, u64>(1)?,
                    mtime_secs: row.get::<_, u64>(2)?,
                    sha256: row.get(3)?,
                    backed_up_at: row.get::<_, u64>(4)?,
                    archive_id: row.get(5)?,
                })
            })
            .context("Failed to execute query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect results")?;

        Ok(entries)
    }

    /// Remove a file from the catalog (e.g., if it's been deleted and you want to re-backup later).
    pub fn remove_entry(&mut self, file_path: impl AsRef<Path>) -> Result<()> {
        let path_str = normalize_path(file_path.as_ref());
        self.conn
            .execute("DELETE FROM backed_up_files WHERE path = ?1", params![&path_str])
            .context("Failed to delete entry")?;
        Ok(())
    }

    /// Clear the entire catalog (dangerous; use with care).
    pub fn clear_all(&mut self) -> Result<()> {
        self.conn
            .execute("DELETE FROM backed_up_files", [])
            .context("Failed to clear catalog")?;
        Ok(())
    }

    /// Export the catalog to a JSON file (for debugging/audit).
    pub fn export_json(&self, output_path: impl AsRef<Path>) -> Result<()> {
        let entries = self.list_all()?;
        let json = serde_json::to_string_pretty(&entries)
            .context("Failed to serialize to JSON")?;
        fs::write(output_path.as_ref(), json)
            .with_context(|| format!("Failed to write JSON to {}", output_path.as_ref().display()))?;
        Ok(())
    }
}

/// Normalize a file path for cross-platform consistency.
/// Converts to lowercase on case-insensitive systems (Windows) for reliable matching.
fn normalize_path(path: &Path) -> String {
    let mut s = path.to_string_lossy().to_string();
    // On Windows, normalize to forward slashes and lowercase.
    #[cfg(target_os = "windows")]
    {
        s = s.replace('\\', "/").to_lowercase();
    }
    s
}

/// Extract mtime as seconds since UNIX_EPOCH.
fn get_mtime_secs(metadata: &fs::Metadata) -> Result<u64> {
    metadata
        .modified()
        .context("Failed to get modification time")?
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("Failed to compute duration since UNIX_EPOCH")
        .map(|d| d.as_secs())
}

/// Get current time as seconds since UNIX_EPOCH.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_catalog_create_and_record() -> Result<()> {
        let db_file = tempfile::NamedTempFile::new()?;
        let mut catalog = BackupCatalog::new(db_file.path())?;

        let entry = BackupEntry {
            path: "photos/vacation.jpg".to_string(),
            size: 2_048_000,
            mtime_secs: 1700000000,
            sha256: Some("abc123".to_string()),
            backed_up_at: now_secs(),
            archive_id: Some("backup_001.oarc".to_string()),
        };

        catalog.record_backup(entry.clone())?;
        let all = catalog.list_all()?;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].path, "photos/vacation.jpg");

        Ok(())
    }

    #[test]
    fn test_should_skip_file() -> Result<()> {
        let db_file = tempfile::NamedTempFile::new()?;
        let mut catalog = BackupCatalog::new(db_file.path())?;

        // Create a temporary test file.
        let test_file = NamedTempFile::new()?;
        let test_path = test_file.path();
        fs::write(test_path, b"test data")?;

        let metadata = fs::metadata(test_path)?;
        let size = metadata.len();
        let mtime = get_mtime_secs(&metadata)?;

        let entry = BackupEntry {
            path: normalize_path(test_path),
            size,
            mtime_secs: mtime,
            sha256: None,
            backed_up_at: now_secs(),
            archive_id: None,
        };

        catalog.record_backup(entry)?;

        // Should skip (unchanged).
        let should_skip = catalog.should_skip_file(test_path)?;
        assert_eq!(should_skip, Some(true));

        Ok(())
    }

    #[test]
    fn test_filter_files_to_backup() -> Result<()> {
        let db_file = tempfile::NamedTempFile::new()?;
        let mut catalog = BackupCatalog::new(db_file.path())?;

        let temp_dir = tempfile::TempDir::new()?;

        // Create two test files.
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        fs::write(&file1, b"data1")?;
        fs::write(&file2, b"data2")?;

        // Record file1 as already backed up.
        let metadata1 = fs::metadata(&file1)?;
        let entry1 = BackupEntry {
            path: normalize_path(&file1),
            size: metadata1.len(),
            mtime_secs: get_mtime_secs(&metadata1)?,
            sha256: None,
            backed_up_at: now_secs(),
            archive_id: None,
        };
        catalog.record_backup(entry1)?;

        // Filter: file1 should skip, file2 should backup.
        let files = vec![file1.clone(), file2.clone()];
        let (skip, backup) = catalog.filter_files_to_backup(files)?;

        assert_eq!(skip.len(), 1);
        assert_eq!(backup.len(), 1);
        assert_eq!(skip[0], file1);
        assert_eq!(backup[0], file2);

        Ok(())
    }
}
