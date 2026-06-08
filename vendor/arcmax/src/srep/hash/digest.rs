//! Block digest computation and verification.
//!
//! SREP stores a hash of each block's **decoded** (uncompressed) bytes in the
//! block header immediately after the three u32 fields. On the compress side,
//! the encoder hashes the raw input chunk before writing the header; on the
//! decompress side, the decoder hashes the reconstructed block and compares it
//! against the stored value.
//!
//! Reference: `srep.cpp` lines 1896 (compress) and 3096-3102 (decompress).

use sha2::Digest as Sha2Digest;
use siphasher::sip::SipHasher24;
use std::hash::Hasher;

use crate::srep::config::HashKind;
use crate::srep::error::SrepError;

/// Compute the block integrity digest over `data` using the given hash and seed.
///
/// Returns an empty `Vec` when `hash == HashKind::None`.
/// Returns `Err(SrepError::Unsupported(_))` for hashes not yet implemented.
pub fn compute_block_digest(
    hash: HashKind,
    seed: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, SrepError> {
    match hash {
        HashKind::None => Ok(vec![]),

        HashKind::SipHash => {
            // SipHash-2-4, 8-byte output, 16-byte key.
            // Mirrors `compute_siphash` in srep.cpp line 549.
            if seed.len() != 16 {
                return Err(SrepError::Format("SipHash seed must be exactly 16 bytes"));
            }
            let k0 = u64::from_le_bytes(seed[0..8].try_into().unwrap());
            let k1 = u64::from_le_bytes(seed[8..16].try_into().unwrap());
            let mut h = SipHasher24::new_with_keys(k0, k1);
            h.write(data);
            Ok(h.finish().to_le_bytes().to_vec())
        }

        HashKind::Sha512 => {
            // SHA-512, 64-byte output, no seed.
            // Mirrors `compute_sha512` in srep.cpp line 248.
            let result = sha2::Sha512::digest(data);
            Ok(result.to_vec())
        }

        HashKind::Sha1 => Err(SrepError::Unsupported(
            "SHA-1 block digest not yet implemented",
        )),

        HashKind::Md5 => Err(SrepError::Unsupported(
            "MD5 block digest not yet implemented",
        )),

        HashKind::Vmac => Err(SrepError::Unsupported(
            "VMAC block digest not yet implemented (Stage K)",
        )),
    }
}

/// Verify `stored` digest against a freshly-computed digest for `data`.
///
/// Returns `Ok(())` if the digests match or if `hash == HashKind::None`.
/// Returns `Err(SrepError::Format("block digest mismatch"))` on mismatch.
pub fn verify_block_digest(
    hash: HashKind,
    seed: &[u8],
    data: &[u8],
    stored: &[u8],
) -> Result<(), SrepError> {
    if hash == HashKind::None || stored.is_empty() {
        return Ok(());
    }
    let computed = compute_block_digest(hash, seed, data)?;
    if computed.as_slice() != stored {
        Err(SrepError::Format("block digest mismatch"))
    } else {
        Ok(())
    }
}
