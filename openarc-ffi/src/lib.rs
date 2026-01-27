use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};
use std::thread;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use openarc_core::orchestrator::{self, OrchestratorSettings};

// Global error message storage (mutable)
static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum CompressionMethod {
    ArcMax = 0,
    Zstd = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum VideoPreset {
    Phone = 0,    // H264/Medium
    Camera = 1,   // H265/Medium
    Fast = 2,     // H264/Fast
    Quality = 3,  // H265/Slow
}

/// Compression settings matching CLI options from openarc-core OrchestratorSettings.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompressionSettings {
    pub bpg_quality: c_int,           // 0-51, lower = better quality (default: 25)
    pub bpg_lossless: bool,           // Enable lossless BPG compression
    pub bpg_bit_depth: c_int,         // 8-12 bit depth
    pub bpg_chroma_format: c_int,     // 0=420, 1=444, 2=RGB
    pub bpg_encoder_type: c_int,      // 0=default, 1=slow
    pub bpg_compression_level: c_int, // 1-9
    pub video_codec: c_int,           // 0=H264, 1=H265
    pub video_speed: c_int,           // 0=Fast, 1=Medium, 2=Slow
    pub video_crf: c_int,             // 0-51, lower = better quality (default: 23)
    pub compression_level: c_int,     // ArcMax compression level (1-22)
    pub enable_catalog: bool,         // Enable incremental backup tracking (default: true)
    pub enable_dedup: bool,           // Enable file deduplication (default: true)
    pub skip_already_compressed_videos: bool, // Skip re-encoding efficient videos (default: true)
}

#[repr(C)]
#[derive(Debug)]
pub struct ProgressInfo {
    pub current_file: c_int,
    pub total_files: c_int,
    pub progress_percent: f64,
    pub current_file_name: *const c_char,
}

pub type ProgressCallback = unsafe extern "C" fn(progress: ProgressInfo);

fn set_last_error(error: String) {
    if let Ok(mut guard) = LAST_ERROR.lock() {
        *guard = CString::new(error).ok();
    }
}

fn get_last_error_ptr() -> *const c_char {
    if let Ok(guard) = LAST_ERROR.lock() {
        if let Some(ref msg) = *guard {
            return msg.as_ptr();
        }
    }
    ptr::null()
}

fn detect_file_type_ffi(file_path: &str) -> c_int {
    match infer::get_from_path(file_path) {
        Ok(Some(info)) => {
            match info.mime_type() {
                "image/jpeg" | "image/png" | "image/tiff" | "image/bmp" => 1, // Image
                "video/mp4" | "video/quicktime" | "video/x-msvideo" | "video/x-matroska" => 2, // Video
                "application/pdf" | "text/plain" => 3, // Document
                _ => 0, // Unknown
            }
        }
        Ok(None) | Err(_) => 0, // Unknown
    }
}

#[export_name = "CreateArchive"]
pub unsafe extern "C" fn CreateArchive(
    output_path: *const c_char,
    input_files: *const *const c_char,
    file_count: c_int,
    settings: *const CompressionSettings,
    callback: Option<ProgressCallback>,
) -> c_int {
    if output_path.is_null() || input_files.is_null() || settings.is_null() {
        set_last_error("Null pointer passed to CreateArchive".to_string());
        return -1;
    }

    let output_path = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid output path string".to_string());
            return -1;
        }
    };

    let input_slice = slice::from_raw_parts(input_files, file_count as usize);
    let mut input_paths = Vec::new();
    
    for &ptr in input_slice {
        if ptr.is_null() {
            set_last_error("Null file path in input array".to_string());
            return -1;
        }
        
        let path = match CStr::from_ptr(ptr).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid file path string in input array".to_string());
                return -1;
            }
        };
        
        input_paths.push(path);
    }

    let compression_settings = *settings;
    
    // Run in a blocking thread to avoid blocking the main thread
    match thread::spawn(move || -> Result<c_int> {
        let input: Vec<std::path::PathBuf> = input_paths.iter().map(|s| std::path::PathBuf::from(s)).collect();

        let progress_fn: Option<Arc<orchestrator::ProgressFn>> = callback.map(|cb| {
            Arc::new(move |cur: usize, total: usize, name: &str| {
                let file_name_c = match CString::new(name) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null(),
                };

                let progress = ProgressInfo {
                    current_file: cur as c_int,
                    total_files: total as c_int,
                    progress_percent: if total > 0 { (cur as f64 / total as f64) * 100.0 } else { 0.0 },
                    current_file_name: file_name_c,
                };

                unsafe { cb(progress) };

                if !file_name_c.is_null() {
                    unsafe { let _ = CString::from_raw(file_name_c as *mut c_char); }
                }
            }) as Arc<orchestrator::ProgressFn>
        });

        let video_preset = match (compression_settings.video_codec, compression_settings.video_speed) {
            (0, 1) => 0, // H264/Medium
            (1, 1) => 1, // H265/Medium
            (0, 0) => 2, // H264/Fast
            (1, 2) => 3, // H265/Slow
            (0, _) => 2, // Default H264 -> Fast
            (1, _) => 1, // Default H265 -> Medium
            _ => 0,
        };

        let orch_settings = OrchestratorSettings {
            bpg_quality: compression_settings.bpg_quality,
            bpg_lossless: compression_settings.bpg_lossless,
            bpg_bit_depth: compression_settings.bpg_bit_depth,
            bpg_chroma_format: compression_settings.bpg_chroma_format,
            bpg_encoder_type: compression_settings.bpg_encoder_type,
            bpg_compression_level: compression_settings.bpg_compression_level,
            video_preset,
            video_crf: compression_settings.video_crf,
            compression_level: compression_settings.compression_level,
            enable_catalog: compression_settings.enable_catalog,
            enable_dedup: compression_settings.enable_dedup,
            skip_already_compressed_videos: compression_settings.skip_already_compressed_videos,
            heic_quality: 90,  // Default HEIC quality for extraction
            jpeg_quality: 92,  // Default JPEG quality for extraction
        };

        let _res = orchestrator::create_archive(
            &input,
            std::path::Path::new(output_path),
            orch_settings,
            progress_fn,
        )?;

        Ok(_res.discovered_files.len() as c_int)
    }).join() {
        Ok(result) => match result {
            Ok(count) => count,
            Err(e) => {
                set_last_error(format!("Failed to create archive: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during archive creation".to_string());
            -1
        }
    }
}

#[export_name = "VerifyArchive"]
pub unsafe extern "C" fn VerifyArchive(
    archive_path: *const c_char,
) -> c_int {
    if archive_path.is_null() {
        set_last_error("Null pointer passed to verify_archive".to_string());
        return -1;
    }

    let archive_path = match CStr::from_ptr(archive_path).to_str() {
        Ok(s) => std::path::Path::new(s).to_path_buf(),
        Err(_) => {
            set_last_error("Invalid archive path string".to_string());
            return -1;
        }
    };

    match thread::spawn(move || -> Result<c_int> {
        openarc_core::hash::verify_tar_zst_archive_with_level(&archive_path, 3)?;
        Ok(0)
    })
    .join()
    {
        Ok(result) => match result {
            Ok(code) => code,
            Err(e) => {
                set_last_error(format!("Failed to verify archive: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during archive verification".to_string());
            -1
        }
    }
}

#[export_name = "ExtractArchive"]
pub unsafe extern "C" fn ExtractArchive(
    archive_path: *const c_char,
    output_dir: *const c_char,
    callback: Option<ProgressCallback>,
) -> c_int {
    if archive_path.is_null() || output_dir.is_null() {
        set_last_error("Null pointer passed to extract_archive".to_string());
        return -1;
    }

    let archive_path = match CStr::from_ptr(archive_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid archive path string".to_string());
            return -1;
        }
    };

    let output_dir = match CStr::from_ptr(output_dir).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid output directory string".to_string());
            return -1;
        }
    };

    // Extract using openarc-core orchestrator
    match thread::spawn(move || -> Result<c_int> {
        let progress_fn: Option<Arc<orchestrator::ProgressFn>> = callback.map(|cb| {
            Arc::new(move |cur: usize, total: usize, name: &str| {
                let file_name_c = match CString::new(name) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null(),
                };

                let progress = ProgressInfo {
                    current_file: cur as c_int,
                    total_files: total as c_int,
                    progress_percent: if total > 0 { (cur as f64 / total as f64) * 100.0 } else { 0.0 },
                    current_file_name: file_name_c,
                };

                unsafe { cb(progress) };

                if !file_name_c.is_null() {
                    unsafe { let _ = CString::from_raw(file_name_c as *mut c_char); }
                }
            }) as Arc<orchestrator::ProgressFn>
        });

        let result = orchestrator::extract_archive(
            std::path::Path::new(archive_path),
            std::path::Path::new(output_dir),
            3, // Default compression level for decompression
            progress_fn,
        )?;

        Ok(result.files_extracted as c_int)
    }).join() {
        Ok(result) => match result {
            Ok(count) => count,
            Err(e) => {
                set_last_error(format!("Failed to extract archive: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during archive extraction".to_string());
            -1
        }
    }
}

/// Extraction settings for FFI
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExtractionSettings {
    /// Decode BPG files back to original formats (using metadata)
    pub decode_images: bool,
    /// HEIC quality (1-100) for re-encoding HEIC files
    pub heic_quality: c_int,
    /// JPEG quality (1-100) for decoding to JPEG
    pub jpeg_quality: c_int,
}

impl Default for ExtractionSettings {
    fn default() -> Self {
        Self {
            decode_images: true,
            heic_quality: 90,
            jpeg_quality: 92,
        }
    }
}

/// Extract archive with optional decoding of BPG/HEIC files
#[export_name = "ExtractArchiveWithDecoding"]
pub unsafe extern "C" fn ExtractArchiveWithDecoding(
    archive_path: *const c_char,
    output_dir: *const c_char,
    settings: *const ExtractionSettings,
    callback: Option<ProgressCallback>,
) -> c_int {
    if archive_path.is_null() || output_dir.is_null() {
        set_last_error("Null pointer passed to extract_archive_with_decoding".to_string());
        return -1;
    }

    let archive_path = match CStr::from_ptr(archive_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid archive path string".to_string());
            return -1;
        }
    };

    let output_dir = match CStr::from_ptr(output_dir).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid output directory string".to_string());
            return -1;
        }
    };

    let ext_settings = if settings.is_null() {
        ExtractionSettings::default()
    } else {
        *settings
    };

    match thread::spawn(move || -> Result<c_int> {
        let progress_fn: Option<Arc<orchestrator::ProgressFn>> = callback.map(|cb| {
            Arc::new(move |cur: usize, total: usize, name: &str| {
                let file_name_c = match CString::new(name) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null(),
                };

                let progress = ProgressInfo {
                    current_file: cur as c_int,
                    total_files: total as c_int,
                    progress_percent: if total > 0 { (cur as f64 / total as f64) * 100.0 } else { 0.0 },
                    current_file_name: file_name_c,
                };

                unsafe { cb(progress) };

                if !file_name_c.is_null() {
                    unsafe { let _ = CString::from_raw(file_name_c as *mut c_char); }
                }
            }) as Arc<orchestrator::ProgressFn>
        });

        let orch_settings = orchestrator::ExtractionSettings {
            decode_images: ext_settings.decode_images,
            heic_quality: ext_settings.heic_quality as u8,
            jpeg_quality: ext_settings.jpeg_quality as u8,
        };

        let result = orchestrator::extract_archive_with_decoding(
            std::path::Path::new(archive_path),
            std::path::Path::new(output_dir),
            3, // Default compression level
            orch_settings,
            progress_fn,
        )?;

        Ok(result.files_extracted as c_int)
    }).join() {
        Ok(result) => match result {
            Ok(count) => count,
            Err(e) => {
                set_last_error(format!("Failed to extract archive: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during archive extraction".to_string());
            -1
        }
    }
}

pub unsafe extern "C" fn DetectFileType(file_path: *const c_char) -> c_int {
    if file_path.is_null() {
        return 0; // Unknown
    }

    let path = match CStr::from_ptr(file_path).to_str() {
        Ok(s) => s,
        Err(_) => return 0, // Unknown
    };

    detect_file_type_ffi(path)
}

#[export_name = "GetOpenArcError"]
pub unsafe extern "C" fn GetOpenArcError() -> *const c_char {
    get_last_error_ptr()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhoneDbEntry {
    path: String,
    size: u64,
    mtime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhoneDb {
    version: u32,
    last_backup_at: u64,
    files: Vec<PhoneDbEntry>,
}

impl Default for PhoneDb {
    fn default() -> Self {
        Self {
            version: 1,
            last_backup_at: 0,
            files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhoneStatus {
    phone_root: String,
    db_path: String,
    first_time: bool,
    last_backup_at: u64,
    total_files: u64,
    archived_files: u64,
    unarchived_files: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn file_mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn phone_db_path(phone_root: &Path) -> PathBuf {
    phone_root.join("OpenArc").join("openarc_device.json")
}

fn normalize_rel_path(phone_root: &Path, file_path: &Path) -> String {
    let rel = file_path.strip_prefix(phone_root).unwrap_or(file_path);
    let mut s = rel.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        s = s.replace('\\', "/").to_lowercase();
    }
    s
}

fn load_phone_db(phone_root: &Path) -> (PhoneDb, bool) {
    let db_path = phone_db_path(phone_root);
    if !db_path.exists() {
        return (PhoneDb::default(), false);
    }

    match fs::read_to_string(&db_path)
        .ok()
        .and_then(|s| serde_json::from_str::<PhoneDb>(&s).ok())
    {
        Some(db) => (db, true),
        None => (PhoneDb::default(), true),
    }
}

fn save_phone_db(phone_root: &Path, db: &PhoneDb) -> std::result::Result<(), String> {
    let db_path = phone_db_path(phone_root);
    let dir = db_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Invalid DB path".to_string())?;

    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create db dir: {e}"))?;

    let json = serde_json::to_string_pretty(db).map_err(|e| format!("Failed to serialize db: {e}"))?;
    fs::write(&db_path, json).map_err(|e| format!("Failed to write db: {e}"))?;
    Ok(())
}

fn phone_candidate_dirs(phone_root: &Path) -> Vec<PathBuf> {
    let names = [
        "DCIM",
        "Pictures",
        "Camera",
        "Movies",
        "Videos",
        "Documents",
        "Download",
        "Downloads",
    ];

    names
        .iter()
        .map(|n| phone_root.join(n))
        .filter(|p| p.exists())
        .collect()
}

fn collect_phone_files(phone_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let dirs = phone_candidate_dirs(phone_root);
    if dirs.is_empty() {
        return Ok(Vec::new());
    }
    openarc_core::orchestrator::collect_files(&dirs)
}

fn compute_phone_status(phone_root: &Path) -> anyhow::Result<(PhoneStatus, Vec<PathBuf>, PhoneDb)> {
    let (mut db, existed) = load_phone_db(phone_root);
    if db.version == 0 {
        db.version = 1;
    }

    let db_path = phone_db_path(phone_root);
    let files = collect_phone_files(phone_root)?;

    let mut archived = 0u64;
    let mut unarchived = Vec::new();

    for f in &files {
        let meta = match fs::metadata(f) {
            Ok(m) => m,
            Err(_) => {
                unarchived.push(f.clone());
                continue;
            }
        };

        let rel = normalize_rel_path(phone_root, f);
        let size = meta.len();
        let mtime = file_mtime_secs(&meta);

        let in_db = db
            .files
            .iter()
            .any(|e| e.path == rel && e.size == size && e.mtime_secs == mtime);

        if in_db {
            archived += 1;
        } else {
            unarchived.push(f.clone());
        }
    }

    let status = PhoneStatus {
        phone_root: phone_root.to_string_lossy().to_string(),
        db_path: db_path.to_string_lossy().to_string(),
        first_time: !existed,
        last_backup_at: db.last_backup_at,
        total_files: files.len() as u64,
        archived_files: archived,
        unarchived_files: unarchived.len() as u64,
    };

    Ok((status, unarchived, db))
}

#[export_name = "FreeCString"]
pub unsafe extern "C" fn FreeCString(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let _ = CString::from_raw(ptr);
}

#[export_name = "PhoneGetStatusJson"]
pub unsafe extern "C" fn PhoneGetStatusJson(phone_root: *const c_char) -> *mut c_char {
    if phone_root.is_null() {
        set_last_error("Null pointer passed to PhoneGetStatusJson".to_string());
        return ptr::null_mut();
    }

    let phone_root = match CStr::from_ptr(phone_root).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid phone root string".to_string());
            return ptr::null_mut();
        }
    };

    let root = PathBuf::from(phone_root);
    match compute_phone_status(&root) {
        Ok((status, _, _)) => match serde_json::to_string(&status) {
            Ok(json) => match CString::new(json) {
                Ok(s) => s.into_raw(),
                Err(_) => {
                    set_last_error("Failed to allocate status string".to_string());
                    ptr::null_mut()
                }
            },
            Err(e) => {
                set_last_error(format!("Failed to serialize status: {e}"));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(format!("Failed to compute phone status: {e}"));
            ptr::null_mut()
        }
    }
}

#[export_name = "PhoneArchivePendingFiles"]
pub unsafe extern "C" fn PhoneArchivePendingFiles(
    phone_root: *const c_char,
    output_path: *const c_char,
    settings: *const CompressionSettings,
    callback: Option<ProgressCallback>,
) -> c_int {
    if phone_root.is_null() || output_path.is_null() || settings.is_null() {
        set_last_error("Null pointer passed to PhoneArchivePendingFiles".to_string());
        return -1;
    }

    let phone_root = match CStr::from_ptr(phone_root).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid phone root string".to_string());
            return -1;
        }
    };

    let output_path = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("Invalid output path string".to_string());
            return -1;
        }
    };

    let compression_settings = *settings;

    match thread::spawn(move || -> Result<c_int> {
        let root = PathBuf::from(&phone_root);

        let (status, pending, mut db) = compute_phone_status(&root)
            .map_err(|e| anyhow::anyhow!(e))?;

        if pending.is_empty() {
            if status.first_time {
                let _ = save_phone_db(&root, &db);
            }
            return Ok(0);
        }

        let progress_fn: Option<Arc<orchestrator::ProgressFn>> = callback.map(|cb| {
            Arc::new(move |cur: usize, total: usize, name: &str| {
                let file_name_c = match CString::new(name) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null(),
                };

                let progress = ProgressInfo {
                    current_file: cur as c_int,
                    total_files: total as c_int,
                    progress_percent: if total > 0 { (cur as f64 / total as f64) * 100.0 } else { 0.0 },
                    current_file_name: file_name_c,
                };

                unsafe { cb(progress) };

                if !file_name_c.is_null() {
                    unsafe { let _ = CString::from_raw(file_name_c as *mut c_char); }
                }
            }) as Arc<orchestrator::ProgressFn>
        });

        let video_preset = match (compression_settings.video_codec, compression_settings.video_speed) {
            (0, 1) => 0,
            (1, 1) => 1,
            (0, 0) => 2,
            (1, 2) => 3,
            (0, _) => 2,
            (1, _) => 1,
            _ => 0,
        };

        let orch_settings = OrchestratorSettings {
            bpg_quality: compression_settings.bpg_quality,
            bpg_lossless: compression_settings.bpg_lossless,
            bpg_bit_depth: compression_settings.bpg_bit_depth,
            bpg_chroma_format: compression_settings.bpg_chroma_format,
            bpg_encoder_type: compression_settings.bpg_encoder_type,
            bpg_compression_level: compression_settings.bpg_compression_level,
            video_preset,
            video_crf: compression_settings.video_crf,
            compression_level: compression_settings.compression_level,
            enable_catalog: false,
            enable_dedup: compression_settings.enable_dedup,
            skip_already_compressed_videos: compression_settings.skip_already_compressed_videos,
            heic_quality: 90,
            jpeg_quality: 92,
        };

        let res = orchestrator::create_archive(
            &pending,
            Path::new(&output_path),
            orch_settings,
            progress_fn,
        )?;

        let mut new_files: Vec<PhoneDbEntry> = Vec::new();
        for pf in &res.processed {
            let p = &pf.original_path;
            let meta = match fs::metadata(p) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let rel = normalize_rel_path(&root, p);
            new_files.push(PhoneDbEntry {
                path: rel,
                size: meta.len(),
                mtime_secs: file_mtime_secs(&meta),
            });
        }

        let mut merged: Vec<PhoneDbEntry> = Vec::new();
        merged.extend(db.files.into_iter());
        for e in new_files {
            merged.retain(|x| x.path != e.path);
            merged.push(e);
        }
        db.files = merged;
        db.last_backup_at = now_secs();

        if let Err(e) = save_phone_db(&root, &db) {
            set_last_error(e);
        }

        Ok(res.processed.len() as c_int)
    })
    .join()
    {
        Ok(result) => match result {
            Ok(count) => count,
            Err(e) => {
                set_last_error(format!("Failed to archive phone files: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during phone archiving".to_string());
            -1
        }
    }
}

/// Archive file information for listing
#[repr(C)]
#[derive(Debug)]
pub struct ArchiveFileInfo {
    pub filename: *const c_char,
    pub original_size: u64,
    pub compressed_size: u64,
    pub file_type: c_int, // 0=unknown, 1=image, 2=video, 3=document
}

/// List archive contents
#[export_name = "ListArchive"]
pub unsafe extern "C" fn ListArchive(
    archive_path: *const c_char,
    file_count: *mut c_int,
    files: *mut *mut ArchiveFileInfo,
) -> c_int {
    if archive_path.is_null() || file_count.is_null() || files.is_null() {
        set_last_error("Null pointer passed to list_archive".to_string());
        return -1;
    }

    let archive_path = match CStr::from_ptr(archive_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid archive path string".to_string());
            return -1;
        }
    };

    // Simple implementation - just return success for now
    // In a full implementation, this would parse the archive structure
    *file_count = 0;
    *files = ptr::null_mut();
    
    0
}

/// Archive record information for FFI
#[repr(C)]
#[derive(Debug)]
pub struct ArchiveRecordInfo {
    pub id: i64,
    pub archive_path: *const c_char,
    pub archive_size: u64,
    pub creation_date: u64,
    pub original_location: *const c_char,
    pub destination_location: *const c_char,
    pub description: *const c_char,
    pub file_count: u32,
}

/// Update archive destination location
#[export_name = "UpdateArchiveDestination"]
pub unsafe extern "C" fn UpdateArchiveDestination(
    catalog_db_path: *const c_char,
    archive_path: *const c_char,
    destination_path: *const c_char,
) -> c_int {
    if catalog_db_path.is_null() || archive_path.is_null() || destination_path.is_null() {
        set_last_error("Null pointer passed to UpdateArchiveDestination".to_string());
        return -1;
    }

    let catalog_db_path = match CStr::from_ptr(catalog_db_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid catalog database path string".to_string());
            return -1;
        }
    };

    let archive_path = match CStr::from_ptr(archive_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid archive path string".to_string());
            return -1;
        }
    };

    let destination_path = match CStr::from_ptr(destination_path).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid destination path string".to_string());
            return -1;
        }
    };

    match thread::spawn(move || -> Result<c_int> {
        orchestrator::update_archive_destination(catalog_db_path, archive_path, destination_path)?;
        Ok(0)
    }).join() {
        Ok(result) => match result {
            Ok(code) => code,
            Err(e) => {
                set_last_error(format!("Failed to update archive destination: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during archive destination update".to_string());
            -1
        }
    }
}

/// Get all archives from the database
#[export_name = "GetAllArchives"]
pub unsafe extern "C" fn GetAllArchives(
    catalog_db_path: *const c_char,
    archive_count: *mut c_int,
    archives: *mut *mut ArchiveRecordInfo,
) -> c_int {
    if catalog_db_path.is_null() || archive_count.is_null() || archives.is_null() {
        set_last_error("Null pointer passed to GetAllArchives".to_string());
        return -1;
    }

    let catalog_db_path = match CStr::from_ptr(catalog_db_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid catalog database path string".to_string());
            return -1;
        }
    };

    let result = (|| -> Result<c_int> {
        let mut catalog = openarc_core::backup_catalog::BackupCatalog::new(catalog_db_path)?;

        // Create archive tracker using the same connection as the backup catalog
        let tracker = openarc_core::archive_tracker::ArchiveTracker::new(catalog.get_connection_mut())?;
        let archive_records = tracker.get_all_archives()?;

        // Allocate memory for the array of archive records
        let mut archive_infos: Vec<ArchiveRecordInfo> = Vec::new();

        for record in archive_records {
            let archive_path_c = match CString::new(record.archive_path) {
                Ok(s) => s.into_raw(),
                Err(_) => ptr::null_mut(),
            };

            let original_location_c = match CString::new(record.original_location) {
                Ok(s) => s.into_raw(),
                Err(_) => ptr::null_mut(),
            };

            let destination_location_c = match record.destination_location {
                Some(dest) => match CString::new(dest) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null_mut(),
                },
                None => ptr::null_mut(),
            };

            let description_c = match record.description {
                Some(desc) => match CString::new(desc) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null_mut(),
                },
                None => ptr::null_mut(),
            };

            archive_infos.push(ArchiveRecordInfo {
                id: record.id.unwrap_or(-1),
                archive_path: archive_path_c,
                archive_size: record.archive_size,
                creation_date: record.creation_date,
                original_location: original_location_c,
                destination_location: destination_location_c,
                description: description_c,
                file_count: record.file_count,
            });
        }

        // Store the count
        *archive_count = archive_infos.len() as c_int;

        // Allocate memory for the array and copy the data
        if !archive_infos.is_empty() {
            let boxed_array = Box::into_raw(archive_infos.into_boxed_slice());
            *archives = boxed_array as *mut ArchiveRecordInfo;
        } else {
            *archives = ptr::null_mut();
        }

        Ok(0)
    })();

    match result {
        Ok(code) => code,
        Err(e) => {
            set_last_error(format!("Failed to get all archives: {}", e));
            -1
        }
    }
}

/// Free the memory allocated by GetAllArchives
#[export_name = "FreeArchivesArray"]
pub unsafe extern "C" fn FreeArchivesArray(
    archives: *mut ArchiveRecordInfo,
    count: c_int,
) {
    if archives.is_null() || count <= 0 {
        return;
    }

    // Convert back to a Vec to properly deallocate
    let slice = std::slice::from_raw_parts_mut(archives, count as usize);

    // Free individual C strings
    for i in 0..count as usize {
        let archive = &mut slice[i];  // Use indexing to avoid moving the slice
        if !archive.archive_path.is_null() {
            let _ = CString::from_raw(archive.archive_path as *mut c_char);
        }
        if !archive.original_location.is_null() {
            let _ = CString::from_raw(archive.original_location as *mut c_char);
        }
        if !archive.destination_location.is_null() {
            let _ = CString::from_raw(archive.destination_location as *mut c_char);
        }
        if !archive.description.is_null() {
            let _ = CString::from_raw(archive.description as *mut c_char);
        }
    }

    // Free the array itself
    let _ = Box::from_raw(slice as *mut [ArchiveRecordInfo] as *mut [ArchiveRecordInfo]);
}

/// Encode a single image file to BPG
#[export_name = "EncodeBpgFile"]
pub unsafe extern "C" fn EncodeBpgFile(
    input_path: *const c_char,
    output_path: *const c_char,
    settings: *const CompressionSettings,
) -> c_int {
    if input_path.is_null() || output_path.is_null() || settings.is_null() {
        set_last_error("Null pointer passed to EncodeBpgFile".to_string());
        return -1;
    }

    let input_path = match CStr::from_ptr(input_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid input path string".to_string());
            return -1;
        }
    };

    let output_path = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid output path string".to_string());
            return -1;
        }
    };

    let compression_settings = *settings;

    match thread::spawn(move || -> Result<c_int> {
        use openarc_core::bpg_wrapper::{BpgConfig, encode_image_to_bpg};

        let config = BpgConfig {
            quality: compression_settings.bpg_quality as u8,
            lossless: compression_settings.bpg_lossless,
            bit_depth: compression_settings.bpg_bit_depth as u8,
            chroma_format: compression_settings.bpg_chroma_format as u8,
            encoder_type: compression_settings.bpg_encoder_type as u8,
            compression_level: compression_settings.bpg_compression_level as u8,
        };

        encode_image_to_bpg(input_path, output_path, &config)?;
        Ok(0)
    }).join() {
        Ok(result) => match result {
            Ok(code) => code,
            Err(e) => {
                set_last_error(format!("Failed to encode BPG: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during BPG encoding".to_string());
            -1
        }
    }
}

/// Encode a single video file with FFmpeg
#[export_name = "EncodeVideoFile"]
pub unsafe extern "C" fn EncodeVideoFile(
    input_path: *const c_char,
    output_path: *const c_char,
    settings: *const CompressionSettings,
) -> c_int {
    if input_path.is_null() || output_path.is_null() || settings.is_null() {
        set_last_error("Null pointer passed to EncodeVideoFile".to_string());
        return -1;
    }

    let input_path = match CStr::from_ptr(input_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid input path string".to_string());
            return -1;
        }
    };

    let output_path = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => std::path::Path::new(s),
        Err(_) => {
            set_last_error("Invalid output path string".to_string());
            return -1;
        }
    };

    let compression_settings = *settings;

    match thread::spawn(move || -> Result<c_int> {
        use openarc_core::codecs::ffmpeg::{FFmpegEncoder, FfmpegEncodeOptions, VideoCodec, VideoSpeedPreset};

        let codec = match compression_settings.video_codec {
            0 => VideoCodec::H264,
            1 => VideoCodec::H265,
            _ => VideoCodec::H264,
        };

        let speed = match compression_settings.video_speed {
            0 => VideoSpeedPreset::Fast,
            1 => VideoSpeedPreset::Medium,
            2 => VideoSpeedPreset::Slow,
            _ => VideoSpeedPreset::Medium,
        };

        let options = FfmpegEncodeOptions {
            codec,
            speed,
            crf: Some(compression_settings.video_crf as u8),
            copy_audio: true,
        };

        let encoder = FFmpegEncoder::with_options(options);
        encoder.encode_file(input_path, output_path)?;
        Ok(0)
    }).join() {
        Ok(result) => match result {
            Ok(code) => code,
            Err(e) => {
                set_last_error(format!("Failed to encode video: {}", e));
                -1
            }
        },
        Err(_) => {
            set_last_error("Thread panicked during video encoding".to_string());
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_detection() {
        assert_eq!(detect_file_type_ffi("test.jpg"), 1); // Image
        assert_eq!(detect_file_type_ffi("test.mp4"), 2); // Video
        assert_eq!(detect_file_type_ffi("test.pdf"), 3); // Document
        assert_eq!(detect_file_type_ffi("test.xyz"), 0); // Unknown
    }
}
