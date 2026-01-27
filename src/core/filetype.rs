//! File type detection for routing to appropriate codecs

use std::path::Path;

/// Supported file types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    // Images
    ImageJpg,
    ImagePng,
    ImageTiff,
    ImageBmp,
    ImageWebP,
    ImageRaw(RawFormat),
    
    // Videos
    VideoMp4,
    VideoMov,
    VideoAvi,
    VideoMkv,
    VideoWebM,
    
    // Other files
    Other,
}

/// RAW image formats from cameras
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawFormat {
    CR2,    // Canon
    NEF,    // Nikon
    ARW,    // Sony
    DNG,    // Adobe
    RAF,    // Fujifilm
    ORF,    // Olympus
    RW2,    // Panasonic
    PEF,    // Pentax
    SRW,    // Samsung
}

/// Detect file type from magic bytes and extension
pub fn detect_file_type(data: &[u8], path: &Path) -> FileType {
    // First try magic number detection
    if let Some(file_type) = detect_from_magic(data) {
        return file_type;
    }
    
    // Fallback to extension
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            return detect_from_extension(ext_str);
        }
    }
    
    FileType::Other
}

/// Detect file type from magic bytes
fn detect_from_magic(data: &[u8]) -> Option<FileType> {
    if data.len() < 12 {
        return None;
    }
    
    // JPEG: FF D8 FF
    if data[0..3] == [0xFF, 0xD8, 0xFF] {
        return Some(FileType::ImageJpg);
    }
    
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(FileType::ImagePng);
    }
    
    // TIFF: 49 49 2A 00 (little-endian) or 4D 4D 00 2A (big-endian)
    if (data[0..4] == [0x49, 0x49, 0x2A, 0x00]) || (data[0..4] == [0x4D, 0x4D, 0x00, 0x2A]) {
        return Some(FileType::ImageTiff);
    }
    
    // BMP: 42 4D
    if data[0..2] == [0x42, 0x4D] {
        return Some(FileType::ImageBmp);
    }
    
    // WebP: 52 49 46 46 xx xx xx xx 57 45 42 50
    if data.len() >= 12 && data[0..4] == [0x52, 0x49, 0x46, 0x46] && data[8..12] == [0x57, 0x45, 0x42, 0x50] {
        return Some(FileType::ImageWebP);
    }
    
    // MP4: xx xx xx xx 66 74 79 70 (ftyp at offset 4)
    if data.len() >= 12 && data[4..8] == [0x66, 0x74, 0x79, 0x70] {
        return Some(FileType::VideoMp4);
    }
    
    // AVI: 52 49 46 46 xx xx xx xx 41 56 49 20
    if data.len() >= 12 && data[0..4] == [0x52, 0x49, 0x46, 0x46] && data[8..12] == [0x41, 0x56, 0x49, 0x20] {
        return Some(FileType::VideoAvi);
    }
    
    // MKV/WebM: 1A 45 DF A3
    if data[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        // Could be MKV or WebM, check extension later
        return Some(FileType::VideoMkv);
    }
    
    None
}

/// Detect file type from file extension
fn detect_from_extension(ext: &str) -> FileType {
    match ext.to_lowercase().as_str() {
        // Images
        "jpg" | "jpeg" => FileType::ImageJpg,
        "png" => FileType::ImagePng,
        "tif" | "tiff" => FileType::ImageTiff,
        "bmp" => FileType::ImageBmp,
        "webp" => FileType::ImageWebP,
        
        // RAW formats
        "cr2" => FileType::ImageRaw(RawFormat::CR2),
        "nef" => FileType::ImageRaw(RawFormat::NEF),
        "arw" => FileType::ImageRaw(RawFormat::ARW),
        "dng" => FileType::ImageRaw(RawFormat::DNG),
        "raf" => FileType::ImageRaw(RawFormat::RAF),
        "orf" => FileType::ImageRaw(RawFormat::ORF),
        "rw2" => FileType::ImageRaw(RawFormat::RW2),
        "pef" => FileType::ImageRaw(RawFormat::PEF),
        "srw" => FileType::ImageRaw(RawFormat::SRW),
        
        // Videos
        "mp4" | "m4v" => FileType::VideoMp4,
        "mov" => FileType::VideoMov,
        "avi" => FileType::VideoAvi,
        "mkv" => FileType::VideoMkv,
        "webm" => FileType::VideoWebM,
        
        // Other
        _ => FileType::Other,
    }
}

/// Check if a file type is an image
pub fn is_image(file_type: &FileType) -> bool {
    matches!(
        file_type,
        FileType::ImageJpg
            | FileType::ImagePng
            | FileType::ImageTiff
            | FileType::ImageBmp
            | FileType::ImageWebP
            | FileType::ImageRaw(_)
    )
}

/// Check if a file type is a video
pub fn is_video(file_type: &FileType) -> bool {
    matches!(
        file_type,
        FileType::VideoMp4
            | FileType::VideoMov
            | FileType::VideoAvi
            | FileType::VideoMkv
            | FileType::VideoWebM
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_detect_jpeg() {
        let jpeg_magic = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        let path = PathBuf::from("test.jpg");
        let file_type = detect_file_type(&jpeg_magic, &path);
        assert_eq!(file_type, FileType::ImageJpg);
    }
    
    #[test]
    fn test_detect_png() {
        let png_magic = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let path = PathBuf::from("test.png");
        let file_type = detect_file_type(&png_magic, &path);
        assert_eq!(file_type, FileType::ImagePng);
    }
    
    #[test]
    fn test_detect_from_extension() {
        let empty_data = vec![];
        let path = PathBuf::from("photo.cr2");
        let file_type = detect_file_type(&empty_data, &path);
        assert_eq!(file_type, FileType::ImageRaw(RawFormat::CR2));
    }
    
    #[test]
    fn test_is_image() {
        assert!(is_image(&FileType::ImageJpg));
        assert!(is_image(&FileType::ImageRaw(RawFormat::NEF)));
        assert!(!is_image(&FileType::VideoMp4));
    }
    
    #[test]
    fn test_is_video() {
        assert!(is_video(&FileType::VideoMp4));
        assert!(!is_video(&FileType::ImageJpg));
    }
}
