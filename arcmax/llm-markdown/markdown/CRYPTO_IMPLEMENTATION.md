# Crypto Implementation for ARC Decryption

## Summary

Successfully fixed the cryptographic implementation for decrypting FreeARC archives encrypted with LZMA2 compression and Blowfish/AES encryption.

## What Was Fixed

### 1. Identified the Root Cause
After analyzing the FreeARC source code (`unarc/Compression/_Encryption/C_Encryption.cpp`), discovered that:

- **FreeARC uses CTR (Counter) mode**, NOT CBC mode
- CTR mode is a **stream cipher mode** - no padding needed
- **Key derivation**: PKCS#5 v2 (PBKDF2-HMAC-SHA512)
- Default iterations: 1000
- CFB mode is also supported, but CTR is most common

### 2. Updated Dependencies (`cargo.toml`)

**Added:**
- `ctr = "0.9"` - For CTR mode encryption
- `cfb-mode = "0.8"` - For CFB mode support
- `pbkdf2 = "0.12"` - For PBKDF2 key derivation
- `hmac = "0.12"` - For HMAC (used in PBKDF2)

**Removed:**
- `block-modes = "0.9"` - No longer needed (was for CBC)

### 3. Fixed `src/core/crypto.rs`

#### Key Derivation (Lines 78-152)
```rust
/// FreeARC uses PKCS#5 v2 (PBKDF2-HMAC-SHA512)
pub struct PasswordDeriver {
    pub iterations: u32,  // default: 1000
}

impl PasswordDeriver {
    pub fn derive_key(&self, password: &str, salt: Option<&[u8]>, key_len: usize) -> Result<Vec<u8>> {
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        let salt_bytes = salt.unwrap_or(&[]);
        let mut key = vec![0u8; key_len];

        pbkdf2_hmac::<Sha512>(
            password.as_bytes(),
            salt_bytes,
            self.iterations,
            &mut key
        );

        Ok(key)
    }
}
```

**Changes:**
- Replaced placeholder SHA-256 hashing with proper PBKDF2-HMAC-SHA512
- Uses same algorithm as FreeARC (confirmed from source)
- Supports configurable iteration count

#### Blowfish Cipher (Lines 154-220)
```rust
/// Blowfish cipher wrapper using CTR mode
pub struct BlowfishCipher {
    key: Vec<u8>,
    iv: Vec<u8>,  // 8 bytes for Blowfish
}

impl BlowfishCipher {
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use blowfish::cipher::{KeyIvInit, StreamCipher};
        use blowfish::Blowfish;
        use ctr::Ctr64LE;  // Little-endian counter (FreeARC uses this)
        use generic_array::GenericArray;

        let key = GenericArray::from_slice(&self.key);
        let iv = GenericArray::from_slice(&self.iv);

        let mut cipher = Ctr64LE::<Blowfish>::new(key, iv);
        let mut buffer = ciphertext.to_vec();
        cipher.apply_keystream(&mut buffer);

        Ok(buffer)
    }
}
```

**Changes:**
- Replaced CBC mode with CTR mode (`Ctr64LE`)
- Removed PKCS#7 padding (not needed for stream ciphers)
- Fixed API usage for RustCrypto crates
- Used little-endian counter mode as per FreeARC specification

#### AES Cipher (Lines 222-314)
```rust
/// AES cipher wrapper using CTR mode
pub struct AesCipher {
    key: Vec<u8>,  // 16, 24, or 32 bytes (AES-128/192/256)
    iv: Vec<u8>,   // 16 bytes
}

impl AesCipher {
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use ctr::Ctr64LE;
        use generic_array::GenericArray;

        let mut buffer = ciphertext.to_vec();

        match self.key.len() {
            16 => {  // AES-128
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<aes::Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            24 => {  // AES-192
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<aes::Aes192>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            32 => {  // AES-256
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<aes::Aes256>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            _ => return Err(anyhow!("Invalid AES key length")),
        }

        Ok(buffer)
    }
}
```

**Changes:**
- Same fixes as Blowfish: CTR mode, no padding
- Support for all AES key sizes (128/192/256-bit)

## Build Status

✅ **Project compiles successfully!**

```bash
$ cargo build
   Compiling arcmax v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Only warnings remain (unused fields/imports), no compilation errors.

## What Still Needs to Be Done

### 1. Parse Encryption Parameters from Archive (TODO)

The encryption method string in FreeARC archives contains parameters like:

```
blowfish-128/ctr:n1000:r0:k<hex_key>:i<hex_iv>:s<hex_salt>:c<hex_code>
```

Where:
- `blowfish-128` = cipher + key size
- `/ctr` = encryption mode (ctr or cfb)
- `n1000` = numIterations for PBKDF2
- `r0` = rounds (0 = default)
- `k<hex>` = key in base16 (hex) encoding
- `i<hex>` = IV in base16
- `s<hex>` = salt in base16
- `c<hex>` = verification code in base16

**Currently:** The code only parses the cipher name ("blowfish", "aes", etc.)

**Needed:** Parse the full method string to extract:
- Encryption mode (ctr/cfb)
- Key size
- Number of iterations
- Key, IV, salt from hex strings

**Location to fix:** `src/core/crypto.rs` - `EncryptionInfo::from_method_string()`

### 2. Handle Key/IV from Archive vs. Password

There are two scenarios:

**Scenario A: Key+IV stored in archive**
- Archive contains pre-computed key and IV (hex-encoded)
- Simply decode from hex and use directly
- No password derivation needed

**Scenario B: Password-based encryption**
- User provides password at runtime
- Archive contains salt
- Derive key+IV using PBKDF2-HMAC-SHA512

**Currently:** The code assumes password-based (Scenario B)

**Needed:** Detect which scenario based on whether `key` field is present in the method string

### 3. Test with Real Encrypted Archives

**Steps:**
1. Create or obtain a test FreeARC archive encrypted with Blowfish+LZMA2
2. Run: `arcmax extract test.arc --password <password>`
3. Verify the extracted files are correct

**Test cases:**
- Blowfish encryption
- AES encryption
- Different PBKDF2 iteration counts
- Cascaded encryption (e.g., "aes+serpent")

## FreeARC Encryption Reference

Based on analysis of `unarc/Compression/_Encryption/C_Encryption.cpp`:

### Supported Ciphers
- **Blowfish**: 8-byte blocks
- **AES (Rijndael)**: 16-byte blocks
- **Twofish**: 16-byte blocks (not yet implemented in Rust)
- **Serpent**: 16-byte blocks (not yet implemented in Rust)

### Encryption Modes
- **CTR (Counter)**: Most common, stream cipher mode
- **CFB (Cipher Feedback)**: Also stream cipher mode

Both modes use little-endian counter: `CTR_COUNTER_LITTLE_ENDIAN`

### Key Derivation Function (PBKDF2)
```c
// From C_Encryption.cpp:154-160
void Pbkdf2Hmac(const BYTE *pwd, int pwdSize,
                const BYTE *salt, int saltSize,
                int numIterations,
                BYTE *key, int keySize) {
    int hash = find_hash("sha512");
    unsigned long ulKeySize = keySize;
    pkcs_5_alg2(pwd, pwdSize, salt, saltSize,
                numIterations, hash, key, &ulKeySize);
}
```

**Rust equivalent:**
```rust
use pbkdf2::pbkdf2_hmac;
use sha2::Sha512;

pbkdf2_hmac::<Sha512>(
    password.as_bytes(),
    salt_bytes,
    iterations,  // typically 1000
    &mut key_buffer
);
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  FreeARC Archive                            │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Directory Block                     │   │
│  │ - encryption_method: "blowfish/ctr"│   │
│  │ - key, iv, salt (hex-encoded)      │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Data Block (encrypted + compressed) │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────┐
│  Decryption Pipeline                        │
│                                             │
│  1. Parse encryption method string          │
│  2. Derive key+IV from password (PBKDF2)    │
│     OR decode from hex if stored            │
│  3. Initialize cipher (Blowfish/AES in CTR) │
│  4. Decrypt block                           │
│  5. Decompress (LZMA2/etc)                  │
│  6. Extract files                           │
└─────────────────────────────────────────────┘
```

## Next Steps

To complete the implementation:

1. **Parse encryption parameters** (see section above)
2. **Implement hex decoding** for keys/IVs from archive
3. **Add CFB mode support** (in addition to CTR)
4. **Test with encrypted archives**
5. Optional: Add Twofish and Serpent cipher support

## References

- FreeARC encryption source: `unarc/Compression/_Encryption/C_Encryption.cpp`
- PBKDF2 implementation: `unarc/Compression/_Encryption/misc/pkcs5/pkcs_5_2.c`
- LibTomCrypt documentation: Used by FreeARC for crypto primitives
- RustCrypto documentation: Modern Rust implementations we're using

## Testing

To test the crypto implementation:

```bash
# Build the project
cd arcmax
cargo build

# Test with an encrypted archive
cargo run -- extract test.arc --password mypassword --output ./extracted

# List encrypted archive contents
cargo run -- list test.arc --password mypassword
```

## Status Summary

| Component | Status | Notes |
|-----------|--------|-------|
| PBKDF2-HMAC-SHA512 | ✅ Complete | Correct algorithm, 1000 iterations |
| Blowfish-CTR | ✅ Complete | Little-endian counter mode |
| AES-CTR | ✅ Complete | All key sizes (128/192/256) |
| CFB mode | ⚠️ Not implemented | Add if needed |
| Parameter parsing | ❌ TODO | Parse method string fully |
| Hex key decoding | ❌ TODO | For archives with stored keys |
| Twofish | ❌ TODO | Low priority |
| Serpent | ❌ TODO | Low priority |
| Real-world testing | ❌ TODO | Need encrypted ARC files |

**Overall: Core crypto implementation is complete and compiles successfully. Ready for testing with real encrypted archives.**
