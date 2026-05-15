use std::path::Path;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use anyhow::Result;
use crate::formats::freearc::reader::FreeArcReader;
use crate::formats::peazip::PeaArchive;

/// Detected archive format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    FreeArc,
    PeaZip,
    Unknown,
}

/// Detect the archive format from file signature
pub fn detect_archive_format(path: &Path) -> Result<ArchiveFormat> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 4];

    // Try to read the header
    let bytes_read = file.read(&mut header)?;
    if bytes_read < 1 {
        return Ok(ArchiveFormat::Unknown);
    }

    // Check for PEA signature: 0xEA (first byte)
    if header[0] == 0xEA {
        eprintln!("Detected PEA format (magic byte 0xEA)");
        return Ok(ArchiveFormat::PeaZip);
    }

    // Check for FreeARC signature: "ArC\x01"
    if bytes_read >= 4 && header == [0x41, 0x72, 0x43, 0x01] {
        eprintln!("Detected FreeARC format (signature ArC\\x01)");
        return Ok(ArchiveFormat::FreeArc);
    }

    // Check file extension as fallback
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "pea" => {
                eprintln!("Detected PEA format from extension");
                return Ok(ArchiveFormat::PeaZip);
            }
            "arc" => {
                eprintln!("Detected FreeARC format from extension");
                return Ok(ArchiveFormat::FreeArc);
            }
            _ => {}
        }
    }

    Ok(ArchiveFormat::Unknown)
}

pub fn detect_format(path: &Path, password: Option<&str>, crypto_flags: Option<&str>) -> Result<Box<dyn crate::core::archive::ArchiveReader>> {
    let format = detect_archive_format(path)?;

    match format {
        ArchiveFormat::PeaZip => {
            eprintln!("Opening as PEA archive");
            let file = File::open(path)?;
            let password_opt = password.map(|s| s.to_string());
            Ok(Box::new(PeaArchive::new(file, password_opt)?))
        }
        ArchiveFormat::FreeArc => {
            eprintln!("Opening as FreeARC archive");
            let file = File::open(path)?;
            let password_opt = password.map(|s| s.to_string());
            Ok(Box::new(FreeArcReader::new(file, password_opt)?))
        }
        ArchiveFormat::Unknown => {
            // Try FreeARC as fallback (it has more robust error handling)
            eprintln!("Unknown format, attempting FreeARC");
            let file = File::open(path)?;
            let password_opt = password.map(|s| s.to_string());
            match FreeArcReader::new(file, password_opt) {
                Ok(archive) => Ok(Box::new(archive)),
                Err(e) => anyhow::bail!("Unsupported archive format: {}", e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_detect_free_arc_format() {
        let mut path = std::env::temp_dir();
        path.push(format!("arcmax_detect_free_arc_{}.arc", std::process::id()));

        {
            let mut f = fs::File::create(&path).unwrap();
            f.write_all(&[0x41, 0x72, 0x43, 0x01]).unwrap();
            f.write_all(b"dummy").unwrap();
        }

        let detected = detect_archive_format(&path).unwrap();
        assert_eq!(detected, ArchiveFormat::FreeArc);

        let _ = fs::remove_file(&path);
    }
}