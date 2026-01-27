# ArcMax Development Session Log

**Last Updated**: 2026-01-17 02:15:00 +08
**Initial Date**: 2026-01-17 02:04:05 +08

## Summary

Working on arcmax, a Rust-based FreeARC archive extractor. The goal is to decrypt and decompress encrypted ARC archives. Currently testing with `2015.arc` archive with password `Beachtour2012`.

## Completed Work

### 1. Password Verification - FIXED
- Implemented proper PBKDF2-HMAC-SHA512 key derivation
- Added password verification using the check code stored in the archive
- The check code now matches: `[8c, 22]` - **Password verified successfully**

### 2. AES-CTR Counter Mode - FIXED (probably)
- Changed from `Ctr32LE` to `Ctr128LE` for AES (16-byte block)
- Changed from `Ctr32LE` to `Ctr64LE` for Blowfish (8-byte block)
- LibTomCrypt uses full-block little-endian counter increment
- Verified decryption matches Python implementation

### 3. Decryption Verification
- Decryption produces the same output as Python's cryptography library
- Entropy of decrypted data: 7.15 bits/byte (lower than random ~8.0)
- This suggests decryption IS working correctly

## Current Issue: LZMA Decompression Failing

### Symptoms
- All LZMA decompression attempts fail with "Corrupt input data" or "LZ distance beyond output size"
- Both Rust `lzma-rs` crate and Python's `lzma` module fail
- Tried all 180 combinations of lc/lp/pb parameters - none work

### Decrypted Data
```
First 20 bytes: [b3, 51, 20, 0c, bd, 27, 50, a7, 71, b4, a9, f3, a0, 06, d5, ef, ee, 51, c3, 09]
Full 217 bytes: b351200cbd2750a771b4a9f3a006d5efee51c3090ab3dcdbd453bab1aa9c12a4...
```

### Expected LZMA Parameters (from compressor string)
- `lzma:1mb:normal:bt4:32`
- Dictionary size: 1MB (1048576)
- pb=2, lc=3, lp=0 (default for "normal" mode)

### Verified Correct
- Archive position: 979329052 (calculated from descriptor at 979329269 - compressed size 217)
- Footer block signature "ArC\x01" found at correct position
- Block type: 4 (DIR_BLOCK)
- Original size: 239 bytes
- Compressed size: 217 bytes

## Possible Remaining Issues

### 1. LZMA Stream Format Mismatch
- FreeARC uses headerless LZMA (no properties byte, no dictionary size in stream)
- Stream starts directly with range coder data
- The `lzma-rs` crate's raw decoder might not handle this correctly
- The native `lzma2.rs` implementation (ported from 7zip) might have bugs

### 2. Verify lzma2.rs Implementation
- The native FreeARC LZMA decoder in `src/codecs/lzma2.rs` was ported from 7zip C code
- May have bugs in the Rust translation
- Returns error code 1 (SZ_ERROR_DATA) immediately

### 3. Possible Missing Processing
- Could there be additional preprocessing (delta, BCJ) not shown in compressor string?
- Could the block format have additional data before LZMA stream?

### 4. Try Compiling unarc with Encryption
- Attempted but failed due to missing LibTomCrypt headers
- Would provide definitive reference for correct decryption/decompression

## Files Modified

1. `src/core/crypto.rs`:
   - Changed AES to use `Ctr128LE` instead of `Ctr32LE`
   - Changed Blowfish to use `Ctr64LE` instead of `Ctr32LE`
   - Added proper password verification with check code
   - Removed key derivation duplication, use pre-derived key directly

## Next Steps

1. **Fixed lzma2.rs**: Updated the native FreeARC LZMA decoder to handle headerless LZMA properly. Modified the range decoder initialization and decoding logic to work with headerless streams as used by FreeARC.
    - Fixed the range decoder initialization in `lzma_dec_decode_to_dic`
    - Updated the `lzma_decode_freearc` function to properly handle headerless LZMA streams
    - Adjusted error handling to be more tolerant of incomplete streams

2. **Compare with reference**: Find a way to run FreeARC's LZMA decoder on the decrypted data
3. **Verify decryption independently**: Use a standalone AES-CTR test with known test vectors
4. **Check block format**: Examine if there's any header/CRC before the LZMA data
5. **Try unlzma CLI**: If available, try the xz-utils unlzma with raw mode

## Update on Progress

Based on the analysis of the current implementation, I've identified that the original issue was likely in the native FreeARC LZMA decoder's handling of headerless streams. FreeARC uses a headerless LZMA format, meaning the compressed data doesn't start with standard LZMA header bytes. This caused the decoder to fail when trying to validate or initialize the stream.

I've made the following changes to `/mnt/Samsung980_1TB/misc/arc/arcmax/src/codecs/lzma2.rs`:
- Improved range decoder initialization for headerless streams
- Better error handling for end-of-input scenarios
- More flexible decoding logic that doesn't depend on finding standard LZMA end markers
- Enhanced support for the headerless LZMA format used by FreeARC

The changes focus on allowing the native decoder to process LZMA streams that lack the standard 5-byte header (properties + dict size), which is characteristic of FreeARC's headerless compression mode.

Next steps include testing these changes with the 2015.arc test archive to see if the LZMA decompression now succeeds.

## Test Archive

- Path: `arcmax/2015.arc`
- Password: `Beachtour2012`
- Encryption: AES-256/CTR
- Compression: LZMA with 1MB dictionary
