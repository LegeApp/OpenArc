//! MTP (Media Transfer Protocol) FFI interface for C# GUI
//! 
//! Provides proper access to MTP devices (phones, cameras) using the Windows
//! Portable Devices API via the winmtp crate. This handles all MTP complexity
//! on the Rust side, returning simple local file paths to C#.
//!
//! Architecture: C# GUI never sees MTP paths - Rust copies files to temp and
//! returns local filesystem paths that work with standard file operations.

use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::Mutex;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use winmtp::Provider;
use winmtp::object::ObjectType;
use winmtp::PortableDevices::WPD_OBJECT_SIZE;
use widestring::U16CString;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// MTP device information - simple struct C# can understand
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpDeviceInfo {
    pub id: String,
    pub friendly_name: String,
    pub device_type: String, // "phone", "camera", "media_player", "unknown"
}

/// MTP file/folder information with local path for cached files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpObjectInfo {
    pub id: String,
    pub name: String,
    pub parent_id: String,
    pub is_folder: bool,
    pub size: u64,
    /// Local temp path if file was cached (for thumbnails etc)
    pub local_path: Option<String>,
}

/// Result structure for MTP operations
#[derive(Debug, Serialize, Deserialize)]
pub struct MtpResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> MtpResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// --- Global State ---
// REMOVED: static MTP_PROVIDER. Sharing COM objects across threads causes failures.
// KEEPING: Cache is fine to share as long as we lock it, it stores simple Strings/Paths.
static TEMP_CACHE: Mutex<Option<HashMap<(String, String), PathBuf>>> = Mutex::new(None);

// Global semaphore to serialize ALL MTP operations (prevents 0x800700AA race conditions)
static MTP_SEMAPHORE: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(1)));

/// Get the temp directory for MTP file caching
fn get_mtp_temp_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("OpenArc_MTP_Cache");
    let _ = std::fs::create_dir_all(&path);
    path
}

/// App identifiers for MTP connection
fn get_app_identifiers() -> winmtp::device::device_values::AppIdentifiers {
    winmtp::make_current_app_identifiers!()
}

/// Helper to get a fresh provider on the CURRENT thread
fn get_provider() -> Result<Provider, String> {
    // Provider::new() internally calls CoInitializeEx, which is required per-thread
    Provider::new().map_err(|e| format!("Failed to init MTP Provider: {:?}", e))
}

/// Determine device type from name heuristics
fn determine_device_type(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("phone") || lower.contains("galaxy") || lower.contains("iphone") ||
       lower.contains("pixel") || lower.contains("oneplus") || lower.contains("samsung") ||
       lower.contains("xiaomi") || lower.contains("huawei") || lower.contains("oppo") ||
       lower.contains("s24") || lower.contains("s23") || lower.contains("s22") ||
       lower.contains("note") || lower.contains("fold") || lower.contains("flip") {
        "phone"
    } else if lower.contains("camera") || lower.contains("canon") || lower.contains("nikon") ||
              lower.contains("sony") || lower.contains("fuji") || lower.contains("gopro") {
        "camera"
    } else if lower.contains("player") || lower.contains("ipod") || lower.contains("walkman") {
        "media_player"
    } else {
        "unknown"
    }
}

// --- FFI Functions ---

/// List all connected MTP devices
#[no_mangle]
pub unsafe extern "C" fn MtpListDevices() -> *mut c_char {
    let result = list_devices_impl();
    to_json_result(result)
}

fn list_devices_impl() -> MtpResult<Vec<MtpDeviceInfo>> {
    let provider = match get_provider() {
        Ok(p) => p,
        Err(e) => return MtpResult::err(e),
    };

    match provider.enumerate_devices() {
        Ok(devices) => {
            let infos = devices.iter().map(|d| {
                let name = d.friendly_name().to_string();
                MtpDeviceInfo {
                    id: d.device_id(),
                    device_type: determine_device_type(&name).to_string(),
                    friendly_name: name,
                }
            }).collect();
            MtpResult::ok(infos)
        }
        Err(e) => MtpResult::err(format!("Enum error: {:?}", e)),
    }
}

/// List contents of a folder on an MTP device
#[no_mangle]
pub unsafe extern "C" fn MtpListFolder(
    device_id: *const c_char,
    object_id: *const c_char
) -> *mut c_char {
    let device_id = if device_id.is_null() {
        return json_error("device_id is null");
    } else {
        CStr::from_ptr(device_id).to_string_lossy().into_owned()
    };

    let object_id = if object_id.is_null() {
        String::new()
    } else {
        CStr::from_ptr(object_id).to_string_lossy().into_owned()
    };

    let result = list_folder_impl(&device_id, &object_id);
    to_json_result(result)
}

fn list_folder_impl(device_id: &str, object_id: &str) -> MtpResult<Vec<MtpObjectInfo>> {
    let provider = match get_provider() { Ok(p) => p, Err(e) => return MtpResult::err(e) };
    
    let devices = match provider.enumerate_devices() { Ok(d) => d, Err(e) => return MtpResult::err(format!("{:?}", e)) };
    let basic_device = match devices.iter().find(|d| d.device_id() == device_id) {
        Some(d) => d, None => return MtpResult::err(format!("Device not found: {}", device_id)),
    };

    let device = match basic_device.open(&get_app_identifiers(), true) {
        Ok(d) => d, Err(e) => return MtpResult::err(format!("Open failed: {:?}", e)),
    };

    let content = match device.content() { Ok(c) => c, Err(e) => return MtpResult::err(format!("{:?}", e)) };

    let parent_obj = if object_id.is_empty() {
        match content.root() { Ok(r) => r, Err(e) => return MtpResult::err(format!("{:?}", e)) }
    } else {
        let oid = match U16CString::from_str(object_id) {
            Ok(o) => o,
            Err(e) => return MtpResult::err(format!("Invalid object ID: {}", e)),
        };
        match content.object_by_id(oid) { Ok(o) => o, Err(e) => return MtpResult::err(format!("{:?}", e)) }
    };

    match parent_obj.children() {
        Ok(children) => {
            let mut objects = Vec::new();
            for child in children {
                let is_folder = matches!(child.object_type(), ObjectType::Folder | ObjectType::FunctionalObject);
                // Fetch size using properties
                let size = if !is_folder {
                    child.properties(&[WPD_OBJECT_SIZE])
                         .and_then(|v| v.get_u64(&WPD_OBJECT_SIZE))
                         .unwrap_or(0)
                } else {
                    0
                };

                objects.push(MtpObjectInfo {
                    id: child.id().to_string_lossy(),
                    name: child.name().to_string_lossy(),
                    parent_id: object_id.to_string(),
                    is_folder,
                    size,
                    local_path: None,
                });
            }
            MtpResult::ok(objects)
        }
        Err(e) => MtpResult::err(format!("Children error: {:?}", e)),
    }
}

/// Get thumbnail for MTP file - tries WPD native thumbnail first, falls back to generating from full file
#[no_mangle]
pub unsafe extern "C" fn MtpGetThumbnail(
    device_id: *const c_char,
    object_id: *const c_char,
    original_name: *const c_char,
    target_width: u32,
    target_height: u32
) -> *mut c_char {
    let device_id = if device_id.is_null() {
        return json_error("device_id is null");
    } else {
        CStr::from_ptr(device_id).to_string_lossy().into_owned()
    };

    let object_id = if object_id.is_null() {
        return json_error("object_id is null");
    } else {
        CStr::from_ptr(object_id).to_string_lossy().into_owned()
    };

    let original_name = if original_name.is_null() {
        "file".to_string()
    } else {
        CStr::from_ptr(original_name).to_string_lossy().into_owned()
    };

    let result = get_thumbnail_impl(&device_id, &object_id, &original_name, target_width, target_height);
    to_json_result(result)
}

fn get_thumbnail_impl(device_id: &str, object_id: &str, original_name: &str, _target_width: u32, _target_height: u32) -> MtpResult<String> {
    // CRITICAL: Serialize all MTP access to prevent race conditions
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _permit = rt.block_on(async {
        MTP_SEMAPHORE.acquire().await.unwrap()
    });
    
    eprintln!("[RUST-MTP] get_thumbnail: device={}, object={}, name={}", device_id, object_id, original_name);
    
    // 1. Try to get WPD native thumbnail first (fast, ~1-2KB)
    match try_get_wpd_thumbnail(device_id, object_id, original_name) {
        Ok(thumb_path) => {
            eprintln!("[RUST-MTP] ✓ WPD thumbnail success: {}", thumb_path);
            return MtpResult::ok(thumb_path);
        }
        Err(e) => {
            eprintln!("[RUST-MTP] WPD thumbnail not available ({}), falling back to full file", e);
        }
    }

    // For videos, do not download the full file just to thumbnail.
    let ext = std::path::Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "3gp" | "m4v"
    ) {
        return MtpResult::err(
            "No WPD thumbnail available for video; refusing to download full file for thumbnail".to_string(),
        );
    }
    
    // 2. Fallback: Download full file and let caller generate thumbnail
    // (semaphore already held, so this is sequential)
    cache_file_to_temp_impl_unlocked(device_id, object_id, original_name)
}

fn try_get_wpd_thumbnail(device_id: &str, object_id: &str, original_name: &str) -> Result<String, String> {
    use winmtp::PortableDevices::WPD_RESOURCE_THUMBNAIL;
    
    eprintln!("[RUST-MTP] → Trying WPD thumbnail for: {}", original_name);
    
    let provider = get_provider()?;
    let devices = provider.enumerate_devices().map_err(|e| format!("{:?}", e))?;
    let basic_device = devices.iter().find(|d| d.device_id() == device_id)
        .ok_or("Device not found")?;
    
    let device = basic_device.open(&get_app_identifiers(), true)
        .map_err(|e| format!("Open failed: {:?}", e))?;
    let content = device.content().map_err(|e| format!("{:?}", e))?;
    
    let oid = U16CString::from_str(object_id).map_err(|_| "Invalid Object ID")?;
    let obj = content.object_by_id(oid).map_err(|e| format!("{:?}", e))?;

    // Only use WPD_RESOURCE_THUMBNAIL (device-provided thumbnail).
    // DO NOT fall back to default resource here: on many devices that is the full file
    // (e.g. multi-GB videos), which is not acceptable for thumbnail generation.
    let mut stream = obj
        .open_resource_stream(&WPD_RESOURCE_THUMBNAIL)
        .map_err(|e| format!("No thumbnail resource: {:?}", e))?;
    
    // Save thumbnail to temp
    let temp_dir = get_mtp_temp_dir();
    let dev_hash = format!("{:x}", md5_simple(device_id.as_bytes()));
    let device_dir = temp_dir.join(&dev_hash[..8]);
    std::fs::create_dir_all(&device_dir).map_err(|e| format!("{}", e))?;
    
    let obj_hash = format!("{:x}", md5_simple(object_id.as_bytes()));

    // Check cache first (try common thumbnail extensions)
    let base_name = format!("{}_thumb", &obj_hash[..12]);
    for ext in ["jpg", "jpeg", "png", "bmp", "gif", "bin"] {
        let candidate = device_dir.join(format!("{}.{}", base_name, ext));
        if candidate.exists() {
            eprintln!("[RUST-MTP] WPD thumbnail cache hit: {}", candidate.display());
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    // Read thumbnail bytes (should be small). Cap at 10MB as a safety valve.
    let mut bytes: Vec<u8> = Vec::new();
    let mut limited = (&mut stream).take(10 * 1024 * 1024);
    limited
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Copy error: {}", e))?;

    let ext = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        "png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "gif"
    } else if bytes.starts_with(b"BM") {
        "bmp"
    } else {
        "bin"
    };

    let thumb_path = device_dir.join(format!("{}.{}", base_name, ext));
    let mut output = std::fs::File::create(&thumb_path)
        .map_err(|e| format!("File create error: {:?}", e))?;
    output
        .write_all(&bytes)
        .map_err(|e| format!("Write error: {}", e))?;

    eprintln!(
        "[RUST-MTP] WPD thumbnail extracted: {} bytes -> {}",
        bytes.len(),
        thumb_path.display()
    );
    Ok(thumb_path.to_string_lossy().to_string())
}

/// Cache a file from MTP device to local temp directory and return the local path.
/// This is the key function - C# never sees MTP paths, only local temp paths.
#[no_mangle]
pub unsafe extern "C" fn MtpCacheFileToTemp(
    device_id: *const c_char,
    object_id: *const c_char,
    original_name: *const c_char
) -> *mut c_char {
    let device_id = if device_id.is_null() {
        return json_error("device_id is null");
    } else {
        CStr::from_ptr(device_id).to_string_lossy().into_owned()
    };

    let object_id = if object_id.is_null() {
        return json_error("object_id is null");
    } else {
        CStr::from_ptr(object_id).to_string_lossy().into_owned()
    };

    let original_name = if original_name.is_null() {
        "file".to_string()
    } else {
        CStr::from_ptr(original_name).to_string_lossy().into_owned()
    };

    let result = cache_file_to_temp_impl(&device_id, &object_id, &original_name);
    to_json_result(result)
}

fn is_resource_in_use_error(msg: &str) -> bool {
    let msg = msg.to_ascii_lowercase();
    msg.contains("800700aa")
        || msg.contains("0x800700aa")
        || msg.contains("requested resource is in use")
        || msg.contains("resource is in use")
}

fn cache_file_to_temp_impl(device_id: &str, object_id: &str, original_name: &str) -> MtpResult<String> {
    // CRITICAL: Serialize all MTP access to prevent race conditions
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _permit = rt.block_on(async {
        MTP_SEMAPHORE.acquire().await.unwrap()
    });

    cache_file_to_temp_impl_unlocked(device_id, object_id, original_name)
}

fn cache_file_to_temp_impl_unlocked(device_id: &str, object_id: &str, original_name: &str) -> MtpResult<String> {
    // DIAGNOSTIC
    eprintln!("[RUST-MTP] cache_file_to_temp: device={}, object={}, name={}", device_id, object_id, original_name);
    
    // 1. Check Memory Cache
    {
        if let Ok(cache_lock) = TEMP_CACHE.lock() {
            if let Some(map) = cache_lock.as_ref() {
                let key = (device_id.to_string(), object_id.to_string());
                if let Some(path) = map.get(&key) {
                    if path.exists() {
                        eprintln!("[RUST-MTP] Cache HIT: {}", path.display());
                        return MtpResult::ok(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    eprintln!("[RUST-MTP] Cache MISS, will copy file");

    // 2. Prepare Paths
    let temp_dir = get_mtp_temp_dir();
    let dev_hash = format!("{:x}", md5_simple(device_id.as_bytes()));
    let device_dir = temp_dir.join(&dev_hash[..8]);
    if let Err(e) = std::fs::create_dir_all(&device_dir) {
        return MtpResult::err(format!("Failed to create temp dir: {}", e));
    }

    let extension = std::path::Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");
    
    let obj_hash = format!("{:x}", md5_simple(object_id.as_bytes()));
    let local_filename = format!("{}.{}", &obj_hash[..12], extension);
    let local_path = device_dir.join(&local_filename);
    let save_path = local_path.clone();

    // 3. Perform Copy (Scoped Provider)
    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        if attempt > 0 {
            let backoff_ms = match attempt {
                1 => 75,
                2 => 200,
                _ => 400,
            };
            std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
        }

        // We execute this in a closure to separate the COM work and handle Results correctly
        let copy_result: Result<(), String> = (|| {
            eprintln!("[RUST-MTP] Getting provider...");
            let provider = get_provider()?;
            eprintln!("[RUST-MTP] Enumerating devices...");
            let devices = provider.enumerate_devices().map_err(|e| format!("{:?}", e))?;
            eprintln!("[RUST-MTP] Found {} devices, looking for {}", devices.len(), device_id);
            let basic_device = devices
                .iter()
                .find(|d| d.device_id() == device_id)
                .ok_or("Device not found")?;

            eprintln!("[RUST-MTP] Opening device...");
            let device = basic_device
                .open(&get_app_identifiers(), true)
                .map_err(|e| format!("{:?}", e))?;
            let content = device.content().map_err(|e| format!("{:?}", e))?;

            eprintln!("[RUST-MTP] Getting object by ID: {}", object_id);
            let oid = U16CString::from_str(object_id).map_err(|_| "Invalid Object ID")?;
            let obj = content.object_by_id(oid).map_err(|e| format!("{:?}", e))?;

            eprintln!("[RUST-MTP] Opening read stream...");
            let mut stream = obj
                .open_read_stream()
                .map_err(|e| format!("Stream error: {:?}", e))?;
            eprintln!("[RUST-MTP] Creating output file: {}", save_path.display());
            let mut output = std::fs::File::create(&save_path)
                .map_err(|e| format!("File create error: {:?}", e))?;

            eprintln!("[RUST-MTP] Copying data...");
            let bytes_copied =
                std::io::copy(&mut stream, &mut output).map_err(|e| format!("Copy error: {}", e))?;
            eprintln!("[RUST-MTP] Copied {} bytes", bytes_copied);
            Ok(())
        })();

        match copy_result {
            Ok(()) => {
                last_err = None;
                break;
            }
            Err(e) => {
                let retryable = is_resource_in_use_error(&e);
                eprintln!(
                    "[RUST-MTP] Copy attempt {} failed (retryable={}): {}",
                    attempt + 1,
                    retryable,
                    e
                );
                last_err = Some(e);
                if retryable && attempt < 2 {
                    continue;
                }
                break;
            }
        }
    }

    match last_err {
        None => {
            eprintln!("[RUST-MTP] SUCCESS: {}", local_path.display());
            // Update cache
            if let Ok(mut cache_lock) = TEMP_CACHE.lock() {
                // Initialize if needed
                if cache_lock.is_none() {
                    *cache_lock = Some(HashMap::new());
                }
                
                if let Some(map) = cache_lock.as_mut() {
                    map.insert(
                        (device_id.to_string(), object_id.to_string()),
                        local_path.clone()
                    );
                }
            }
            MtpResult::ok(local_path.to_string_lossy().to_string())
        }
        Some(e) => {
            eprintln!("[RUST-MTP] FAILED: {}", e);
            // Cleanup partial file (best effort)
            let _ = std::fs::remove_file(&local_path);
            MtpResult::err(e)
        }
    }
}

/// Simple MD5-like hash for creating short filenames
fn md5_simple(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    hash
}

/// Get temp file path for an MTP object if already cached, without copying.
#[no_mangle]
pub unsafe extern "C" fn MtpGetCachedPath(
    device_id: *const c_char,
    object_id: *const c_char
) -> *mut c_char {
    let device_id = if device_id.is_null() {
        return json_error("device_id is null");
    } else {
        CStr::from_ptr(device_id).to_string_lossy().into_owned()
    };

    let object_id = if object_id.is_null() {
        return json_error("object_id is null");
    } else {
        CStr::from_ptr(object_id).to_string_lossy().into_owned()
    };

    if let Ok(cache_lock) = TEMP_CACHE.lock() {
        if let Some(map) = cache_lock.as_ref() {
            let key = (device_id, object_id);
            if let Some(path) = map.get(&key) {
                if path.exists() {
                     return to_json_result(MtpResult::ok(path.to_string_lossy().to_string()));
                }
            }
        }
    }
    
    to_json_result(MtpResult::<String>::ok(String::new()))
}

/// Clear the MTP temp cache (removes temp files)
#[no_mangle]
pub unsafe extern "C" fn MtpClearCache() -> *mut c_char {
    let mut cleared = 0;
    
    if let Ok(mut cache_lock) = TEMP_CACHE.lock() {
        if let Some(ref mut cache_map) = *cache_lock {
            for (_, path) in cache_map.drain() {
                if let Ok(_) = std::fs::remove_file(&path) {
                    cleared += 1;
                }
            }
        }
    }

    // Also try to clean up the temp directory
    let temp_dir = get_mtp_temp_dir();
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    to_json_result(MtpResult::ok(format!("Cleared {} cached files", cleared)))
}

/// Helper to serialize result to JSON string ptr
fn to_json_result<T: Serialize>(result: MtpResult<T>) -> *mut c_char {
    let json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
    CString::new(json).map(|s| s.into_raw()).unwrap_or(ptr::null_mut())
}

/// Helper for JSON errors
fn json_error(msg: &str) -> *mut c_char {
    let result: MtpResult<()> = MtpResult::err(msg);
    let json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
    CString::new(json).map(|s| s.into_raw()).unwrap_or(ptr::null_mut())
}

/// Read file to memory - note: THIS IS DANGEROUS FOR LARGE FILES
/// Included for compatibility but better to use cache_file_to_temp
#[no_mangle]
pub unsafe extern "C" fn MtpReadFileToMemory(
    device_id: *const c_char,
    object_id: *const c_char,
    out_size: *mut u64
) -> *mut u8 {
    let device_id = if device_id.is_null() { return ptr::null_mut(); } else { CStr::from_ptr(device_id).to_string_lossy().into_owned() };
    let object_id = if object_id.is_null() { return ptr::null_mut(); } else { CStr::from_ptr(object_id).to_string_lossy().into_owned() };

    let read_result: Result<Vec<u8>, String> = (|| {
        let provider = get_provider()?;
        let devices = provider.enumerate_devices().map_err(|e| format!("{:?}", e))?;
        let basic_device = devices.iter().find(|d| d.device_id() == device_id).ok_or("Device not found")?;
        let device = basic_device.open(&get_app_identifiers(), true).map_err(|e| format!("{:?}", e))?;
        let content = device.content().map_err(|e| format!("{:?}", e))?;
        let oid = U16CString::from_str(&object_id).map_err(|_| "Invalid Object ID")?;
        let obj = content.object_by_id(oid).map_err(|e| format!("{:?}", e))?;
        
        let mut stream = obj.open_read_stream().map_err(|e| format!("Stream error: {:?}", e))?;
        let mut data = Vec::new();
        stream.read_to_end(&mut data).map_err(|e| format!("{}", e))?;
        Ok(data)
    })();

    match read_result {
        Ok(data) => {
            if !out_size.is_null() {
                *out_size = data.len() as u64;
            }
            // Leak memory to return to C#, C# must call MtpFreeData
            let boxed_slice = data.into_boxed_slice();
            Box::into_raw(boxed_slice) as *mut u8
        },
        Err(_) => {
            if !out_size.is_null() { *out_size = 0; }
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn MtpFreeData(data: *mut u8, size: usize) {
    if !data.is_null() && size > 0 {
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(data, size));
    }
}

#[no_mangle]
pub unsafe extern "C" fn MtpFreeString(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}
