Below is the porting strategy I would use. The main point: **do not port this as “C++ with Rust syntax.”** Treat the C++ as an executable specification of SREP’s archive format and match-finding behavior, then rebuild the implementation around Rust ownership, typed records, checked offsets, and explicit pipelines.

## 1. What the code is doing

SREP is a **huge-dictionary LZ77 preprocessor**. It does not primarily “compress” like zstd/deflate; it rewrites the input into literals plus long-distance matches so that a later compressor can handle the transformed stream better. The core C++ code defines SREP archive constants, methods `-m0..-m5`, block headers, `STAT`-encoded LZ records, rolling hashes, digest/hash choices, and the global `HashTable` used for cross-file match discovery. 

The uploaded code splits into these important units:

| C++ file             | Role in the Rust port                                                                                                                                                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `srep.cpp`           | Archive format, CLI/library orchestration, global match table, hashing, decompression, background I/O/compression thread.                                                                              |
| `compress.cpp`       | Fixed-size block global match search for slower/stronger methods, with bit-array probing, rolling hashes, prefetching, and statbuf output.                                                             |
| `compress_cdc.cpp`   | Content-defined chunking for `-m1/-m2`, including “fast” CDC and ZPAQ-like CDC, plus per-thread chunk hash calculation.                                                                                |
| `compress_inmem.cpp` | Sliding in-memory dictionary compressor for `-m0`/dictionary mode.                                                                                                                                     |
| `srep_lib.h`         | Attempted C API wrapper with callback-based I/O and config fields. This is useful as a shape for a Rust public API, but not as an implementation model.                                                |
| `to-do.txt`          | Important design notes from the original author: universal `Deduplicator`, chunked arrays instead of large contiguous allocations, more parallelism, CDC structure reuse, and stronger hashing ideas.  |

## 2. Port goal: compatibility first, idiomatic performance second

Use two targets:

1. **Compatibility mode**: reads and writes original SREP streams byte-for-byte compatible where feasible.
2. **Native mode**: same conceptual algorithm, but with a cleaner Rust format/version if you want to evolve it later.

Do not start by optimizing. Start by building a correct decoder and a minimal encoder, then add the stronger matchers.

Recommended order:

1. `format`: archive header/footer/block/stat encoding.
2. `decompress`: decode existing `.srep` files.
3. `inmem`: port `DictionaryCompressor` as the simplest match finder.
4. `fixed`: port `compress.cpp` global fixed-block matcher.
5. `cdc`: port `compress_cdc.cpp`.
6. `pipeline`: parallel block scheduling.
7. `simd/prefetch/large-pages`: only after correctness and profiling.

## 3. Crate structure

Use a workspace or a single crate with strict modules:

```text
srep-rs/
  Cargo.toml
  src/
    lib.rs
    error.rs
    config.rs
    format/
      mod.rs
      header.rs
      stat.rs
      block.rs
    io/
      mod.rs
      source.rs
      sink.rs
      mmap.rs
    hash/
      mod.rs
      rolling.rs
      crc.rs
      digest.rs
    matchers/
      mod.rs
      deduplicator.rs
      inmem.rs
      fixed.rs
      cdc.rs
      hash_table.rs
      slice_hash.rs
    pipeline/
      mod.rs
      block_worker.rs
      scheduler.rs
    cli.rs
  src/bin/srep.rs
```

This matters because the C++ file is a monolith: hash algorithms, archive format, error handling, C library callbacks, match storage, and threading are entangled in `srep.cpp`. Rust should separate these from the start.

## 4. Replace C++ global state with explicit context

The C++ uses globals like `pc`, `program_version`, `selected_hash`, and library-mode globals with `setjmp/longjmp` error handling. In Rust, make all of this explicit:

```rust
#[derive(Debug, Clone)]
pub struct SrepConfig {
    pub mode: CommandMode,
    pub method: Method,
    pub chunk_len: usize,
    pub min_match: usize,
    pub block_size: usize,
    pub dict_size: u64,
    pub threads: usize,
    pub hash: HashKind,
    pub compare_digests: bool,
    pub precompute_digests: bool,
    pub use_mmap: bool,
    pub large_pages: LargePageMode,
    pub acceleration: Acceleration,
}

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

pub struct SrepContext {
    pub config: SrepConfig,
    pub counters: PerfCounters,
}
```

The original `srep_lib.h` exposes a large callback/config struct. In Rust, do not mirror it directly. Use traits for I/O and a small FFI wrapper only at the edge. The C header’s callback shape is useful for FFI, but the Rust-native API should be `Read + Seek` / `Write + Seek`, not raw callbacks. 

## 5. Type mapping

Use explicit aliases and newtypes where the C++ uses `typedef`:

```rust
pub type Offset = u64;
pub type Stat = u32;
pub type ChunkId = u32;
pub type HashValue = usize;
pub type StoredHashValue = u32;
pub type BigHash = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chunk(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct FilePos(pub u64);
```

Important rules:

* Use `u64` for file positions and match offsets.
* Use `usize` only for memory indexes and slice lengths.
* Never subtract file positions without `checked_sub` or an invariant-checked helper.
* Keep archive endianness explicit. Do not rely on native-endian casts.
* Do not represent `NOT_FOUND = 0` by overloading valid chunk IDs. Use `Option<ChunkId>` internally, and only encode sentinel values at format boundaries.

## 6. Port the `STAT` match format as typed records

The C++ uses macros `ENCODE_LZ_MATCH` and `DECODE_LZ_MATCH`. Each match stores literal length, match offset, optional high offset word, and length-minus-base. Rounded mode uses three `STAT`s; non-rounded mode uses four. 

In Rust, make this a real type:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzMatch {
    pub src: u64,
    pub dest: u64,
    pub len: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct EncodedMatch {
    pub literal_len: u32,
    pub offset: u64,
    pub len: u32,
}

pub struct StatCodec {
    pub round_matches: bool,
    pub base_len: u32,
}

impl StatCodec {
    pub fn stats_per_match(&self) -> usize {
        if self.round_matches { 3 } else { 4 }
    }

    pub fn encode(&self, out: &mut Vec<Stat>, m: EncodedMatch) -> Result<(), SrepError> {
        let unit = if self.round_matches { self.base_len as u64 } else { 1 };
        if self.round_matches {
            if m.offset % unit != 0 || (m.len as u64) % unit != 0 {
                return Err(SrepError::Format("rounded match not aligned"));
            }
        }

        out.push(m.literal_len);
        out.push((m.offset / unit) as u32);

        if !self.round_matches {
            out.push(((m.offset / unit) >> 32) as u32);
        }

        let stored_len = ((m.len as u64).checked_sub(self.base_len as u64)
            .ok_or(SrepError::Format("match shorter than base length"))?) / unit;

        out.push(stored_len as u32);
        Ok(())
    }
}
```

Do this early. Most bugs in a port will come from offset math, match overlap, or incorrectly decoding stat streams.

## 7. Replace macros and `goto` with small return enums

`compress.cpp` uses macros like `check_match` and `prefetch_and_store_match`, including `goto match_found` to break out of inner loops. 

Rust version:

```rust
enum ProbeOutcome {
    NoMatch,
    MatchRecorded { new_last_match_end: usize },
}

fn try_record_match(
    params: &FixedParams,
    table: &mut HashTable,
    block: &[u8],
    block_start: u64,
    i: usize,
    chunk: ChunkId,
    last_match_end: usize,
    stats: &mut Vec<Stat>,
    literal_bytes: &mut usize,
) -> Result<ProbeOutcome, SrepError> {
    // Equivalent of record_match(...)
    // Return MatchRecorded instead of goto.
    todo!()
}
```

This is both safer and easier to test.

## 8. Rolling hash port

The C++ has:

* `PolynomialHash`
* `PolynomialRollingHash`
* `CrcRollingHash`
* runtime CRC32C detection
* VMAC/SipHash/SHA/MD5 digest descriptors 

Start with trait-based rolling hashes:

```rust
pub trait RollingHash: Clone {
    type Value: Copy + Ord;

    fn new(window: usize, seed: Self::Value) -> Self;
    fn move_to(&mut self, data: &[u8]);
    fn update(&mut self, remove: u8, add: u8);
    fn value(&self) -> Self::Value;
}
```

For polynomial hash, use wrapping arithmetic intentionally:

```rust
#[derive(Clone)]
pub struct PolyRolling64 {
    value: u64,
    prime: u64,
    prime_l: u64,
    window: usize,
}

impl PolyRolling64 {
    pub fn new(window: usize, prime: u64) -> Self {
        let prime_l = pow_wrapping(prime, window as u32);
        Self { value: 0, prime, prime_l, window }
    }

    #[inline(always)]
    pub fn update(&mut self, remove: u8, add: u8) {
        self.value = self.value
            .wrapping_mul(self.prime)
            .wrapping_add(add as u64)
            .wrapping_sub(self.prime_l.wrapping_mul(remove as u64));
    }
}
```

Do not use `checked_*` inside hash arithmetic. The C++ relies on overflow wrapping. In Rust, make that explicit with `wrapping_*`.

For CRC32C:

* use `std::arch` intrinsics behind `#[cfg(target_arch = "x86_64")]`;
* dispatch once at startup using `is_x86_feature_detected!("sse4.2")`;
* provide a table fallback;
* hide the unsafe intrinsic call in a tiny function.

## 9. HashTable design

The C++ `HashTable` owns several large arrays:

* `chunkarr`
* `hasharr`
* `startarr`
* `digestarr`
* `bitarr`
* `SliceHash`
* file/mmap reference
* mode flags for CDC, digest comparison, precompute, etc. 

Rust should split this into mode-specific tables:

```rust
pub struct FixedChunkTable {
    chunk_slots: Box<[ChunkId]>,
    stored_hashes: Box<[StoredHashValue]>,
    bit_filter: Option<BitFilter>,
    digests: Option<ChunkDigests>,
    slice_hash: Option<SliceHash>,
    layout: TableLayout,
}

pub struct CdcChunkTable {
    chunk_slots: Box<[ChunkId]>,
    starts: ChunkStarts,
    digests: Option<ChunkDigests>,
    layout: TableLayout,
}
```

Do not keep every optional array inside one huge struct unless the original archive compatibility forces it. It makes invariants harder.

For large files, strongly consider chunked allocation instead of one massive `Vec`. The original to-do explicitly calls out allocating arrays in 4 MB chunks, partly to avoid large contiguous allocation issues. 

A good abstraction:

```rust
pub struct SegmentedVec<T> {
    segments: Vec<Box<[T]>>,
    segment_len: usize,
}

impl<T: Copy + Default> SegmentedVec<T> {
    pub fn new_len(len: usize, segment_len: usize) -> Self { /* ... */ }
    pub fn get(&self, index: usize) -> T { /* ... */ }
    pub fn set(&mut self, index: usize, value: T) { /* ... */ }
}
```

Use `Vec<T>` for initial implementation. Switch to `SegmentedVec<T>` after correctness tests pass.

## 10. In-memory dictionary compressor

`compress_inmem.cpp` is a good first matcher to port. It has:

* dictionary size and hash size;
* `prepare_buffer`, which selects the maximal rolling hash inside each `L`-byte block and stores `(hash, offset-within-block)`;
* `compress`, which checks the hash table, validates ring-buffer freshness, extends matches backward and forward, and emits match stats. 

Rust design:

```rust
pub struct InMemDeduplicator {
    chunk_len: usize,
    min_match: usize,
    base_len: usize,
    dict_size: usize,
    block_size: usize,
    buffers: usize,
    hash_mask: usize,
    hash_arr: Box<[usize]>,
}

pub struct PreparedBlock {
    // pairs: hash index, best byte offset inside L-window
    entries: Vec<(usize, usize)>,
}

pub trait Deduplicator {
    fn prepare_block(&mut self, block_start: u64, block: &[u8]) -> Result<PreparedBlock, SrepError>;

    fn compress_block(
        &mut self,
        dict: &RingDict,
        block_start: u64,
        block: &[u8],
        prepared: &PreparedBlock,
        out: &mut MatchSink,
    ) -> Result<BlockStats, SrepError>;
}
```

The original to-do also points toward a universal `Deduplicator(prepare_block, compress_block, memreq, errcode)` interface. That is exactly the right Rust abstraction. 

## 11. Ring dictionary instead of pointer arithmetic

The C++ computes `bufstart = buf - dict`, checks wraparound, and uses `DataStart` to reject stale dictionary matches. 

Rust should model the ring explicitly:

```rust
pub struct RingDict {
    buf: Box<[u8]>,
    logical_start: u64,
    write_pos: usize,
}

impl RingDict {
    pub fn physical_index(&self, logical_pos: u64) -> Option<usize> {
        // Return None if logical_pos no longer resides in the ring.
        todo!()
    }

    pub fn window(&self, logical_pos: u64, len: usize) -> Option<RingWindow<'_>> {
        // May be contiguous or split across wrap.
        todo!()
    }
}
```

This removes the nastiest class of C++ bugs: accidentally comparing against overwritten bytes.

For maximum performance, you can later keep a double-mapped ring buffer on platforms where that is available, but do not start there.

## 12. Fixed-block global matcher

`compress.cpp` is the hardest hot loop. It uses:

* two rolling hashes;
* a `bitarr` filter;
* prefetch;
* `LOOKAHEAD` buffers;
* `ACCELERATOR` as a compile-time template parameter;
* preservation/merging of input matches from earlier stages;
* match rounding depending on mode. 

Rust implementation strategy:

### Phase A: clear scalar implementation

Implement the same logic without prefetching, without template specialization, and without unsafe.

```rust
pub struct FixedMatcher {
    params: FixedParams,
    table: FixedChunkTable,
}

pub struct FixedParams {
    pub round_matches: bool,
    pub chunk_len: usize,
    pub min_match: usize,
    pub base_len: usize,
    pub accelerator: usize,
}

impl FixedMatcher {
    pub fn compress_block(
        &mut self,
        block_start: u64,
        block: &[u8],
        input_matches: &[LzMatch],
        out: &mut MatchSink,
    ) -> Result<BlockStats, SrepError> {
        // First correct scalar version.
        todo!()
    }
}
```

### Phase B: specialize acceleration with const generics

The C++ template parameter `ACCELERATOR` should become const generics only after you have tests:

```rust
pub fn compress_fixed<const ACCEL: usize>(
    params: &FixedParams,
    table: &mut FixedChunkTable,
    block_start: u64,
    block: &[u8],
    input: &[LzMatch],
    out: &mut MatchSink,
) -> Result<BlockStats, SrepError> {
    // ACCEL == 0, 1, 2, 4, 8, 16 variants.
    todo!()
}
```

Then dispatch:

```rust
match config.acceleration.probe_stride() {
    0 => compress_fixed::<0>(...),
    1 => compress_fixed::<1>(...),
    2 => compress_fixed::<2>(...),
    4 => compress_fixed::<4>(...),
    8 => compress_fixed::<8>(...),
    _ => compress_fixed::<16>(...),
}
```

This keeps the optimizer benefits of the old template code without forcing the entire matcher to become macro soup.

## 13. Content-defined chunking

`compress_cdc.cpp` has two boundary finders:

1. **Fast CDC**: rolling hash of the last `WINSIZE = 48` bytes; boundary when hash exceeds `maxhash`, giving roughly `1 / L` probability. It processes a stripe in three streams and sorts discovered marks. 
2. **ZPAQ-like CDC**: tracks an order-1 predictor table and updates a hash differently on predicted vs mispredicted bytes. 

Rust API:

```rust
#[derive(Debug, Clone, Copy)]
pub enum CdcMode {
    Fast,
    Zpaq,
}

pub struct CdcParams {
    pub avg_chunk_len: usize,
    pub min_match: usize,
    pub stripe_len: usize,
    pub window: usize,
    pub mode: CdcMode,
}

pub struct ChunkMark {
    pub offset_in_block: usize,
}

pub fn find_chunks(
    mode: CdcMode,
    params: CdcParams,
    block: &[u8],
    base_offset: usize,
    out: &mut Vec<ChunkMark>,
) {
    match mode {
        CdcMode::Fast => find_chunks_fast(params, block, base_offset, out),
        CdcMode::Zpaq => find_chunks_zpaq(params, block, base_offset, out),
    }
}
```

Do not port the “pointer-to-pointer marks array” literally. Use offsets into the block. That gives you bounds checks and removes lifetime problems.

For parallel CDC, use `rayon` or `crossbeam` scoped threads. A stripe job returns:

```rust
pub struct CdcStripeResult {
    pub marks: Vec<usize>,
    pub digests: Vec<Digest128>,
}
```

Then merge results in stripe order. Avoid shared mutable state in worker threads.

## 14. Digest and hash choices

The C++ supports MD5, SHA1, SHA512, VMAC, and SipHash descriptors. It defaults to VMAC and uses VHash as a digest substitute. 

For Rust:

* `sha1` crate only if compatibility requires it.
* `md-5` only for compatibility.
* `sha2` for SHA-512.
* `siphasher` or std-compatible SipHash if exact compatibility is not needed.
* VMAC is the problem. A compatible VMAC implementation may be awkward. For compatibility mode, you may need to port or FFI VMAC. For native mode, prefer BLAKE3 keyed hashing or HighwayHash/AHash-style non-cryptographic hash depending on collision requirements.

Define:

```rust
pub enum HashKind {
    Md5,
    Sha1,
    Sha512,
    SipHash,
    VmacCompat,
    Blake3Native,
}

pub trait BlockDigest {
    const SIZE: usize;
    fn seed_len(&self) -> usize;
    fn compute(&self, input: &[u8], out: &mut [u8]);
}
```

Keep archive hash IDs separate from implementation enum values. The archive format has numeric tags in the C++ descriptor table. 

## 15. Error handling

Replace:

* `exit(code)`
* `goto cleanup`
* `errcode` fields
* `setjmp/longjmp`
* `fprintf(stderr, ...)`

with:

```rust
#[derive(thiserror::Error, Debug)]
pub enum SrepError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid archive format: {0}")]
    Format(&'static str),

    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    #[error("allocation failed for {component}: requested {bytes} bytes")]
    Allocation { component: &'static str, bytes: u64 },

    #[error("integer overflow in {0}")]
    Overflow(&'static str),
}
```

Internal functions return `Result<T, SrepError>`. The CLI converts errors to exit codes. The FFI wrapper converts errors to `i32`.

## 16. I/O and mmap

The C++ switches between `FILE*`, temp files, and `MMAP_FILE`, and background compression/writing is interleaved in `BG_COMPRESSION_THREAD`. 

Rust should define source/sink traits:

```rust
pub trait Input: std::io::Read + std::io::Seek {}
impl<T: std::io::Read + std::io::Seek> Input for T {}

pub trait Output: std::io::Write + std::io::Seek {}
impl<T: std::io::Write + std::io::Seek> Output for T {}
```

For mmap, use `memmap2`, but isolate it:

```rust
pub enum InputStorage {
    Streaming(Box<dyn ReadSeekSend>),
    Mapped(memmap2::Mmap),
}
```

Do not require mmap for correctness. Use it as an optimization path for match verification.

## 17. Threading model

The C++ background thread has fixed double buffers, events, volatile error codes, and manually allocated per-buffer stat/header/hash memory. 

Rust model:

```text
reader thread / stage
  -> bounded channel of InputBlock
worker pool
  -> bounded channel of CompressedBlock
writer stage
```

Use `crossbeam-channel` or `std::sync::mpsc` plus scoped workers. For CPU parallelism, `rayon` is simpler, but a bounded channel pipeline gives better memory control.

Data types:

```rust
pub struct InputBlock {
    pub index: u64,
    pub file_offset: u64,
    pub data: Box<[u8]>,
}

pub struct CompressedBlock {
    pub index: u64,
    pub file_offset: u64,
    pub literal_bytes: usize,
    pub stats: Vec<Stat>,
    pub header: BlockHeader,
    pub payload: Box<[u8]>,
}
```

Important: writing must preserve block order. Workers can finish out of order; writer buffers by `index`.

## 18. Memory allocation and large pages

The C++ uses `BigAlloc`, optional large pages, and raw allocation/free. Several arrays are enormous. Rust defaults are safer but not always ideal.

Start with:

* `Vec<T>` / `Box<[T]>`
* `try_reserve_exact`
* explicit allocation errors
* `bytemuck` only for plain-old-data conversions if needed

Later add:

```rust
pub trait AllocatorPolicy {
    fn alloc_zeroed_u8(&self, len: usize) -> Result<Box<[u8]>, SrepError>;
    fn alloc_zeroed_chunks<T: Copy + Default>(&self, len: usize) -> Result<Box<[T]>, SrepError>;
}
```

Large-page support should be optional and platform-specific. It should not infect algorithm code.

## 19. Unsafe policy

You can make the first correct implementation almost entirely safe Rust.

Allow `unsafe` only in these places:

1. CPU intrinsics for CRC32C/prefetch/SIMD.
2. Mmap creation.
3. Optional large-page allocator.
4. Optional FFI compatibility layer.
5. Maybe highly optimized byte comparison after safe scalar version is validated.

Everything else should be safe.

The original code has pointer arithmetic throughout. In Rust, use slices, offsets, and typed windows. Do not use raw pointers for the matchers unless profiling proves bounds checks are a real bottleneck.

## 20. Fast match extension

The C++ has `find_match_start` and `find_match_end` that walk backward/forward byte-by-byte. 

Rust scalar version:

```rust
fn common_prefix(a: &[u8], b: &[u8], max: usize) -> usize {
    let n = max.min(a.len()).min(b.len());
    let mut i = 0;
    while i < n && a[i] == b[i] {
        i += 1;
    }
    i
}

fn common_suffix(a: &[u8], b: &[u8], max: usize) -> usize {
    let n = max.min(a.len()).min(b.len());
    let mut i = 0;
    while i < n && a[a.len() - 1 - i] == b[b.len() - 1 - i] {
        i += 1;
    }
    i
}
```

Later, replace with chunked `u64` comparison using `align_to::<u64>()` or a safe crate. Keep the scalar version as the correctness reference.

## 21. Public Rust API

A clean Rust API should look like this:

```rust
pub fn compress<R, W>(
    input: R,
    output: W,
    config: SrepConfig,
) -> Result<CompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek;

pub fn decompress<R, W>(
    input: R,
    output: W,
    config: DecompressConfig,
) -> Result<DecompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek;

pub fn inspect<R: Read + Seek>(
    input: R,
) -> Result<ArchiveInfo, SrepError>;
```

Then add FFI separately:

```rust
#[repr(C)]
pub struct srep_params_t {
    // C-compatible mirror, not used internally.
}

#[no_mangle]
pub extern "C" fn srep_run(params: *mut srep_params_t) -> i32 {
    // validate pointer, catch_unwind, convert Result to code
}
```

Do not let the FFI struct become your internal config. The uploaded header’s struct is broad and callback-heavy because it was retrofitted onto C++ code. 

## 22. Testing strategy

Create fixtures before rewriting hot loops.

### Unit tests

* `StatCodec` roundtrip.
* Header parse/write.
* Rolling hash equivalence against C++ vectors.
* CRC fallback vs intrinsic path.
* CDC boundary detection on fixed buffers.
* Ring dictionary wraparound.
* Match extension at block edges.

### Golden tests

Use the original C++ implementation to produce:

```text
small.bin
small.bin.srep
repeated.bin
repeated.bin.srep
random.bin
random.bin.srep
large-boundary.bin
large-boundary.bin.srep
cdc-fast.bin.srep
cdc-zpaq.bin.srep
```

Then test:

1. Rust decoder reproduces original input.
2. Rust encoder output decodes with original C++ decoder.
3. Original C++ encoder output decodes with Rust decoder.
4. Native Rust encode/decode roundtrips.

### Differential testing

For early development:

```rust
proptest! {
    #[test]
    fn stat_codec_roundtrips(matches in arbitrary_match_stream()) {
        // encode -> decode -> compare
    }
}
```

For matchers, compare not byte-for-byte compressed output, but semantic expansion.

## 23. Optimization plan

After correctness:

### First-level optimizations

* `#[inline(always)]` for rolling hash update, bit filter check, stat encode.
* `wrapping_*` arithmetic.
* const generic acceleration variants.
* `Box<[T]>` instead of `Vec<T>` where length is fixed.
* avoid trait objects inside hot loops.
* structure-of-arrays for hash table fields.

### Second-level optimizations

* CRC32C intrinsic dispatch.
* prefetch with `core::arch::x86_64::_mm_prefetch`.
* manual loop unrolling in hash `move_to`.
* compare 8 or 16 bytes at a time for match extension.
* chunked/segmented arrays for huge files.
* optional large pages.

### Third-level optimizations

* mode-specific compiled functions rather than runtime flags.
* pipeline overlap of read/hash/compress/write.
* SIMD CDC scanning.
* stronger cache-line-aware hash table layout.

Do not implement prefetch early. The C++ code is extremely prefetch-conscious, but bad prefetching can regress modern CPUs. Port scalar first, benchmark, then add it behind feature flags.

## 24. Specific C++ constructs to avoid copying

| C++ pattern                           | Rust replacement                                |
| ------------------------------------- | ----------------------------------------------- |
| `#define ENCODE_LZ_MATCH`             | `StatCodec::encode()`                           |
| `goto cleanup`                        | `Result` and RAII                               |
| `setjmp/longjmp`                      | error enum + FFI boundary conversion            |
| `BYTE* marks[]`                       | `Vec<usize>` offsets                            |
| `buf - dict`                          | `RingDict` logical offsets                      |
| `NOT_FOUND = 0`                       | `Option<ChunkId>`                               |
| raw `BigAlloc/BigFree`                | `Box<[T]>`, `Vec<T>`, optional allocator policy |
| global `pc`                           | `PerfCounters` in context                       |
| `volatile int errcode`                | channels + `Result` propagation                 |
| compile-time C++ templates everywhere | const generics only for hot variants            |
| monolithic `HashTable` flags          | mode-specific table structs                     |

## 25. Port sequence I would give to an agent

Use this ordered implementation plan.

### Stage 1: format and decoder

Implement:

```text
format/header.rs
format/stat.rs
format/block.rs
decompress.rs
```

Acceptance:

* can parse SREP signature/version/method/hash metadata;
* can decode stat streams into typed `LzMatch`;
* can decompress simple archives;
* no compression yet.

### Stage 2: minimal encoder

Implement literal-only output and block headers.

Acceptance:

* Rust encoder creates an archive that Rust decoder can restore;
* original decoder compatibility tested if feasible.

### Stage 3: in-memory dictionary matcher

Port `DictionaryCompressor` into `InMemDeduplicator`.

Acceptance:

* repeated data produces matches;
* ring wraparound is tested;
* no unsafe code.

### Stage 4: fixed global matcher

Port scalar `compress.cpp`.

Acceptance:

* match counts and expanded output match the C++ implementation on fixtures;
* acceleration disabled first;
* then add `ACCEL = 1/2/4/8/16`.

### Stage 5: CDC

Port `fast_find_chunks` and `zpaq_find_chunks`.

Acceptance:

* boundary offsets match C++ on test buffers;
* per-chunk digest computation works;
* CDC compressed output expands correctly.

### Stage 6: pipeline

Replace the old background thread model with ordered block pipeline.

Acceptance:

* compression is deterministic;
* memory cap is honored;
* errors from any worker stop the pipeline cleanly.

### Stage 7: performance

Only now add:

* CRC32C intrinsics;
* prefetch;
* segmented arrays;
* large pages;
* SIMD match extension;
* benchmark harness.

## 26. Likely hard parts

The risky areas are:

1. **Archive compatibility**: especially `STAT` encoding, hash seed storage, and method-specific flags.
2. **Offset math**: match source/destination relationship, rounded matches, and future-LZ behavior.
3. **Ring dictionary freshness**: C++ rejects stale matches with tricky wraparound logic. Model this directly instead of transliterating pointer comparisons.
4. **CDC exactness**: boundary positions must match if you want compatibility.
5. **Digest equivalence**: VMAC compatibility may be annoying. Decide early whether exact old archive compatibility matters.
6. **Large-file memory use**: original SREP targets huge data. A naive `Vec` implementation may work for testing but fail on multi-hundred-GB inputs.

## 27. Recommended dependencies

Keep the core lean:

```toml
[dependencies]
thiserror = "2"
byteorder = "1"
crossbeam-channel = "0.5"
rayon = { version = "1", optional = true }
memmap2 = { version = "0.9", optional = true }
sha1 = { version = "0.10", optional = true }
sha2 = { version = "0.10", optional = true }
md-5 = { version = "0.10", optional = true }
blake3 = { version = "1", optional = true }
```

For benchmarking:

```toml
[dev-dependencies]
criterion = "0.5"
proptest = "1"
```

## 28. Bottom line

The best Rust port is:

* **format-compatible where needed**;
* **not pointer-compatible**;
* built around a `Deduplicator` trait;
* using typed `LzMatch`/`StatCodec`;
* using explicit `RingDict` and `HashTable` invariants;
* safe by default;
* optimized later with const generics, CRC32C intrinsics, segmented arrays, and prefetch.

The original author’s own notes point in this same direction: unify compressors behind a deduplicator interface, improve hash table layout, reduce contiguous allocation pressure, and restructure the sequential/parallel flow rather than preserving the old thread/event model. 
