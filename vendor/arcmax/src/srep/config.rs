/// Compression method, matching the `-m0`..`-m5` CLI options of the original SREP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// In-memory sliding-window dict only (`-m0`).
    M0,
    /// Content-defined chunking, fast rolling-hash boundary finder (`-m1`).
    M1,
    /// Content-defined chunking, ZPAQ-style boundary finder (`-m2`).
    M2,
    /// Fixed-size blocks, match verified by precomputed digests (`-m3`). Default.
    M3,
    /// Fixed-size blocks, match verified by re-reading input (`-m4`).
    M4,
    /// Fixed-size blocks, exhaustive byte-level search (`-m5`).
    M5,
}

impl Default for Method {
    fn default() -> Self {
        Method::M3
    }
}

/// Output/index format baked into the archive header version byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Match list in the footer; decompressor seeks back (`-m3` default, v4).
    IndexLz,
    /// Match list inline per block; decompressor reads output file for back-references (v1/v2).
    IoLz,
    /// Match list inline, decompressor holds only future matches in RAM (v3).
    FutureLz,
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::IndexLz
    }
}

/// Which hash algorithm guards block integrity checksums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashKind {
    /// No block hash — fastest, no integrity checking (archive hash_num = 1).
    None,
    /// MD5 — 16-byte output, no seed (hash_num = 0).
    Md5,
    /// SHA-1 — 20-byte output, no seed (hash_num = 2).
    Sha1,
    /// SHA-512 — 64-byte output, no seed (hash_num = 3).
    Sha512,
    /// VMAC-128 keyed — 16-byte output, 32-byte seed (hash_num = 4). Original default.
    /// Not yet implemented; will be added in Stage K.
    Vmac,
    /// SipHash keyed — 8-byte output, 16-byte seed (hash_num = 5). Rust-native default.
    SipHash,
}

impl Default for HashKind {
    fn default() -> Self {
        HashKind::SipHash
    }
}

impl HashKind {
    /// Numeric tag stored in the archive header (bits 8-15 of word 2).
    pub fn archive_num(self) -> u8 {
        match self {
            HashKind::Md5 => 0,
            HashKind::None => 1,
            HashKind::Sha1 => 2,
            HashKind::Sha512 => 3,
            HashKind::Vmac => 4,
            HashKind::SipHash => 5,
        }
    }

    pub fn from_archive_num(n: u8) -> Option<Self> {
        match n {
            0 => Some(HashKind::Md5),
            1 => Some(HashKind::None),
            2 => Some(HashKind::Sha1),
            3 => Some(HashKind::Sha512),
            4 => Some(HashKind::Vmac),
            5 => Some(HashKind::SipHash),
            _ => None,
        }
    }

    /// Number of seed bytes stored in the archive header after the 4 fixed STAT words.
    pub fn seed_len(self) -> usize {
        match self {
            HashKind::Vmac => 32,    // VMAC_KEY_LEN_BYTES
            HashKind::SipHash => 16, // SIPHASH_KEY_LEN_BYTES
            _ => 0,
        }
    }

    /// Number of digest bytes appended to each block header.
    pub fn digest_len(self) -> usize {
        match self {
            HashKind::None => 0,
            HashKind::Md5 => 16,
            HashKind::Sha1 => 20,
            HashKind::Sha512 => 64,
            HashKind::Vmac => 16,   // VMAC_TAG_LEN_BYTES
            HashKind::SipHash => 8, // SIPHASH_TAG_LEN_BYTES
        }
    }
}

/// Large-page allocation policy for hash-table arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LargePageMode {
    #[default]
    Try,
    Force,
    Disable,
}

/// Acceleration level: how aggressively to skip hash table probes.
/// 0 = always probe (slowest, best ratio), 16 = max skipping (fastest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Acceleration(pub u8);

impl Default for Acceleration {
    fn default() -> Self {
        Acceleration(1)
    }
}

impl Acceleration {
    pub fn probe_stride(self) -> usize {
        self.0 as usize
    }
}

/// All tunable parameters for a compression run.
#[derive(Debug, Clone)]
pub struct SrepConfig {
    pub method: Method,
    pub output_mode: OutputMode,
    /// Chunk length `L` in bytes. Must be a power of 2 for fixed-block methods.
    pub chunk_len: usize,
    /// Minimum match length (must be ≥ 16).
    pub min_match: usize,
    /// Input buffer / block size in bytes (default 8 MiB).
    pub block_size: usize,
    /// In-memory dictionary size in bytes (0 = disabled).
    pub dict_size: usize,
    pub threads: usize,
    pub hash: HashKind,
    pub large_pages: LargePageMode,
    pub acceleration: Acceleration,
    pub use_mmap: bool,
}

impl Default for SrepConfig {
    fn default() -> Self {
        // Match FreeARC's -m3 defaults: 512-byte chunks, 512-byte min match.
        //
        // Why not 32/32: the original SREP is designed for *long-range* dedup
        // at gigabyte scale, where byte-level matching is the downstream coder's
        // job (LZMA, PPMd, BSC).  With chunk_len=32 on a 3 GiB corpus we'd
        // build a 1.7 GiB hash table that blows past L3 cache, making every
        // table probe a DRAM hit (~100s of seconds of wall time).
        //
        // chunk_len=512 → ~6 M chunks per GiB → ~30 MiB chunkarr → fits in L3
        // on modern CPUs, and the per-chunk probe cost stays in cache.
        //
        // Trade-off: matches shorter than 512 bytes are missed.  That's fine
        // because the downstream entropy coder handles short repetitions.
        Self {
            method: Method::M3,
            output_mode: OutputMode::IndexLz,
            chunk_len: 512,
            min_match: 512,
            block_size: 8 * 1024 * 1024,
            dict_size: 0,
            threads: 1,
            hash: HashKind::None,
            large_pages: LargePageMode::Try,
            acceleration: Acceleration(1),
            use_mmap: true,
        }
    }
}

impl SrepConfig {
    /// True for `-m3` without dictionary (ROUND_MATCHES in the C++ source).
    /// When true, offset and length are stored divided by `chunk_len` — 3 stats/match.
    pub fn round_matches(&self) -> bool {
        self.method == Method::M3 && self.dict_size == 0
    }

    /// True for `-m3` — digests are precomputed for all chunks before matching.
    pub fn precompute_digests(&self) -> bool {
        self.method == Method::M3
    }

    /// True for `-m0`..`-m3` — matches are confirmed by comparing stored digests.
    pub fn compare_digests(&self) -> bool {
        matches!(
            self.method,
            Method::M0 | Method::M1 | Method::M2 | Method::M3
        )
    }

    /// `BASE_LEN` stored in the archive header: minimum guaranteed match bytes.
    pub fn base_len(&self) -> u32 {
        self.chunk_len.min(self.min_match) as u32
    }
}

/// Counters collected during a compression or decompression run.
#[derive(Debug, Default, Clone)]
pub struct PerfCounters {
    pub max_offset: u64,
    pub find_match: u64,
    pub find_match_memaccess: u64,
    pub check_hasharr: u64,
    pub hash_found: u64,
    pub check_len: u64,
    pub record_match: u64,
    pub total_match_len: u64,
}
