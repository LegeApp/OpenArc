use crate::error::Result;
use crate::formats::arcmax::wire::FileEntry;

/// Accumulated file entries before they are serialized into the central directory.
#[derive(Debug, Default)]
pub struct CentralDirectory {
    entries: Vec<FileEntry>,
}

impl CentralDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: FileEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Serialize all entries into a flat byte buffer.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        for entry in &self.entries {
            entry.serialize(&mut buf)?;
        }
        Ok(buf)
    }

    /// Deserialize a central directory from its raw bytes.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = data;
        let mut entries = Vec::new();
        while !cursor.is_empty() {
            entries.push(FileEntry::deserialize(&mut cursor)?);
        }
        Ok(Self { entries })
    }
}
