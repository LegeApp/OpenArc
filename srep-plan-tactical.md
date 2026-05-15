# SREP Full Rust Port — Tactical Implementation Plan

## Context

The existing `crates/arcmax` project has C++ FreeARC codec FFI bindings. A secondary use case needs SREP (a huge-dictionary LZ77 preprocessor) for compressing 3+ GB Arrow parquet and ZIP archives, where standard compressors fail because their dictionaries cap at ~256 MB. SREP works across the entire file, making it far more effective at that scale.

The user wants a **pure Rust implementation**, not C++ FFI. The SREP-dev/ folder already has a C++ library scaffold (infrastructure complete, algorithms stubbed out). That scaffold is **not reused** — we build a clean Rust crate from scratch using the C++ source as an executable specification.

Port guide: `D:\Rust-projects\openarc\SREP-port-guide.md` (28 sections, authoritative strategy)  
Algorithm source (specification): `D:\Rust-projects\openarc\crates\arcmax\freearc_cpp_lib\Compression\SREP\srep.cpp` (3244 lines)  
GitHub reference: https://github.com/Intensity/srep

**Goal of initial implementation:** Compression testing on large archives — so correctness and benchmark-ability of `-m3` (fixed block, digest comparison) come first, with full method coverage layered on top incrementally.

---

## Crate location

```
D:\Rust-projects\openarc\crates\arcmax\src\srep\
```

SREP is part of the `arcmax` crate. Do **not** add a standalone `crates/srep`
workspace member; that split creates duplicate ownership and breaks the intended
integration point. `arcmax` exports the module as `arcmax::srep`.

---

## Implementation Stages (A → I)

---

### A. Workspace bootstrap + type foundations

**Files to create:**
- `crates/arcmax/src/srep/mod.rs`
- `crates/arcmax/src/srep/error.rs`
- `crates/arcmax/src/srep/types.rs`
- `crates/arcmax/src/srep/config.rs`

**`crates/arcmax/Cargo.toml` deps (lean start):**
```toml
[dependencies]
thiserror.workspace = true
byteorder = "1"
siphasher = "0.3"
rand = "0.9"
```

**`error.rs`** — `SrepError` enum (see guide §15):
- `Io(#[from] std::io::Error)`
- `Format(&'static str)`
- `Unsupported(&'static str)`
- `Allocation { component, bytes }`
- `Overflow(&'static str)`

**`types.rs`** — explicit aliases and newtypes (guide §5):
```rust
pub type Offset = u64;
pub type Stat = u32;
pub type ChunkId = u32;
pub type HashValue = usize;
pub type StoredHashValue = u32;

pub const NOT_FOUND: ChunkId = ChunkId::MAX;  // never use 0 as sentinel
```
Note: use `Option<ChunkId>` internally, `NOT_FOUND` only at format boundaries.

**`config.rs`** — `SrepConfig`, `Method` enum, `HashKind`, `LargePageMode`, `Acceleration`, `PerfCounters` (guide §4).

**Workspace `Cargo.toml`:**
No additional SREP workspace member is needed. SREP is compiled through
`crates/arcmax`.

**Verification:** `cargo test -p arcmax srep` compiles and runs the SREP module tests.

---

### B. Format layer — archive constants + stat codec

**Files to create:**
```
crates/arcmax/src/srep/format/
  mod.rs
  constants.rs   — archive signatures, version enums, method flags
  stat.rs        — StatCodec, LzMatch, EncodedMatch
  block.rs       — BlockHeader read/write
  header.rs      — ArchiveHeader read/write
```

**`constants.rs`** — from srep.cpp lines 23-62:
```rust
pub const SREP_SIGNATURE: u32 = 0x5045_5253;      // "SREP"
pub const BULAT_SIGNATURE: u32 = 0x72736552;       // archive outer sig
pub const ARCHIVE_HEADER_SIZE: usize = 4;          // STAT words
pub const BLOCK_HEADER_SIZE: usize = 3;
pub const MAX_HEADER_SIZE: usize = 4;
pub const MAX_HASH_SIZE: usize = 256;
pub const MINIMAL_MIN_MATCH: usize = 16;
pub const DEFAULT_MIN_MATCH: usize = 32;
```

Archive header packed format (srep.cpp line ~2418):
```
[0] BULAT_SIGNATURE
[1] SREP_SIGNATURE
[2] packed: bits 0-7 format_version | bits 8-15 hash_num | bits 16-23 seed_size | bits 24-31 (hash_size-16)
[3] BASE_LEN
[4..4+seed_size] hash seed bytes
```
All multi-byte values are **little-endian** (native in C++ on x86 — confirm with golden test).

**`stat.rs`** — `StatCodec` with `encode()` / `decode()` (guide §6). This is the most bug-prone area. Key rules:
- Round-matches mode (`-m3` without dict): 3 stats per match; offset and len divided by `chunk_len`
- Non-round mode: 4 stats; full 64-bit offset in two u32 words
- `len_stored = (len - base_len) / unit`
- `decode()` is the inverse; must detect stat stream exhaustion cleanly

**`block.rs`** — `BlockHeader`: `literal_bytes: u32`, `uncompressed_len: u32`, `stat_size: u32` (0 for FUTURE/INDEX_LZ). Plus optional hash digest bytes after header.

**`header.rs`** — `ArchiveHeader::read()` / `write()` using `byteorder::ReadBytesExt`.

**Unit tests in `stat.rs`:**
- `StatCodec::encode` → `StatCodec::decode` roundtrip for round-match and non-round variants
- Stat stream with multiple matches
- Length below `base_len` returns `Err(Format(...))`

**Verification:** `cargo test -p arcmax format` passes.

---

### C. Decompressor — INDEX_LZ and IO_LZ modes

**Files to create:**
```
crates/arcmax/src/srep/decompress/
  mod.rs         — dispatch on method flags
  io_lz.rs      — reads output file as back-reference source
  index_lz.rs   — reads footer match list, then sequential decode
```

**`mod.rs`** — public entry point:
```rust
pub fn decompress<R, W>(input: R, output: W, cfg: DecompressConfig) 
    -> Result<DecompressionReport, SrepError>
where R: Read + Seek, W: Write + Seek;
```

**`io_lz.rs`** decode loop (srep.cpp lines ~1290-1340):
1. Read and verify `ArchiveHeader`
2. For each block:
   a. Read `BlockHeader`
   b. Read stat array (`stat_size` u32 words) + literal payload
   c. Parse matches with `StatCodec::decode()`
   d. For each match: write literal bytes, then copy `len` bytes from `output` at `src` position
   e. Validate hash digest if present

**`index_lz.rs`** — same block loop, but match list is in the footer:
1. `seek_end` → read footer (6 STAT words backwards)
2. Parse per-block stat sizes from footer
3. Seek to start of stat data, read full stat array
4. Then decode each block as IO_LZ (same copy loop)

**Golden test fixture generation** (do this before writing the decoder):
```
# Run original SREP binary on test files to produce .srep fixtures
# Place in crates/arcmax/tests/fixtures/srep/
srep -m3 small.bin small.bin.srep
srep -m3 repeated.bin repeated.bin.srep
srep -d0 -m3 random.bin random.bin.srep
```
Test: Rust decoder expands each fixture back to original bytes.

**Verification:** `cargo test -p arcmax srep::decompress` passes on all fixtures.

---

### D. Minimal encoder — literal-only pass

**Files to create:**
```
crates/arcmax/src/srep/compress/
  mod.rs
  literal.rs    — encoder with no matching (all literals)
```

**Purpose:** Establishes the write pipeline, archive header/footer, block boundary logic, and stat encoding — all without any matching complexity. A literal-only `.srep` file expands correctly by the decompressor.

**`literal.rs`** logic:
1. Write `ArchiveHeader` with chosen method/hash
2. For each `block_size` chunk of input:
   - Emit `BlockHeader { literal_bytes: block.len(), uncompressed_len: block.len(), stat_size: 0 }`
   - Emit hash digest of block (if hash ≠ None)
   - Emit literal bytes verbatim (no stat entries, so no matches)
3. Write footer if INDEX_LZ mode

**`compress/mod.rs`** public API:
```rust
pub fn compress<R, W>(input: R, output: W, config: SrepConfig)
    -> Result<CompressionReport, SrepError>
where R: Read + Seek, W: Write + Seek;
```

**Test:** Literal-encoded file → Rust decoder → original bytes. Round-trip test with proptest on random input.

**Verification:** `cargo test -p arcmax roundtrip_literal` passes.

---

### E. Hash layer — rolling hashes + block digests

**Files to create:**
```
crates/arcmax/src/srep/hash/
  mod.rs         — RollingHash trait, HashKind enum, BlockDigest trait
  rolling.rs     — PolyRolling64, CrcRollingHash
  digest.rs      — SHA1, SipHash128 (default), optional SHA512/MD5
```

**`RollingHash` trait** (guide §8):
```rust
pub trait RollingHash: Clone {
    type Value: Copy + Ord + std::ops::BitAnd<Output = Self::Value>;
    fn new(window: usize) -> Self;
    fn move_to(&mut self, data: &[u8]);
    fn update(&mut self, remove: u8, add: u8);
    fn value(&self) -> Self::Value;
}
```

**`PolyRolling64`** (srep.cpp lines ~320-427) — polynomial rolling hash:
- `PRIME1 = 1234567891_u64`, `PRIME2 = 987654321_u64` (from C++ source)
- Constructor computes `prime_l = prime.pow(window)` using `wrapping_mul` throughout
- `update`: `v = v.wrapping_mul(prime).wrapping_add(add as u64).wrapping_sub(prime_l.wrapping_mul(remove as u64))`
- **Critical**: use `wrapping_*` everywhere — the C++ relies on overflow

**`CrcRollingHash`** (srep.cpp lines ~464-525):
- Builds `CRCTab[256]` and `RollingCRCTab[256]` at construction
- `update`: `v = CRCTab[(v ^ add) & 0xFF] ^ (v >> 8) ^ RollingCRCTab[remove as usize]`
- Later: add `#[target_feature(enable = "sse4.2")]` CRC32C intrinsic path behind `cfg(target_arch = "x86_64")`

**`BlockDigest` trait** (guide §14):
```rust
pub trait BlockDigest: Send + Sync {
    fn seed_len(&self) -> usize;
    fn output_len(&self) -> usize;
    fn compute(&self, input: &[u8], seed: &[u8], out: &mut [u8]);
}
```
Implementations: `SipHashDigest` (default, `siphasher` crate), `Sha1Digest` (optional feature), `Sha512Digest` (optional feature). VMAC deferred to later milestone.

Add to `Cargo.toml`:
```toml
siphasher = "0.3"
sha1 = { version = "0.10", optional = true }
sha2 = { version = "0.10", optional = true }
md-5 = { version = "0.10", optional = true }
```

**Unit tests:** rolling hash equivalence vectors computed from C++ output; CRC32C table matches expected values.

**Verification:** `cargo test -p arcmax srep::hash` passes.

---

### F. HashTable + SliceHash

**Files to create:**
```
crates/arcmax/src/srep/matchers/
  mod.rs
  hash_table.rs   — FixedChunkTable (chunkarr + hasharr + optional bitarr + optional digestarr)
  slice_hash.rs   — SliceHash bloom-like filter
```

**`FixedChunkTable`** (srep.cpp lines ~776-1176):

Key fields:
```rust
pub struct FixedChunkTable {
    chunk_slots: Vec<u32>,       // chunkarr: hash_bits | chunk_num packed
    stored_hashes: Vec<u32>,     // hasharr: one hash value per chunk
    bit_filter: Option<Vec<u8>>, // bitarr: bloom filter (1 bit per chunk)
    digests: Option<Vec<u8>>,    // digestarr: hash_size bytes * total_chunks
    chunk_len: usize,            // L
    total_chunks: u64,
    hash_mask: usize,            // chunkarr length - 1
}
```

Probing logic (srep.cpp `add_hash0`, lines ~988-1036):
- `first_slot(h) = h & hash_mask`
- `next_slot(h) = (h.wrapping_mul(123456791).wrapping_add(h >> 16)) & hash_mask`
- Chain limit: `MAX_HASH_CHAIN = 12`
- Slot value packing: `value = (hash_bits << chunk_bits) | chunk_id`
- Match confirmation: compare `stored_hash[chunk_id]` first, then full digest bytes if `compare_digests`

Methods:
- `add_hash(&mut self, chunk_id: ChunkId, rolling_hash: u64, digest: Option<&[u8]>) -> Option<ChunkId>`
- `find_match(&self, rolling_hash: u64, digest: Option<&[u8]>) -> Option<ChunkId>`
- `chunk_offset(chunk_id: ChunkId) -> Offset` — `chunk_id as u64 * chunk_len as u64`

**`SliceHash`** (srep.cpp lines ~681-773):
- One `u32` per `L` bytes of input; 4 bits per sub-slice
- `check(pos_in_block) -> bool` — returns false if surrounding sub-slices differ (eliminates ~90% false matches)

**Memory note:** Use `Vec<T>` initially; switch to `SegmentedVec<T>` (from guide §9) for very large files in stage I.

**Unit tests:** `add_hash` then `find_match` on controlled inputs; chaining behavior with hash collisions; `NOT_FOUND` when chain exhausted.

**Verification:** `cargo test -p arcmax hash_table` passes.

---

### G. Fixed global matcher — `-m3`, then `-m4/-m5`

**Files to create:**
```
crates/arcmax/src/srep/matchers/fixed.rs
crates/arcmax/src/srep/matchers/deduplicator.rs   — Deduplicator trait
```

**`Deduplicator` trait** (guide §10, per author's own to-do notes):
```rust
pub trait Deduplicator {
    fn prepare_block(&mut self, block_start: u64, block: &[u8]) -> Result<PreparedBlock, SrepError>;
    fn compress_block(
        &mut self,
        block_start: u64,
        block: &[u8],
        prepared: &PreparedBlock,
        input_storage: &InputStorage,
        out: &mut MatchSink,
    ) -> Result<BlockStats, SrepError>;
    fn memory_required(&self) -> u64;
}
```

**`FixedMatcher`** — Phase A scalar first (guide §12):
- Inner loop: for each position `i` in block, advance rolling hash, check bit_filter, look up `FixedChunkTable`
- On match: call `try_record_match()` returning `ProbeOutcome` enum (no `goto`)
- `try_record_match`: verify with `match_len()`, extend forward/backward using `common_prefix`/`common_suffix`, emit to `MatchSink`
- `match_len()` for `-m3`: compare precomputed digests from `digestarr`
- `match_len()` for `-m4/-m5`: reread `input_storage` at old position and byte-compare

**`common_prefix` / `common_suffix`** (guide §20) — safe scalar first, no unsafe.

**Phase B const-generic acceleration** — only after tests pass:
```rust
pub fn compress_fixed<const ACCEL: usize>(/* ... */) -> Result<BlockStats, SrepError>
```
Dispatch table: `match config.acceleration { 0 => compress_fixed::<0>(...), 1 => ... }`

**`MatchSink`:**
```rust
pub struct MatchSink {
    codec: StatCodec,
    stats: Vec<Stat>,
    literal_buf: Vec<u8>,
    last_match_end: usize,
}
impl MatchSink {
    pub fn record_match(&mut self, block: &[u8], dest: usize, src: Offset, len: u32) -> Result<(), SrepError>;
    pub fn flush_literals(&mut self, block: &[u8]);
}
```

**Wire into `compress/mod.rs`:** dispatch `Method::M3 | M4 | M5 => FixedMatcher`, output INDEX_LZ block format.

**Golden tests:** compress `repeated.bin` with `-m3`, compare decompressed output to original. Compare match counts (not exact compressed bytes) against C++ binary output.

**Verification:** `cargo test -p arcmax fixed_matcher` + manual benchmark on 3 GB test file.

---

### H. In-memory dictionary matcher — `-m0`

**File:** `crates/arcmax/src/srep/matchers/inmem.rs`

**`InMemDeduplicator`** (compress_inmem.cpp, guide §10):
- `hash_arr: Vec<usize>` — maps rolling hash → offset in ring dict
- `RingDict: Vec<u8>` — circular buffer, `logical_start: u64`, `write_pos: usize`
- `prepare_block()` — for each `L`-byte window, compute max rolling hash, store `(hash_index, byte_offset)` in `PreparedBlock`
- `compress_block()` — for each prepared entry: lookup `hash_arr`, validate ring freshness via `RingDict::physical_index()`, extend match, emit

**`RingDict`** (guide §11):
```rust
pub struct RingDict {
    buf: Vec<u8>,
    logical_start: u64,
    write_pos: usize,
    capacity: usize,
}
impl RingDict {
    pub fn physical_index(&self, logical_pos: u64) -> Option<usize>;
    pub fn window(&self, logical_pos: u64, len: usize) -> Option<&[u8]>;  // may split — handle wrap
    pub fn push_block(&mut self, block: &[u8]);
}
```

**Verification:** `cargo test -p arcmax inmem` with repeated-pattern test data; ring wraparound tests.

---

### I. CDC matchers — `-m1` (Fast) and `-m2` (ZPAQ)

**File:** `crates/arcmax/src/srep/matchers/cdc.rs`

**Fast CDC** (compress_cdc.cpp, guide §13):
- Window: 48 bytes, rolling hash > `maxhash = UINT_MAX * 3/4`
- Three parallel streams: each starts at `i=0`, `i=L/3`, `i=2*L/3`
- Collect boundary offsets → sort → deduplicate

**ZPAQ CDC**:
- Order-1 context: `predicted[prev_byte]` table
- Hash update differs on predicted vs. mispredicted bytes

**API:**
```rust
pub enum CdcMode { Fast, Zpaq }
pub fn find_chunks(mode: CdcMode, block: &[u8], avg_len: usize, out: &mut Vec<usize>);
```

**Threading for CDC** — use `rayon::par_iter()` on stripe slices; each stripe returns `Vec<usize>` of boundary offsets; merge in order.

**Verification:** Boundary positions match C++ binary output on test fixtures.

---

### J. Pipeline — parallel block processing

**Files:**
```
crates/arcmax/src/srep/pipeline/
  mod.rs
  scheduler.rs   — block ordering, error aggregation
  worker.rs      — InputBlock → CompressedBlock
```

**Model** (guide §17) — reader → bounded channel → worker pool → bounded channel → writer:
```rust
pub struct InputBlock  { pub index: u64, pub file_offset: u64, pub data: Box<[u8]> }
pub struct CompressedBlock { pub index: u64, pub stats: Vec<Stat>, pub payload: Box<[u8]>, pub header: BlockHeader }
```
- Writer buffers out-of-order blocks by `index` and flushes in order
- Error in any worker → cancel remaining work, drain channels, propagate

**Verification:** multi-threaded compress → decompress roundtrip produces identical output to single-threaded path.

---

### K. Performance layer (last, after all above correct)

- CRC32C intrinsic: `unsafe { core::arch::x86_64::_mm_crc32_u64(...) }` behind `#[target_feature(enable = "sse4.2")]`, dispatched once at runtime with `is_x86_feature_detected!("sse4.2")`
- `#[inline(always)]` on `RollingHash::update`, `StatCodec::encode`, `BitFilter::check`
- `SegmentedVec<T>` for `chunkarr`/`digestarr` in large-file scenarios
- Prefetch: `core::arch::x86_64::_mm_prefetch` behind `unsafe` + feature flag
- VMAC compatibility hash (feature-gated, needed only for reading old `.srep` archives)
- Benchmark harness with `criterion` on 100 MB and 3 GB test inputs

---

## Critical files to reference during implementation

| File | Purpose |
|---|---|
| `D:\Rust-projects\openarc\SREP-port-guide.md` | Authoritative Rust design guide |
| `D:\Rust-projects\openarc\crates\arcmax\freearc_cpp_lib\Compression\SREP\srep.cpp` | Algorithm specification (lines 776–1176 = HashTable; 1290–1340 = decompress; 2413–2850 = compress loop) |
| `D:\Rust-projects\openarc\Cargo.toml` | Workspace — keep SREP inside the existing `arcmax` member |
| `D:\Rust-projects\openarc\crates\zenzstd\` | Pattern for a standalone pure-Rust codec crate in this workspace |

---

## Implementation order summary

```
A  Workspace bootstrap, types, error, config
B  Format layer: constants, StatCodec, BlockHeader, ArchiveHeader
C  Decompressor: IO_LZ + INDEX_LZ (read existing .srep files)
D  Literal-only encoder (establishes write pipeline)
E  Hash layer: rolling hashes, block digest trait + SipHash impl
F  HashTable + SliceHash data structures
G  FixedMatcher (-m3 scalar → const-generic ACCEL → -m4/-m5)  ← benchmark target
H  InMemDeduplicator + RingDict (-m0)
I  CDC matchers + rayon parallelism (-m1/-m2)
J  Pipeline: parallel block reader/worker/writer
K  Performance: CRC32C intrinsic, prefetch, SegmentedVec, VMAC compat
```

Stages A–G unlock benchmarking `-m3` on large archives, which is the stated goal. Stages H–K complete the full method coverage.
