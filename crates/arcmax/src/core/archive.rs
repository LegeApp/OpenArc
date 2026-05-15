use std::path::Path;
use std::io::Read;
use anyhow::Result;

#[derive(Debug)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub mtime: Option<u64>,
    pub is_dir: bool,
}

pub trait ArchiveReader {
    fn list(&mut self) -> Result<Vec<FileEntry>>;
    fn extract(&mut self, entry: &FileEntry, writer: &mut dyn std::io::Write) -> Result<()>;
    fn extract_all(&mut self, output_dir: &Path) -> Result<()>;
}

pub trait ArchiveWriter {
    fn add_file(&mut self, path: &Path, reader: &mut dyn Read) -> Result<()>;
    fn finalize(&mut self) -> Result<()>;
}