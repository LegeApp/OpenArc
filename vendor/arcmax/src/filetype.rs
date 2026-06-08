//! File-type detection and codec pipeline suggestions.
//!
//! This module classifies files by extension and magic-byte header, then
//! recommends a `Method` (filter + compressor) tuned for each class.
//!
//! The suggestions are a *starting point*: they are based on known structural
//! properties of each file type (e.g. 16-bit pixel delta for RAW, call-target
//! normalisation for executables) and tested against representative fixtures in
//! `tests/filetype_headsup.rs`. Benchmarking on a real corpus is needed to
//! fine-tune the choices for a specific workload.
//!
//! # Usage
//!
//! ```rust,ignore
//! use arcmax::filetype::{detect, SuggestionConfig, suggest_pipeline};
//!
//! let hint = detect("photo.dng", &header_bytes);
//! let method = suggest_pipeline(hint, &SuggestionConfig::default());
//! let mut pipeline = CodecPipeline::new(method);
//! ```

use std::collections::HashMap;

use crate::codec::filters::{DeltaOptions, DispackOptions, RawBayerOptions};
use crate::codec::lz4::Lz4Options;
use crate::codec::lzma::LzmaOptions;
use crate::codec::ppmd::PpmdOptions;
use crate::codec::zstd::ZstdOptions;
use crate::method::Method;
use crate::srep::SrepConfig;

#[cfg(feature = "bsc")]
use crate::codec::BscOptions;

// ── FileTypeHint ──────────────────────────────────────────────────────────────

/// Broad file-type classification used for codec pipeline selection.
///
/// Variants are intentionally coarse — the goal is to pick the right *class*
/// of codec/filter, not to identify every possible format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileTypeHint {
    /// x86/x64 executables, shared libraries, kernel modules
    /// (`.exe`, `.dll`, `.sys`, `.ocx`, `.so`, `.dylib`, `.o`, `.obj`)
    Executable,

    /// Source code, markup, config, logs, structured text
    /// (`.rs`, `.c`, `.py`, `.json`, `.xml`, `.html`, `.txt`, `.csv`, ...)
    Text,

    /// 16-bit camera RAW sensor data
    /// (`.dng`, `.cr2`, `.cr3`, `.nef`, `.arw`, `.orf`, `.rw2`, `.raf`, ...)
    RawImage,

    /// Already-compressed raster images — further compression rarely helps
    /// (`.jpg`, `.png`, `.gif`, `.webp`, `.heic`, `.avif`, `.jxl`)
    CompressedImage,

    /// Uncompressed PCM audio — delta filter reduces entropy significantly
    /// (`.wav`, `.aiff`, `.pcm`)
    AudioPcm,

    /// Already-compressed audio or video — Store wins
    /// (`.mp3`, `.ogg`, `.flac`, `.mp4`, `.mkv`, `.avi`, ...)
    CompressedMedia,

    /// Relational or embedded databases
    /// (`.db`, `.sqlite`, `.sqlite3`, `.mdb`, `.accdb`, `.dbf`)
    Database,

    /// Already-compressed archives — Store wins
    /// (`.zip`, `.gz`, `.bz2`, `.xz`, `.7z`, `.rar`, `.arc`, `.zst`, `.lz4`, ...)
    Archive,

    /// ArcMax native archive format (`.amx`, magic `ARCMAX\x01\x00`).
    ///
    /// Each block is already compressed by the archive writer — re-compressing
    /// at the file level expands the data.
    ArcmaxArchive,

    /// Virtual-machine disk images and raw filesystem images.
    ///
    /// (`.vhd`, `.vhdx`, `.vmdk`, `.qcow`, `.qcow2`, `.img`, `.raw`, `.vdi`)
    ///
    /// Typical workload: gigabyte-to-terabyte scale with substantial *byte-level*
    /// duplication — sparse zero regions, identical OS files placed at multiple
    /// offsets by the guest filesystem, snapshot deltas.  This is SREP's sweet
    /// spot: the long-range dedup pass collapses 10s-100s of MiB before LZMA
    /// finishes the encoding.
    VmImage,

    /// Columnar / analytical data files.
    ///
    /// (`.parquet`, `.arrow`, `.feather`, `.orc`, `.avro`)
    ///
    /// Dictionary-encoded columns produce exact byte sequences that repeat
    /// across row groups.  SREP + Zstd or SREP + LZMA at multi-GB scale.
    ColumnarData,

    /// Uncompressed bundle archives — tar, cpio, ar.
    ///
    /// (`.tar`, `.cpio`, `.ar`, `.a`, `.deb`, `.rpm`)
    ///
    /// Contents are arbitrary, but the *bundle* form encourages long-range
    /// repetition (identical embedded files, repeated headers, license blobs).
    /// SREP + LZMA at GB scale.
    TarBundle,

    /// General binary without a known beneficial filter
    Binary,

    /// No reliable information; inherit the caller's base method unchanged.
    Unknown,
}

impl FileTypeHint {
    /// Returns `true` when the data is very likely already compressed or
    /// encrypted, so further compression will not shrink (and may expand) it.
    pub fn is_already_compressed(self) -> bool {
        matches!(
            self,
            FileTypeHint::CompressedImage
                | FileTypeHint::CompressedMedia
                | FileTypeHint::Archive
                | FileTypeHint::ArcmaxArchive
        )
    }

    /// Returns `true` for file types where a long-range dedup preprocessor
    /// (SREP) plausibly helps when the input is multi-gigabyte.
    ///
    /// SREP is most effective when:
    /// 1. The input is large enough that the downstream LZ coder's dictionary
    ///    (typically 32-256 MiB) cannot span it, AND
    /// 2. The data contains *exact* byte-sequence repetitions at long range
    ///    (sparse zeros, duplicate embedded files, dictionary-encoded blocks).
    ///
    /// Notably this excludes [`FileTypeHint::Text`]: natural-language text is
    /// handled better by PPMd's statistical context model, which generalises
    /// across repeating patterns without needing byte-exact dedup.  Empirical
    /// result: on a 2 GiB CBETA XML corpus, SREP+PPMd matched PPMd alone to
    /// three decimal places (0.143× vs 0.143×) while adding ~15% CPU overhead.
    pub fn benefits_from_srep(self) -> bool {
        matches!(
            self,
            FileTypeHint::VmImage
                | FileTypeHint::ColumnarData
                | FileTypeHint::TarBundle
                | FileTypeHint::Binary // unknown binary — safe to try at scale
        )
    }
}

// ── Extension-based detection ─────────────────────────────────────────────────

/// Classify a file by its extension (case-insensitive).
///
/// Returns `FileTypeHint::Unknown` for unrecognised or missing extensions.
pub fn detect_by_extension(filename: &str) -> FileTypeHint {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        // ── Executables & object files ────────────────────────────────────────
        "exe" | "dll" | "sys" | "ocx" | "com" | "scr" | "so" | "dylib" | "o" | "obj" | "pyd"
        | "efi" => FileTypeHint::Executable,

        // ── Text / source / markup / config ──────────────────────────────────
        "txt" | "log" | "csv" | "tsv" | "xml" | "json" | "yaml" | "yml" | "toml" | "ini"
        | "cfg" | "conf" | "html" | "htm" | "xhtml" | "css" | "md" | "rst" | "tex" | "bib"
        | "rs" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hxx" | "cs" | "java" | "kt"
        | "swift" | "py" | "rb" | "go" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "php" | "lua"
        | "pl" | "pm" | "r" | "sql" | "sh" | "bash" | "zsh" | "fish" | "bat" | "ps1" | "psm1"
        | "psd1" | "nix" | "svelte" | "vue" | "sass" | "scss" | "less" | "diff" | "patch"
        | "srt" | "vtt" | "ass" | "sub" => FileTypeHint::Text,

        // ── Camera RAW (16-bit sensor data) ───────────────────────────────────
        "dng" | "cr2" | "cr3" | "nef" | "arw" | "orf" | "rw2" | "raf" | "3fr" | "pef" | "srw"
        | "x3f" | "iiq" | "mef" | "nrw" | "rwl" | "cap" | "erf" => FileTypeHint::RawImage,

        // ── Compressed / rendered images ──────────────────────────────────────
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "avif" | "jxl" | "jp2"
        | "j2k" => FileTypeHint::CompressedImage,

        // Uncompressed raster — BMP, TIFF, TGA may benefit from delta; treat as
        // Binary for now since they're rarely large in practice.
        "bmp" | "tif" | "tiff" | "tga" | "ppm" | "pgm" | "pbm" | "pnm" => FileTypeHint::Binary,

        // ── Uncompressed PCM audio ────────────────────────────────────────────
        "wav" | "aiff" | "aif" | "aifc" | "pcm" | "caf" | "w64" => FileTypeHint::AudioPcm,

        // ── Already-compressed audio / video ──────────────────────────────────
        "mp3" | "ogg" | "opus" | "flac" | "m4a" | "aac" | "wma" | "ape" | "mka" | "mp4" | "mkv"
        | "avi" | "mov" | "webm" | "wmv" | "flv" | "m2ts" | "mts" | "mxf" | "vob" | "ogv"
        | "3gp" | "3g2" => FileTypeHint::CompressedMedia,
        // ".ts" is ambiguous (TypeScript vs MPEG-2 TS); handled above as Text.

        // ── Databases ─────────────────────────────────────────────────────────
        "db" | "sqlite" | "sqlite3" | "mdb" | "accdb" | "dbf" | "frm" | "ibd" => {
            FileTypeHint::Database
        }

        // ── ArcMax native archive ─────────────────────────────────────────────
        "amx" => FileTypeHint::ArcmaxArchive,

        // ── Already-compressed archives ───────────────────────────────────────
        // .iso / .dmg removed: those are typically *uncompressed* filesystem
        // images with massive internal redundancy — handled as VmImage instead.
        "zip" | "jar" | "war" | "ear" | "apk" | "ipa" | "gz" | "tgz" | "bz2" | "tbz2" | "xz"
        | "txz" | "lzma" | "zst" | "zstd" | "lz" | "lz4" | "lzh" | "lha" | "7z" | "rar" | "arc"
        | "arj" | "cab" | "br" | "snappy" => FileTypeHint::Archive,

        // ── Virtual-machine / disk images ─────────────────────────────────────
        "vhd" | "vhdx" | "vmdk" | "qcow" | "qcow2" | "vdi" | "img" | "raw" | "ima" | "iso"
        | "dmg" | "wim" | "esd" => FileTypeHint::VmImage,

        // ── Columnar / analytical data ────────────────────────────────────────
        "parquet" | "arrow" | "feather" | "orc" | "avro" => FileTypeHint::ColumnarData,

        // ── Uncompressed bundle archives ──────────────────────────────────────
        // .deb and .rpm are technically ar/cpio containers with possibly-compressed
        // payloads, but the outer container is uncompressed — SREP can still find
        // cross-package dedup when bundling multiple .deb/.rpm files.
        "tar" | "cpio" | "ar" | "deb" | "rpm" | "pkg" | "ipk" => FileTypeHint::TarBundle,

        _ => FileTypeHint::Unknown,
    }
}

// ── Magic-byte detection ──────────────────────────────────────────────────────

/// Classify a file from its first few bytes (magic numbers).
///
/// Extension detection is preferred; use this as a fallback for extensionless
/// files or when you have the header bytes available anyway.
///
/// Requires at least 4 bytes; returns `Unknown` for shorter slices.
pub fn detect_by_magic(header: &[u8]) -> FileTypeHint {
    if header.len() < 4 {
        return FileTypeHint::Unknown;
    }

    // Use explicit byte indexing to avoid Rust slice-pattern limitations with
    // embedded wildcards. Patterns are checked in specificity order (longer
    // signatures first) to prevent false positives.

    // 8-byte signatures
    if header.len() >= 8 {
        // ArcMax native archive
        if &header[..8] == b"ARCMAX\x01\x00" {
            return FileTypeHint::ArcmaxArchive;
        }
        // PNG
        if &header[..8] == b"\x89PNG\r\n\x1a\n" {
            return FileTypeHint::CompressedImage;
        }
        // 7-zip
        if &header[..6] == b"7z\xbc\xaf\x27\x1c" {
            return FileTypeHint::Archive;
        }
        // XZ
        if &header[..6] == b"\xfd7zXZ\x00" {
            return FileTypeHint::Archive;
        }
        // VHDX — "vhdxfile" at offset 0
        if &header[..8] == b"vhdxfile" {
            return FileTypeHint::VmImage;
        }
        // Apache Arrow IPC / Feather v2 — "ARROW1" + 2 NUL bytes
        if &header[..6] == b"ARROW1" {
            return FileTypeHint::ColumnarData;
        }
    }

    // 5-byte signatures
    if header.len() >= 5 {
        // QCOW / QCOW2 — "QFI\xfb" + version byte
        if &header[..4] == b"QFI\xfb" {
            return FileTypeHint::VmImage;
        }
        // tar (POSIX ustar) — "ustar" at offset 257.  Too far in for typical
        // 512-byte header checks, so we rely on extension detection only.
    }

    // 4-byte signatures: VMDK sparse extent, Parquet
    if header.len() >= 4 {
        // VMDK sparse extent — "KDMV" magic
        if &header[..4] == b"KDMV" {
            return FileTypeHint::VmImage;
        }
        // Parquet — "PAR1" magic at start (and end) of file
        if &header[..4] == b"PAR1" {
            return FileTypeHint::ColumnarData;
        }
    }

    // 12-byte RIFF sub-type signatures
    if header.len() >= 12 && &header[..4] == b"RIFF" {
        let sub = &header[8..12];
        if sub == b"WAVE" {
            return FileTypeHint::AudioPcm;
        }
        if sub == b"WEBP" {
            return FileTypeHint::CompressedImage;
        }
        if sub == b"AVI " {
            return FileTypeHint::CompressedMedia;
        }
    }

    // 4-byte signatures
    match header[..4] {
        // MZ (DOS/PE executables, DLLs, drivers)
        [0x4D, 0x5A, _, _] => FileTypeHint::Executable,
        // ELF
        [0x7F, b'E', b'L', b'F'] => FileTypeHint::Executable,
        // JPEG
        [0xFF, 0xD8, 0xFF, _] => FileTypeHint::CompressedImage,
        // GIF87a / GIF89a
        [b'G', b'I', b'F', b'8'] => FileTypeHint::CompressedImage,
        // SQLite3 ("SQLi")
        [b'S', b'Q', b'L', b'i'] => FileTypeHint::Database,
        // ZIP / JAR / APK / docx / xlsx etc.
        [0x50, 0x4B, 0x03, 0x04] | [0x50, 0x4B, 0x05, 0x06] | [0x50, 0x4B, 0x07, 0x08] => {
            FileTypeHint::Archive
        }
        // GZip
        [0x1F, 0x8B, _, _] => FileTypeHint::Archive,
        // Bzip2 ("BZh")
        [b'B', b'Z', b'h', _] => FileTypeHint::Archive,
        // RAR ("Rar!")
        [b'R', b'a', b'r', b'!'] => FileTypeHint::Archive,
        // Zstd frame magic (LE 0xFD2FB528)
        [0x28, 0xB5, 0x2F, 0xFD] => FileTypeHint::Archive,
        // LZ4 frame magic (LE 0x184D2204)
        [0x04, 0x22, 0x4D, 0x18] => FileTypeHint::Archive,
        // PDF — no special filter known; treat as Binary
        [b'%', b'P', b'D', b'F'] => FileTypeHint::Binary,
        // MP3 sync word (0xFFE* or 0xFFF*)
        [0xFF, b, _, _] if b & 0xE0 == 0xE0 => FileTypeHint::CompressedMedia,
        // OGG
        [b'O', b'g', b'g', b'S'] => FileTypeHint::CompressedMedia,
        // FLAC
        [b'f', b'L', b'a', b'C'] => FileTypeHint::CompressedMedia,
        // MP4 / ISOM — check ftyp at offset 4
        _ if header.len() >= 8 && &header[4..8] == b"ftyp" => FileTypeHint::CompressedMedia,
        _ => FileTypeHint::Unknown,
    }
}

// ── Combined detection ────────────────────────────────────────────────────────

/// Combine extension-based and magic-byte detection.
///
/// Extension wins when it is definitive (`!= Unknown`). Magic bytes are used
/// as a fallback for extensionless files. The extension check is faster and
/// avoids misclassifying TIFF-based RAW files (which share the TIFF magic).
pub fn detect(filename: &str, header: &[u8]) -> FileTypeHint {
    let ext_hint = detect_by_extension(filename);
    if ext_hint != FileTypeHint::Unknown {
        ext_hint
    } else {
        detect_by_magic(header)
    }
}

// ── Suggestion config ─────────────────────────────────────────────────────────

/// The codec to prefer for text / source / markup files.
///
/// Both PPMd and BSC (BWT+QLFC) have strong high-order statistical models that
/// outperform general-purpose LZ codecs on natural language and source code.
/// BSC tends to win on larger blocks (> 1 MiB) but requires the `bsc` Cargo
/// feature and a GCC toolchain.
#[derive(Debug, Clone)]
pub enum TextCodec {
    /// PPMd order-12, 32 MiB context window.
    Ppmd(PpmdOptions),
    /// libbsc BWT+QLFC — requires the `bsc` feature.
    #[cfg(feature = "bsc")]
    Bsc(BscOptions),
    /// Fall back to `SuggestionConfig::default_codec` unchanged.
    Default,
}

impl Default for TextCodec {
    fn default() -> Self {
        TextCodec::Ppmd(PpmdOptions::default())
    }
}

// ── CompressionTarget ─────────────────────────────────────────────────────────

/// High-level compression goal that drives codec selection.
///
/// Passed to [`SuggestionConfig::with_target`] to override the `default_codec`
/// and `text_codec` fields.
///
/// | Target | Speed (typ.) | Ratio (typ.) | Codecs used |
/// |--------|-------------|--------------|-------------|
/// | `Speed` | 300–600 MB/s | ~50–55% | LZ4 / Snappy |
/// | `Balanced` | 80–200 MB/s | ~28–35% | Zstd:3, PPMd:o4 |
/// | `Ratio` | 5–20 MB/s | ~15–25% | LZMA:9, PPMd:o6 (default) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionTarget {
    /// Fastest possible; sacrifices compression ratio.  Uses LZ4.
    Speed,
    /// Balanced speed/ratio trade-off.  Uses Zstd:3 / PPMd:o4.
    #[default]
    Balanced,
    /// Best compression ratio; sacrifices speed.  Uses LZMA:9 / PPMd:o6.
    Ratio,
}

impl CompressionTarget {
    /// The general-purpose codec for this target.
    pub fn default_codec(self) -> Method {
        match self {
            CompressionTarget::Speed => Method::Lz4(Lz4Options::default()),
            CompressionTarget::Balanced => Method::Zstd(ZstdOptions { level: 3 }),
            CompressionTarget::Ratio => Method::Lzma(LzmaOptions {
                level: Some(9),
                ..LzmaOptions::default()
            }),
        }
    }

    /// The text-optimised codec for this target.
    pub fn text_codec(self) -> TextCodec {
        match self {
            CompressionTarget::Speed => TextCodec::Default,
            CompressionTarget::Balanced => TextCodec::Ppmd(PpmdOptions {
                order: 4,
                ..PpmdOptions::default()
            }),
            CompressionTarget::Ratio => TextCodec::Ppmd(PpmdOptions::default()),
        }
    }
}

/// Default size threshold above which SREP is inserted into pipelines that
/// benefit from long-range dedup.  512 MiB.
///
/// Reasoning: a typical LZMA dictionary is 32-256 MiB.  Below 512 MiB the
/// downstream coder's own window covers most of the input already, so SREP's
/// overhead (~5-15% CPU) is rarely repaid by extra compression.  Above 512 MiB
/// the coder is window-limited and SREP routinely recovers 10-30% on dedup-
/// friendly inputs (VM images, columnar data, software bundles).
pub const DEFAULT_SREP_THRESHOLD: u64 = 512 * 1024 * 1024;

/// Drives the behaviour of [`suggest_pipeline`].
#[derive(Debug, Clone)]
pub struct SuggestionConfig {
    /// The general-purpose codec applied after any pre-filter, and used for
    /// types that don't benefit from a specialised codec (Database, Binary, …).
    /// Defaults to LZMA with standard settings.
    pub default_codec: Method,

    /// The codec chosen specifically for text / source / markup files.
    /// Defaults to PPMd.
    pub text_codec: TextCodec,

    /// Approximate uncompressed input size in bytes, when known.
    ///
    /// Drives the decision to insert SREP for [`FileTypeHint::benefits_from_srep`]
    /// types — SREP is added only when the input is at least [`srep_threshold`]
    /// bytes.  `None` disables size-aware SREP insertion (SREP is never added).
    pub input_size_hint: Option<u64>,

    /// Minimum input size for SREP to be inserted.  See [`DEFAULT_SREP_THRESHOLD`].
    pub srep_threshold: u64,

    /// Override the SREP configuration when inserted.  `None` uses the default.
    pub srep: Option<SrepConfig>,

    /// Optional sample bytes from a bundle file (`.tar`, `.cpio`, `.deb`).
    ///
    /// When set and the detected hint is [`FileTypeHint::TarBundle`],
    /// [`suggest_pipeline`] calls [`sniff_tar_inner`] to determine the dominant
    /// inner content type and routes accordingly.  A few KiB of the file start
    /// is enough; the entire archive is not required.
    pub bundle_sample: Option<Vec<u8>>,
}

impl Default for SuggestionConfig {
    fn default() -> Self {
        Self {
            default_codec: Method::Lzma(LzmaOptions::default()),
            text_codec: TextCodec::Ppmd(PpmdOptions::default()),
            input_size_hint: None,
            srep_threshold: DEFAULT_SREP_THRESHOLD,
            srep: None,
            bundle_sample: None,
        }
    }
}

impl SuggestionConfig {
    /// Use PPMd for text files (the default).
    pub fn with_ppmd_text() -> Self {
        Self {
            text_codec: TextCodec::Ppmd(PpmdOptions::default()),
            ..Self::default()
        }
    }

    /// Use BSC for text files.
    ///
    /// BSC typically achieves better ratios than PPMd on blocks > 1 MiB.
    /// Requires the `bsc` Cargo feature.
    #[cfg(feature = "bsc")]
    pub fn with_bsc_text() -> Self {
        Self {
            text_codec: TextCodec::Bsc(BscOptions::default()),
            ..Self::default()
        }
    }

    /// Override the default (non-text) codec.
    pub fn with_default_codec(mut self, codec: Method) -> Self {
        self.default_codec = codec;
        self
    }

    /// Provide the input size so [`suggest_pipeline`] can route through SREP
    /// when appropriate.
    pub fn with_input_size(mut self, bytes: u64) -> Self {
        self.input_size_hint = Some(bytes);
        self
    }

    /// Apply a [`CompressionTarget`] — overrides `default_codec` and `text_codec`.
    ///
    /// Any explicit `with_default_codec` / `with_ppmd_text` call made *after*
    /// this one will override the target's choices.
    pub fn with_target(mut self, target: CompressionTarget) -> Self {
        self.default_codec = target.default_codec();
        self.text_codec = target.text_codec();
        self
    }

    /// Provide sample bytes from a bundle file so [`suggest_pipeline`] can
    /// sniff the inner content type.  Pass the first 128 KiB or so; the full
    /// archive is not required.
    pub fn with_bundle_sample(mut self, data: &[u8]) -> Self {
        self.bundle_sample = Some(data.to_vec());
        self
    }

    /// True when SREP should be inserted for a SREP-friendly hint at the
    /// currently configured input size.
    fn should_use_srep(&self, hint: FileTypeHint) -> bool {
        hint.benefits_from_srep()
            && self
                .input_size_hint
                .is_some_and(|n| n >= self.srep_threshold)
    }

    /// Construct the SREP stage with this config's overrides applied.
    fn srep_stage(&self) -> Method {
        Method::Srep(self.srep.clone().unwrap_or_default())
    }
}

// ── Pipeline suggestion ───────────────────────────────────────────────────────

/// Return the recommended [`Method`] for the given [`FileTypeHint`].
///
/// ## Routing table
///
/// | Hint | Suggested pipeline |
/// |------|--------------------|
/// | `Archive` / `CompressedImage` / `CompressedMedia` | `store` |
/// | `Text` | PPMd (or BSC / `text_codec`) — *no SREP regardless of size* |
/// | `Executable` | `dispack + [srep?] + default_codec` |
/// | `RawImage` | `rawbayer + default_codec` |
/// | `AudioPcm` | `delta:2 + default_codec` |
/// | `VmImage` | `[srep?] + lzma` |
/// | `ColumnarData` | `[srep?] + default_codec` |
/// | `TarBundle` | `[srep?] + default_codec` |
/// | `Database` | `default_codec` |
/// | `Binary` | `[srep?] + default_codec` |
/// | `Unknown` | `default_codec` |
///
/// `[srep?]` means the SREP preprocessor is inserted **only when**
/// `cfg.input_size_hint` is set and `>= cfg.srep_threshold` (default 512 MiB).
/// Text is intentionally exempt: empirical testing on a 2 GiB XML corpus
/// confirmed PPMd matches SREP+PPMd to three decimal places.
pub fn suggest_pipeline(hint: FileTypeHint, cfg: &SuggestionConfig) -> Method {
    // Helper: optionally prepend SREP to a chain.
    let with_srep = |chain: Vec<Method>| -> Method {
        if cfg.should_use_srep(hint) {
            let mut full = Vec::with_capacity(chain.len() + 1);
            full.push(cfg.srep_stage());
            full.extend(chain);
            Method::Pipeline(full)
        } else if chain.len() == 1 {
            chain.into_iter().next().unwrap()
        } else {
            Method::Pipeline(chain)
        }
    };

    match hint {
        // Already compressed — further compression typically expands the data.
        FileTypeHint::CompressedImage
        | FileTypeHint::CompressedMedia
        | FileTypeHint::Archive
        | FileTypeHint::ArcmaxArchive => Method::Store,

        // Call-target normalisation before general-purpose LZ.  At large scale,
        // SREP can dedup the post-Dispack stream across repeated runtime stubs.
        FileTypeHint::Executable => {
            let mut chain = vec![Method::Dispack(DispackOptions::default())];
            if cfg.should_use_srep(FileTypeHint::Binary) || cfg.should_use_srep(hint) {
                // Executable doesn't itself opt into SREP via benefits_from_srep,
                // but large executables (installers, single-binary games) do
                // benefit — inject SREP here when size threshold is crossed.
                if cfg.input_size_hint.is_some_and(|n| n >= cfg.srep_threshold) {
                    chain.push(cfg.srep_stage());
                }
            }
            chain.push(cfg.default_codec.clone());
            Method::Pipeline(chain)
        }

        // 16-bit Bayer pixel delta + byte-split reduces entropy significantly.
        FileTypeHint::RawImage => Method::Pipeline(vec![
            Method::RawBayer(RawBayerOptions::default()),
            cfg.default_codec.clone(),
        ]),

        // Stride-2 delta aligns with 16-bit PCM samples.
        FileTypeHint::AudioPcm => Method::Pipeline(vec![
            Method::Delta(DeltaOptions { stride: 2 }),
            cfg.default_codec.clone(),
        ]),

        // High-order statistical models outperform LZ on natural text.
        // SREP is *not* helpful here: PPMd generalises across long-range pattern
        // repetition without needing byte-exact dedup.
        FileTypeHint::Text => match &cfg.text_codec {
            TextCodec::Ppmd(opts) => Method::Ppmd(*opts),
            #[cfg(feature = "bsc")]
            TextCodec::Bsc(opts) => Method::Bsc(*opts),
            TextCodec::Default => cfg.default_codec.clone(),
        },

        // SREP's classical sweet spot: VHD/VHDX/VMDK/QCOW/raw images.
        // Sparse zero regions + duplicate OS files across snapshots.
        // Force LZMA as the back-end (overriding default_codec) because LZ-style
        // coders extend SREP matches at the byte level.  Use a larger dict so
        // the post-SREP stream has room to find any remaining near-matches.
        FileTypeHint::VmImage => with_srep(vec![Method::Lzma(LzmaOptions {
            dict_size: 64 * 1024 * 1024,
            ..LzmaOptions::default()
        })]),

        // Parquet / Arrow / ORC — dictionary-encoded columns repeat exact bytes
        // across row groups; SREP collapses them before the LZ pass.
        FileTypeHint::ColumnarData => with_srep(vec![cfg.default_codec.clone()]),

        // Uncompressed bundle archives — route by inner content when a sample
        // is available; fall back to SREP + default codec otherwise.
        FileTypeHint::TarBundle => {
            if let Some(sample) = &cfg.bundle_sample {
                let inner = sniff_tar_inner(sample);
                // Re-enter suggest_pipeline with the inner hint (no bundle_sample
                // to avoid recursion) and the same config otherwise.
                let inner_cfg = SuggestionConfig {
                    bundle_sample: None,
                    ..cfg.clone()
                };
                suggest_pipeline(inner, &inner_cfg)
            } else {
                with_srep(vec![cfg.default_codec.clone()])
            }
        }

        // Unknown binary at large scale — SREP is a safe bet (can't hurt much,
        // can help a lot).  Below threshold it adds nothing and is skipped.
        FileTypeHint::Binary => with_srep(vec![cfg.default_codec.clone()]),

        // No known beneficial filter — use the default codec directly.
        FileTypeHint::Database | FileTypeHint::Unknown => cfg.default_codec.clone(),
    }
}

// ── Bundle content sniffing ───────────────────────────────────────────────────

/// Sniff the dominant inner content type of a TAR archive from a data sample.
///
/// Reads up to 32 regular-file entries, classifies each by filename extension,
/// and returns the plurality [`FileTypeHint`].  Falls back to [`FileTypeHint::TarBundle`]
/// when the sample is too short, the archive is empty, or no clear winner exists.
///
/// A few kilobytes of the archive's start is sufficient — TAR entries begin at
/// 512-byte-aligned block boundaries and the filename is in the first 100 bytes
/// of each header.
pub fn sniff_tar_inner(data: &[u8]) -> FileTypeHint {
    const BLOCK: usize = 512;
    const MAX_ENTRIES: usize = 32;

    let mut pos = 0;
    let mut votes: HashMap<FileTypeHint, u32> = HashMap::new();
    let mut entries_seen = 0;

    while pos + BLOCK <= data.len() && entries_seen < MAX_ENTRIES {
        let block = &data[pos..pos + BLOCK];

        // All-zero block signals end of archive.
        if block.iter().all(|&b| b == 0) {
            break;
        }

        // Filename: bytes 0–99, null-terminated.
        let name_end = block[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = std::str::from_utf8(&block[..name_end]).unwrap_or("");

        // File size: bytes 124–135, null/space-terminated octal ASCII.
        let size_field = &block[124..136];
        let size_end = size_field
            .iter()
            .position(|&b| b == 0 || b == b' ')
            .unwrap_or(12);
        let file_size = usize::from_str_radix(
            std::str::from_utf8(&size_field[..size_end])
                .unwrap_or("0")
                .trim(),
            8,
        )
        .unwrap_or(0);

        // Type flag: byte 156.
        let type_flag = block[156];

        let data_blocks = (file_size + BLOCK - 1) / BLOCK;

        match type_flag {
            b'0' | 0 => {
                // Regular file.
                if !name.is_empty() {
                    let hint = detect_by_extension(name);
                    if hint != FileTypeHint::Unknown {
                        *votes.entry(hint).or_insert(0) += 1;
                    }
                    entries_seen += 1;
                }
            }
            b'5' => {
                // Directory entry — skip without counting.
            }
            _ => {
                // GNU long name (L), PAX header (x/g), symlink (2), etc. — skip.
            }
        }

        pos += BLOCK + data_blocks * BLOCK;
    }

    if votes.is_empty() {
        return FileTypeHint::TarBundle;
    }

    votes
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(hint, _)| hint)
        .unwrap_or(FileTypeHint::TarBundle)
}
