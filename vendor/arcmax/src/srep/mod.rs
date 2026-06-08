//! Pure Rust port of SREP — huge-dictionary LZ77 preprocessor.
//!
//! SREP finds long-range repeated byte sequences across gigabyte-scale input
//! and encodes them as a compact match list, reducing entropy before a second-
//! stage compressor (LZMA, Tornado, zstd, etc.) finishes the job.
//!
//! Implementation follows `SREP-port-guide.md` and uses the original
//! `freearc_cpp_lib/Compression/SREP/srep.cpp` as the algorithmic specification.
//!
//! ## Module layout
//!
//! ```text
//! srep/
//!   error.rs    — SrepError
//!   types.rs    — Offset, Stat, ChunkId, NOT_FOUND
//!   config.rs   — SrepConfig, Method, HashKind, PerfCounters
//!   format/     — on-disk format: ArchiveHeader, BlockHeader, StatCodec
//! ```
//!
//! ## Implementation stages (see SREP_PORT_PLAN.md)
//! A – foundations (done)
//! B – format layer (done)
//! C – decompressor  (done)
//! D – literal encoder (done)
//! E – rolling hashes + block digests (done)
//! F – HashTable + SliceHash (done)
//! G – FixedMatcher -m3/-m4/-m5 (done)
//! H – InMemDeduplicator -m0 (done)
//! I – CDC matchers -m1/-m2 (todo)
//! J – pipeline (todo)
//! K – performance (todo)

pub mod compress;
pub mod config;
pub mod decompress;
pub mod error;
pub mod format;
pub mod hash;
pub mod matchers;
pub mod types;

pub use compress::{compress, CompressionReport};
pub use config::{
    Acceleration, HashKind, LargePageMode, Method, OutputMode, PerfCounters, SrepConfig,
};
pub use decompress::{decompress, DecompressionReport};
pub use error::SrepError;
pub use types::{ChunkId, Offset, Stat, NOT_FOUND};
