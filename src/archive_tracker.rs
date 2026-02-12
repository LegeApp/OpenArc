use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArchiveRecord {
    pub id: Option<i64>, // None when inserting new records
    pub archive_path: String,
    pub archive_size: u64,
    pub creation_date: u64,
    pub original_location: String,
    pub destination_location: Option<String>,
    pub description: Option<String>,
    pub file_count: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArchiveFileMapping {
    pub id: Option<i64>, // None when inserting new records
    pub archive_id: i64,
    pub file_path: String,
    pub original_path: String,
    pub file_size: u64,
    pub archived_at: u64,
}

pub struct ArchiveTracker<'a> {
    conn: &'a mut Connection,
}

impl<'a> ArchiveTracker<'a> {
    pub fn new(connection: &'a mut Connection) -> Result<Self> {
        let tracker = Self { conn: connection };
        tracker.init_schema().context("Failed to initialize schema")?;
        Ok(tracker)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            -- Table to track created archives
            CREATE TABLE IF NOT EXISTS archives (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                archive_path TEXT NOT NULL,
                archive_size INTEGER NOT NULL,
                creation_date INTEGER NOT NULL,
                original_location TEXT NOT NULL,
                destination_location TEXT,
                description TEXT,
                file_count INTEGER NOT NULL DEFAULT 0
            );

            -- Index for faster lookups by archive path
            CREATE INDEX IF NOT EXISTS idx_archives_path ON archives (archive_path);

            -- Index for faster lookups by creation date
            CREATE INDEX IF NOT EXISTS idx_archives_creation_date ON archives (creation_date);

            -- Table to map files to archives
            CREATE TABLE IF NOT EXISTS archive_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                archive_id INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                original_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                archived_at INTEGER NOT NULL,
                FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE
            );

            -- Index for faster lookups by archive_id
            CREATE INDEX IF NOT EXISTS idx_archive_files_archive_id ON archive_files (archive_id);

            -- Index for faster lookups by file_path
            CREATE INDEX IF NOT EXISTS idx_archive_files_path ON archive_files (file_path);
        "#,
            )
            .context("Failed to create schema")?;
        Ok(())
    }

    pub fn record_archive(&mut self, mut record: ArchiveRecord) -> Result<i64> {
        let now = now_secs();
        
        // Insert the archive record
        let archive_id = self.conn.query_row(
            "INSERT INTO archives 
             (archive_path, archive_size, creation_date, original_location, destination_location, description, file_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             RETURNING id",
            params![
                &record.archive_path,
                record.archive_size as i64,
                now as i64,
                &record.original_location,
                &record.destination_location,
                &record.description,
                record.file_count as i32,
            ],
            |row| row.get(0),
        ).context("Failed to insert archive record")?;

        // Update the record with the assigned ID
        record.id = Some(archive_id);

        Ok(archive_id)
    }

    pub fn record_archive_files(&mut self, archive_id: i64, files: Vec<ArchiveFileMapping>) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .context("Failed to start transaction")?;
            
        let now = now_secs();

        for mut file_mapping in files {
            tx.execute(
                "INSERT INTO archive_files 
                 (archive_id, file_path, original_path, file_size, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    archive_id,
                    &file_mapping.file_path,
                    &file_mapping.original_path,
                    file_mapping.file_size as i64,
                    now as i64,
                ],
            )
            .context("Failed to insert archive file mapping")?;
        }

        tx.commit().context("Failed to commit transaction")?;
        Ok(())
    }

    pub fn get_archive_by_path(&self, archive_path: &str) -> Result<Option<ArchiveRecord>> {
        let record = self.conn.query_row(
            "SELECT id, archive_path, archive_size, creation_date, original_location, destination_location, description, file_count
             FROM archives 
             WHERE archive_path = ?1",
            params![archive_path],
            |row| {
                Ok(ArchiveRecord {
                    id: Some(row.get(0)?),
                    archive_path: row.get(1)?,
                    archive_size: row.get::<_, i64>(2)? as u64,
                    creation_date: row.get::<_, i64>(3)? as u64,
                    original_location: row.get(4)?,
                    destination_location: row.get(5)?,
                    description: row.get(6)?,
                    file_count: row.get::<_, i32>(7)? as u32,
                })
            },
        ).optional().context("Failed to query archive by path")?;

        Ok(record)
    }

    pub fn get_archive_files(&self, archive_id: i64) -> Result<Vec<ArchiveFileMapping>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, archive_id, file_path, original_path, file_size, archived_at 
                      FROM archive_files 
                      WHERE archive_id = ?1 
                      ORDER BY archived_at DESC")
            .context("Failed to prepare query")?;

        let mappings = stmt
            .query_map(params![archive_id], |row| {
                Ok(ArchiveFileMapping {
                    id: Some(row.get(0)?),
                    archive_id: row.get(1)?,
                    file_path: row.get(2)?,
                    original_path: row.get(3)?,
                    file_size: row.get::<_, i64>(4)? as u64,
                    archived_at: row.get::<_, i64>(5)? as u64,
                })
            })
            .context("Failed to execute query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect results")?;

        Ok(mappings)
    }

    pub fn get_all_archives(&self) -> Result<Vec<ArchiveRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, archive_path, archive_size, creation_date, original_location, destination_location, description, file_count 
                      FROM archives 
                      ORDER BY creation_date DESC")
            .context("Failed to prepare query")?;

        let archives = stmt
            .query_map([], |row| {
                Ok(ArchiveRecord {
                    id: Some(row.get(0)?),
                    archive_path: row.get(1)?,
                    archive_size: row.get::<_, i64>(2)? as u64,
                    creation_date: row.get::<_, i64>(3)? as u64,
                    original_location: row.get(4)?,
                    destination_location: row.get(5)?,
                    description: row.get(6)?,
                    file_count: row.get::<_, i32>(7)? as u32,
                })
            })
            .context("Failed to execute query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect results")?;

        Ok(archives)
    }

    pub fn update_archive_destination(&mut self, archive_path: &str, destination: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE archives SET destination_location = ?1 WHERE archive_path = ?2",
                params![destination, archive_path],
            )
            .context("Failed to update archive destination")?;
        Ok(())
    }

    pub fn export_json(&self, output_path: impl AsRef<Path>) -> Result<()> {
        let archives = self.get_all_archives()?;
        let json = serde_json::to_string_pretty(&archives).context("Failed to serialize to JSON")?;
        std::fs::write(output_path.as_ref(), json)
            .with_context(|| format!("Failed to write JSON to {}", output_path.as_ref().display()))?;
        Ok(())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_archive_tracking() -> Result<()> {
        let db_file = NamedTempFile::new()?;
        let mut conn = Connection::open(db_file.path())?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to enable WAL mode")?;
        
        let mut tracker = ArchiveTracker::new(&mut conn)?;

        // Create a test archive record
        let archive_record = ArchiveRecord {
            id: None,
            archive_path: "/path/to/archive.oarc".to_string(),
            archive_size: 1024,
            creation_date: 0, // Will be overridden
            original_location: "/original/location".to_string(),
            destination_location: Some("/destination/location".to_string()),
            description: Some("Test archive".to_string()),
            file_count: 5,
        };

        // Record the archive
        let archive_id = tracker.record_archive(archive_record.clone())?;

        // Verify the archive was recorded
        let retrieved = tracker.get_archive_by_path("/path/to/archive.oarc")?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.archive_path, "/path/to/archive.oarc");
        assert_eq!(retrieved.archive_size, 1024);
        assert_eq!(retrieved.destination_location, Some("/destination/location".to_string()));

        // Add some files to the archive
        let files = vec![
            ArchiveFileMapping {
                id: None,
                archive_id,
                file_path: "/archive/file1.jpg".to_string(),
                original_path: "/original/file1.jpg".to_string(),
                file_size: 512,
                archived_at: 0, // Will be overridden
            },
            ArchiveFileMapping {
                id: None,
                archive_id,
                file_path: "/archive/file2.png".to_string(),
                original_path: "/original/file2.png".to_string(),
                file_size: 256,
                archived_at: 0, // Will be overridden
            },
        ];

        tracker.record_archive_files(archive_id, files)?;

        // Retrieve the files for the archive
        let archive_files = tracker.get_archive_files(archive_id)?;
        assert_eq!(archive_files.len(), 2);
        assert_eq!(archive_files[0].file_path, "/archive/file1.jpg");
        assert_eq!(archive_files[1].file_path, "/archive/file2.png");

        // Get all archives
        let all_archives = tracker.get_all_archives()?;
        assert_eq!(all_archives.len(), 1);

        Ok(())
    }
}