//! Hash infrastructure for SREP.
//!
//! Two kinds of hash are used:
//!
//! 1. **Rolling hashes** (`rolling.rs`) — used by the match-finding engine
//!    (Stages F/G) to find candidate chunk boundaries efficiently as a sliding
//!    window advances through the input.
//!
//! 2. **Block digests** (`digest.rs`) — per-block integrity checksums stored
//!    in block headers. The encoder writes them; the decoder verifies them.

pub mod digest;
pub mod rolling;

pub use digest::{compute_block_digest, verify_block_digest};
pub use rolling::{
    pow_wrapping, CrcRollingHash, PolyRolling64, CRC32_CASTAGNOLI, CRC32_IEEE, PRIME_M3,
    PRIME_M3_ALT,
};
