use arcmax::formats::freearc::{
    writer::{FreeArcWriter, ArchiveOptions},
    reader::FreeArcReader,
};
use std::io::Cursor;
use anyhow::Result;

#[test]
fn test_freearc_roundtrip() -> Result<()> {
    // Create test data
    let test_file_name = "test.txt";
    let test_file_content = b"Hello, FreeArc! This is a test file.";
    
    // Create archive in memory
    let archive_buffer = Cursor::new(Vec::new());
    
    // Write archive
    let archive_data = {
        let options = ArchiveOptions {
            compression: "lzma".to_string(),
            compression_level: 3,
            encryption: None,
            password: None,
        };
        
        let mut writer = FreeArcWriter::new(archive_buffer, options)?;
        
        // Add test file with data
        writer.add_file(test_file_name, test_file_content)?;
        
        // Finalize archive and get the writer back
        let cursor = writer.finish()?;
        cursor.into_inner()
    };
    
    // Read archive
    {
        println!("Archive size: {} bytes", archive_data.len());
        
        let cursor = Cursor::new(archive_data);
        let reader = FreeArcReader::new(cursor, None)?;
        
        // Verify directory structure
        assert_eq!(reader.directory.files.len(), 1, "Should have 1 file");
        assert_eq!(reader.directory.files[0].name, test_file_name);
        assert_eq!(reader.directory.files[0].size, test_file_content.len() as u64);
        assert_eq!(reader.directory.files[0].is_dir, false);
        
        // Extract and verify file content
        let extracted_data = reader.extract_file(0)?;
        assert_eq!(extracted_data.len(), test_file_content.len());
        assert_eq!(&extracted_data[..], test_file_content);
        
        println!("Successfully verified file: {}", test_file_name);
    }
    
    Ok(())
}

#[test]
fn test_freearc_multiple_files() -> Result<()> {
    // Create test data
    let files: Vec<(&str, &[u8])> = vec![
        ("file1.txt", b"First file content"),
        ("file2.txt", b"Second file content with more data"),
        ("file3.txt", b"Third"),
    ];
    
    // Create archive in memory
    let archive_buffer = Cursor::new(Vec::new());
    
    // Write archive
    let archive_data = {
        let options = ArchiveOptions {
            compression: "lzma".to_string(),
            compression_level: 3,
            encryption: None,
            password: None,
        };
        
        let mut writer = FreeArcWriter::new(archive_buffer, options)?;
        
        // Add all files
        for (name, content) in &files {
            writer.add_file(name, content)?;
        }
        
        let cursor = writer.finish()?;
        cursor.into_inner()
    };
    
    // Read and verify
    {
        println!("Multi-file archive size: {} bytes", archive_data.len());
        
        let cursor = Cursor::new(archive_data);
        let reader = FreeArcReader::new(cursor, None)?;
        
        assert_eq!(reader.directory.files.len(), files.len());
        
        for (i, (name, content)) in files.iter().enumerate() {
            assert_eq!(reader.directory.files[i].name, *name);
            assert_eq!(reader.directory.files[i].size, content.len() as u64);
            
            let extracted = reader.extract_file(i)?;
            assert_eq!(&extracted[..], *content);
            
            println!("Verified file {}: {}", i, name);
        }
    }
    
    Ok(())
}

#[test]
fn test_freearc_empty_archive() -> Result<()> {
    let archive_buffer = Cursor::new(Vec::new());
    
    let archive_data = {
        let options = ArchiveOptions {
            compression: "lzma".to_string(),
            compression_level: 3,
            encryption: None,
            password: None,
        };
        
        let writer = FreeArcWriter::new(archive_buffer, options)?;
        let cursor = writer.finish()?;
        cursor.into_inner()
    };
    
    {
        println!("Empty archive size: {} bytes", archive_data.len());
        
        let cursor = Cursor::new(archive_data);
        let reader = FreeArcReader::new(cursor, None)?;
        
        assert_eq!(reader.directory.files.len(), 0);
        assert_eq!(reader.directory.data_blocks.len(), 0);
    }
    
    Ok(())
}
