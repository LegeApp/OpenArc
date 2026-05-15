use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneSourceKind {
    Mtp,
    MountedFilesystem,
}

#[derive(Debug, Clone)]
pub struct DetectedPhone {
    pub id: String,
    pub display_name: String,
    pub source_kind: PhoneSourceKind,
    pub root_hint: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StagedPhoneInput {
    pub device_key: String,
    pub display_name: String,
    pub staged_root: PathBuf,
    pub catalog_db_path: PathBuf,
    pub total_files: usize,
    pub copied_files: usize,
    pub reused_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageManifestEntry {
    source_id: String,
    size: u64,
    mtime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageManifest {
    version: u32,
    files: HashMap<String, StageManifestEntry>,
}

impl Default for StageManifest {
    fn default() -> Self {
        Self {
            version: 1,
            files: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct FsSourceFile {
    source_path: PathBuf,
    rel_path: PathBuf,
    source_id: String,
    size: u64,
    mtime_secs: u64,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct MtpSourceFile {
    object_id: String,
    rel_path: PathBuf,
    size: u64,
}

const PHONE_MEDIA_DIRS: &[&str] = &[
    "dcim",
    "pictures",
    "picture",
    "camera",
    "videos",
    "video",
    "movies",
    "documents",
    "downloads",
    "download",
];

pub fn detect_phones() -> Result<Vec<DetectedPhone>> {
    let mut phones = Vec::new();

    #[cfg(windows)]
    {
        phones.extend(detect_windows_mtp_phones()?);
    }

    #[cfg(not(windows))]
    {
        phones.extend(detect_mounted_phones()?);
    }

    let mut seen = HashSet::new();
    phones.retain(|p| seen.insert(p.id.clone()));
    Ok(phones)
}

pub fn stage_phone_media(phone: &DetectedPhone) -> Result<StagedPhoneInput> {
    match phone.source_kind {
        PhoneSourceKind::MountedFilesystem => stage_from_filesystem(phone),
        PhoneSourceKind::Mtp => {
            #[cfg(windows)]
            {
                stage_from_mtp(phone)
            }
            #[cfg(not(windows))]
            {
                let _ = phone;
                Err(anyhow!("MTP staging is only available on Windows"))
            }
        }
    }
}

fn stage_from_filesystem(phone: &DetectedPhone) -> Result<StagedPhoneInput> {
    let root = phone
        .root_hint
        .clone()
        .or_else(|| phone.id.strip_prefix("fs:").map(PathBuf::from))
        .ok_or_else(|| anyhow!("Filesystem phone source is missing a root path"))?;

    let files = collect_filesystem_phone_files(&root)?;
    if files.is_empty() {
        return Err(anyhow!(
            "No files found in phone media folders under {}",
            root.display()
        ));
    }

    let device_key = make_device_key(&phone.id);
    let state_dir = phone_state_dir(&device_key);
    let stage_root = phone_stage_dir(&device_key);
    fs::create_dir_all(&state_dir)
        .with_context(|| format!("Failed to create state directory {}", state_dir.display()))?;
    fs::create_dir_all(&stage_root)
        .with_context(|| format!("Failed to create staging directory {}", stage_root.display()))?;

    let manifest_path = state_dir.join("staging_manifest.json");
    let old_manifest = load_manifest(&manifest_path)?;
    let mut new_manifest = StageManifest::default();

    let mut copied_files = 0usize;
    let mut reused_files = 0usize;

    for file in &files {
        let key = normalize_rel_key(&file.rel_path);
        let dst = stage_root.join(&file.rel_path);
        let unchanged = old_manifest
            .files
            .get(&key)
            .map(|prev| {
                prev.source_id == file.source_id
                    && prev.size == file.size
                    && prev.mtime_secs == file.mtime_secs
            })
            .unwrap_or(false)
            && dst.exists();

        if !unchanged {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create staging directory {}", parent.display())
                })?;
            }
            fs::copy(&file.source_path, &dst).with_context(|| {
                format!(
                    "Failed to stage file {} -> {}",
                    file.source_path.display(),
                    dst.display()
                )
            })?;
            copied_files += 1;
        } else {
            reused_files += 1;
        }

        new_manifest.files.insert(
            key,
            StageManifestEntry {
                source_id: file.source_id.clone(),
                size: file.size,
                mtime_secs: file.mtime_secs,
            },
        );
    }

    prune_stale_staged_files(&old_manifest, &new_manifest, &stage_root)?;
    save_manifest(&manifest_path, &new_manifest)?;

    Ok(StagedPhoneInput {
        device_key,
        display_name: phone.display_name.clone(),
        staged_root: stage_root,
        catalog_db_path: state_dir.join("catalog.sqlite"),
        total_files: files.len(),
        copied_files,
        reused_files,
    })
}

#[cfg(windows)]
fn stage_from_mtp(phone: &DetectedPhone) -> Result<StagedPhoneInput> {
    use widestring::U16CString;
    use winmtp::Provider;

    let provider = Provider::new().map_err(|e| anyhow!("Failed to initialize MTP provider: {e:?}"))?;
    let devices = provider
        .enumerate_devices()
        .map_err(|e| anyhow!("Failed to enumerate MTP devices: {e:?}"))?;

    let basic_device = devices
        .iter()
        .find(|d| d.device_id() == phone.id)
        .ok_or_else(|| anyhow!("MTP device was not found: {}", phone.display_name))?;

    let app_identifiers = winmtp::make_current_app_identifiers!();
    let device = basic_device
        .open(&app_identifiers, true)
        .map_err(|e| anyhow!("Failed to open MTP device: {e:?}"))?;
    let content = device
        .content()
        .map_err(|e| anyhow!("Failed to read MTP content: {e:?}"))?;
    let root = content
        .root()
        .map_err(|e| anyhow!("Failed to read MTP root object: {e:?}"))?;

    let mut files = Vec::new();
    collect_mtp_files_recursive(&root, None, 0, &mut files)?;
    if files.is_empty() {
        return Err(anyhow!(
            "No files found in MTP folders (Documents/Downloads/Pictures/DCIM/Videos)"
        ));
    }

    let device_key = make_device_key(&phone.id);
    let state_dir = phone_state_dir(&device_key);
    let stage_root = phone_stage_dir(&device_key);
    fs::create_dir_all(&state_dir)
        .with_context(|| format!("Failed to create state directory {}", state_dir.display()))?;
    fs::create_dir_all(&stage_root)
        .with_context(|| format!("Failed to create staging directory {}", stage_root.display()))?;

    let manifest_path = state_dir.join("staging_manifest.json");
    let old_manifest = load_manifest(&manifest_path)?;
    let mut new_manifest = StageManifest::default();

    let mut copied_files = 0usize;
    let mut reused_files = 0usize;

    for file in &files {
        let key = normalize_rel_key(&file.rel_path);
        let dst = stage_root.join(&file.rel_path);
        let unchanged = old_manifest
            .files
            .get(&key)
            .map(|prev| prev.source_id == file.object_id && prev.size == file.size)
            .unwrap_or(false)
            && dst.exists();

        if !unchanged {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create staging directory {}", parent.display())
                })?;
            }

            let oid = U16CString::from_str(&file.object_id)
                .map_err(|e| anyhow!("Invalid MTP object id '{}': {e}", file.object_id))?;
            let obj = content
                .object_by_id(oid)
                .map_err(|e| anyhow!("Failed to get MTP object '{}': {e:?}", file.object_id))?;

            let mut stream = obj
                .open_read_stream()
                .map_err(|e| anyhow!("Failed to open MTP read stream: {e:?}"))?;
            let mut output = fs::File::create(&dst)
                .with_context(|| format!("Failed to create staged file {}", dst.display()))?;
            io::copy(&mut stream, &mut output).with_context(|| {
                format!("Failed to copy MTP object '{}' to {}", file.object_id, dst.display())
            })?;
            copied_files += 1;
        } else {
            reused_files += 1;
        }

        new_manifest.files.insert(
            key,
            StageManifestEntry {
                source_id: file.object_id.clone(),
                size: file.size,
                mtime_secs: 0,
            },
        );
    }

    prune_stale_staged_files(&old_manifest, &new_manifest, &stage_root)?;
    save_manifest(&manifest_path, &new_manifest)?;

    Ok(StagedPhoneInput {
        device_key,
        display_name: phone.display_name.clone(),
        staged_root: stage_root,
        catalog_db_path: state_dir.join("catalog.sqlite"),
        total_files: files.len(),
        copied_files,
        reused_files,
    })
}

#[cfg(windows)]
fn collect_mtp_files_recursive(
    obj: &winmtp::object::Object,
    selected_prefix: Option<String>,
    depth: usize,
    out: &mut Vec<MtpSourceFile>,
) -> Result<()> {
    use winmtp::object::ObjectType;
    use winmtp::PortableDevices::WPD_OBJECT_SIZE;

    if depth > 32 {
        return Ok(());
    }

    let children = obj
        .children()
        .map_err(|e| anyhow!("Failed to enumerate MTP children: {e:?}"))?;

    for child in children {
        let name = child.name().to_string_lossy();
        if name.is_empty() {
            continue;
        }

        let is_folder = matches!(
            child.object_type(),
            ObjectType::Folder | ObjectType::FunctionalObject
        );

        if is_folder {
            let next_prefix = if let Some(prefix) = selected_prefix.as_ref() {
                Some(format!("{prefix}/{name}"))
            } else if is_phone_media_dir_name(&name) {
                Some(name.clone())
            } else {
                None
            };
            collect_mtp_files_recursive(&child, next_prefix, depth + 1, out)?;
            continue;
        }

        if let Some(prefix) = selected_prefix.as_ref() {
            let rel = PathBuf::from(format!("{prefix}/{name}"));
            let size = child
                .properties(&[WPD_OBJECT_SIZE])
                .and_then(|v| v.get_u64(&WPD_OBJECT_SIZE))
                .unwrap_or(0);
            out.push(MtpSourceFile {
                object_id: child.id().to_string_lossy(),
                rel_path: rel,
                size,
            });
        }
    }

    Ok(())
}

#[cfg(windows)]
fn detect_windows_mtp_phones() -> Result<Vec<DetectedPhone>> {
    use winmtp::Provider;

    let provider = Provider::new().map_err(|e| anyhow!("Failed to initialize MTP provider: {e:?}"))?;
    let devices = provider
        .enumerate_devices()
        .map_err(|e| anyhow!("Failed to enumerate MTP devices: {e:?}"))?;

    Ok(devices
        .into_iter()
        .map(|device| {
            let name = device.friendly_name().to_string();
            DetectedPhone {
                id: device.device_id(),
                display_name: if name.trim().is_empty() {
                    "Unnamed MTP Device".to_string()
                } else {
                    name
                },
                source_kind: PhoneSourceKind::Mtp,
                root_hint: None,
            }
        })
        .collect())
}

#[cfg(not(windows))]
fn detect_mounted_phones() -> Result<Vec<DetectedPhone>> {
    let mut phones = Vec::new();
    let mut seen = HashSet::new();

    for root in linux_mount_roots() {
        if !root.exists() {
            continue;
        }

        if let Some(media_root) = locate_media_root(&root) {
            let canonical = fs::canonicalize(&media_root).unwrap_or(media_root.clone());
            let id = format!("fs:{}", canonical.to_string_lossy());
            if seen.insert(id.clone()) {
                phones.push(DetectedPhone {
                    id,
                    display_name: display_name_from_path(&canonical),
                    source_kind: PhoneSourceKind::MountedFilesystem,
                    root_hint: Some(canonical),
                });
            }
        }

        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(media_root) = locate_media_root(&path) {
                let canonical = fs::canonicalize(&media_root).unwrap_or(media_root.clone());
                let id = format!("fs:{}", canonical.to_string_lossy());
                if seen.insert(id.clone()) {
                    phones.push(DetectedPhone {
                        id,
                        display_name: display_name_from_path(&canonical),
                        source_kind: PhoneSourceKind::MountedFilesystem,
                        root_hint: Some(canonical),
                    });
                }
            }
        }
    }

    Ok(phones)
}

#[cfg(not(windows))]
fn linux_mount_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(user) = std::env::var("USER") {
        roots.push(PathBuf::from(format!("/run/media/{user}")));
        roots.push(PathBuf::from(format!("/media/{user}")));
    }
    roots.push(PathBuf::from("/run/media"));
    roots.push(PathBuf::from("/media"));
    roots.push(PathBuf::from("/mnt"));
    roots
}

fn locate_media_root(base: &Path) -> Option<PathBuf> {
    if has_phone_media_dirs(base) {
        return Some(base.to_path_buf());
    }

    let entries = fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && has_phone_media_dirs(&path) {
            return Some(path);
        }
    }

    None
}

fn collect_filesystem_phone_files(root: &Path) -> Result<Vec<FsSourceFile>> {
    let mut files = Vec::new();

    let candidate_dirs = fs::read_dir(root)
        .with_context(|| format!("Failed to read phone root {}", root.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(is_phone_media_dir_name)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    for dir in candidate_dirs {
        for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let source_path = entry.path().to_path_buf();
            let rel_path = source_path
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_path_buf();

            let metadata = fs::metadata(&source_path)
                .with_context(|| format!("Failed to read metadata for {}", source_path.display()))?;
            let mtime_secs = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            files.push(FsSourceFile {
                source_path,
                source_id: normalize_rel_key(&rel_path),
                rel_path,
                size: metadata.len(),
                mtime_secs,
            });
        }
    }

    Ok(files)
}

fn prune_stale_staged_files(old: &StageManifest, new: &StageManifest, stage_root: &Path) -> Result<()> {
    for old_key in old.files.keys() {
        if new.files.contains_key(old_key) {
            continue;
        }
        let stale = stage_root.join(old_key);
        if stale.exists() {
            let _ = fs::remove_file(&stale);
        }
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<StageManifest> {
    if !path.exists() {
        return Ok(StageManifest::default());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read staging manifest {}", path.display()))?;
    let manifest = serde_json::from_str::<StageManifest>(&content)
        .with_context(|| format!("Failed to parse staging manifest {}", path.display()))?;
    Ok(manifest)
}

fn save_manifest(path: &Path, manifest: &StageManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create manifest directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(manifest).context("Failed to serialize staging manifest")?;
    fs::write(path, json)
        .with_context(|| format!("Failed to write staging manifest {}", path.display()))?;
    Ok(())
}

fn has_phone_media_dirs(path: &Path) -> bool {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .map(is_phone_media_dir_name)
            .unwrap_or(false)
    })
}

fn is_phone_media_dir_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    PHONE_MEDIA_DIRS.contains(&lower.as_str())
}

fn make_device_key(device_id: &str) -> String {
    let digest = Sha256::digest(device_id.as_bytes());
    let full = hex::encode(digest);
    full.chars().take(16).collect()
}

fn normalize_rel_key(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    #[cfg(target_os = "windows")]
    {
        value = value.to_lowercase();
    }
    value
}

fn display_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn openarc_local_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("OpenArc")
}

fn phone_state_dir(device_key: &str) -> PathBuf {
    openarc_local_dir().join("phone_state").join(device_key)
}

fn phone_stage_dir(device_key: &str) -> PathBuf {
    openarc_local_dir().join("phone_staging").join(device_key)
}
