use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BackupEntry {
    pub path: String,
    pub size: u64,
    pub mtime_secs: u64,
    pub sha256: Option<String>,
    pub backed_up_at: u64,
    pub archive_id: Option<String>,
}

pub struct BackupCatalog {
    conn: Connection,
    db_path: PathBuf,
}

impl BackupCatalog {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open catalog DB at {}", db_path.display()))?;

        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to enable WAL mode")?;

        let mut catalog = Self { conn, db_path };
        catalog.init_schema().context("Failed to initialize schema")?;
        Ok(catalog)
    }

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

    pub fn should_skip_file(&self, file_path: impl AsRef<Path>) -> Result<Option<bool>> {
        let path_str = normalize_path(file_path.as_ref());

        let metadata = fs::metadata(file_path.as_ref()).context("Failed to read file metadata")?;
        let current_size = metadata.len();
        let current_mtime = get_mtime_secs(&metadata)?;

        let entry: Option<(u64, u64)> = self
            .conn
            .query_row(
                "SELECT size, mtime_secs FROM backed_up_files WHERE path = ?1",
                params![&path_str],
                |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
            )
            .optional()
            .context("Failed to query catalog")?;

        Ok(entry.map(|(cat_size, cat_mtime)| cat_size == current_size && cat_mtime == current_mtime))
    }

    pub fn filter_files_to_backup(&self, file_paths: Vec<PathBuf>) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
        let mut skip = Vec::new();
        let mut backup = Vec::new();

        for path in file_paths {
            match self.should_skip_file(&path) {
                Ok(Some(true)) => skip.push(path),
                Ok(Some(false)) => backup.push(path),
                Ok(None) => backup.push(path),
                Err(e) => {
                    eprintln!("Warning: Failed to check {}: {}", path.display(), e);
                    skip.push(path);
                }
            }
        }

        Ok((skip, backup))
    }

    pub fn list_all(&self) -> Result<Vec<BackupEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, size, mtime_secs, sha256, backed_up_at, archive_id FROM backed_up_files ORDER BY backed_up_at DESC")
            .context("Failed to prepare query")?;

        let entries = stmt
            .query_map([], |row| {
                Ok(BackupEntry {
                    path: row.get(0)?,
                    size: row.get::<_, i64>(1)? as u64,
                    mtime_secs: row.get::<_, i64>(2)? as u64,
                    sha256: row.get(3)?,
                    backed_up_at: row.get::<_, i64>(4)? as u64,
                    archive_id: row.get(5)?,
                })
            })
            .context("Failed to execute query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect results")?;

        Ok(entries)
    }

    pub fn export_json(&self, output_path: impl AsRef<Path>) -> Result<()> {
        let entries = self.list_all()?;
        let json = serde_json::to_string_pretty(&entries).context("Failed to serialize to JSON")?;
        fs::write(output_path.as_ref(), json)
            .with_context(|| format!("Failed to write JSON to {}", output_path.as_ref().display()))?;
        Ok(())
    }

    pub fn get_connection(&self) -> &Connection {
        &self.conn
    }

    pub fn get_connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

pub fn normalize_path(path: &Path) -> String {
    let mut s = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        s = s.replace('\\', "/").to_lowercase();
    }
    s
}

fn get_mtime_secs(metadata: &fs::Metadata) -> Result<u64> {
    metadata
        .modified()
        .context("Failed to get modification time")?
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("Failed to compute duration since UNIX_EPOCH")
        .map(|d| d.as_secs())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_files_to_backup() -> Result<()> {
        let db_file = tempfile::NamedTempFile::new()?;
        let mut catalog = BackupCatalog::new(db_file.path())?;

        let temp_dir = tempfile::TempDir::new()?;

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        fs::write(&file1, b"data1")?;
        fs::write(&file2, b"data2")?;

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

        let files = vec![file1.clone(), file2.clone()];
        let (skip, backup) = catalog.filter_files_to_backup(files)?;

        assert_eq!(skip.len(), 1);
        assert_eq!(backup.len(), 1);
        assert_eq!(skip[0], file1);
        assert_eq!(backup[0], file2);

        Ok(())
    }
}
