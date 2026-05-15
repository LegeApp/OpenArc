# Visual Integration Flow & Reference

## Current (Broken) Flow

```
Archive → Footer Parsing → Dir Block → Data Block
                                          ↓
                                    decompress_data()
                                          ↓
              ┌───────────────────────────┴───────────────────────────┐
              ↓                                                         ↓
      Method String                                         ciphertext (3152 bytes)
  "dict:12kb+lzp:12kb+                                      
   ppmd:16:384mb+                    ✗ ERROR: Tries to decompress
   blowfish-448/ctr:..."             with "blowfish-448/ctr"
                                      as a codec!
              ↓
      decompress_compression_chain()
              ↓
   ├─ DICT processing
   ├─ LZP processing  
   ├─ PPMd processing (SKIPPED - "Unknown stage")
   └─ Blowfish processing ✗ FAILS
       
       Error: "Unsupported final compression method: blowfish-448/..."
```

## Fixed (Correct) Flow

```
Archive → Footer Parsing → Dir Block → Data Block
                                          ↓
                                    decompress_data()
                                          ↓
              ┌──────────────────────────────────────────────────────┐
              ↓                                        ↓              ↓
     Method String                              Encryption?      Password?
     (from archive)                             (from string)     (from CLI)
              ↓                                        ↓              ↓
      split_encryption_from_compression()
              ↓
    ┌─────────────────────────────────────────────────┐
    ↓                                                 ↓
Compression Chain              Encryption Method
"dict:12kb+lzp:12kb+          "blowfish-448/ctr:
 ppmd:16:384mb"                n1000:r0:i...:s...:c..."
    ↓                                  ↓
    │                          decrypt_data()
    │                                  ↓
    │                    EncryptionInfo::from_method_string()
    │                                  ↓
    │                    ┌─────────────────────────────┐
    │                    ↓          ↓         ↓         ↓
    │                 Algo      Iterations  Salt      IV
    │                Blowfish    1000       (hex)    (hex)
    │                    ↓
    │            PasswordDeriver::derive_key()
    │                    ↓
    │         Key (56 bytes) from PBKDF2-SHA512
    │                    ↓
    │            CascadedDecryptor::new()
    │                    ↓
    │            BlowfishCipher::new(key, iv)
    │                    ↓
    │            BlowfishCipher::decrypt(ciphertext)
    │                    ↓
    │         Plaintext (11044 bytes) ✓
    │
    ├─ decompress_compression_chain()
    │       ↓
    │   DICT preprocessing (skip)
    │       ↓
    │   LZP preprocessing (skip)
    │       ↓
    │   PPMd decompression (decompress)
    │       ↓
    └──→ Output (decompressed) ✓
            ↓
        Combine with plaintext
            ↓
        Final result
```

## Method String Anatomy

Your archive's method string:

```
dict:12kb:80%:l8192:m400:s100
    ↓ (preprocessing)

+lzp:12kb:92%:145:h14:d1mb
    ↓ (preprocessing)

+ppmd:16:384mb
    ↓ (actual compression)

+blowfish-448/ctr:
    n1000:
    r0:
    i4062f442dc757ea7:
    s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:
    c3a25
    ↓ (encryption)
```

### Stage Breakdown

```
┌─────────────────────────────────────────────────────────────────┐
│ PREPROCESSING STAGES (applied during compression)               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ DICT:12kb (Dictionary preprocessing)                            │
│   ├─ 12kb window size                                           │
│   ├─ 80% coverage                                               │
│   └─ LZ77-style literal replacement                            │
│                                                                 │
│ LZP:12kb (LZ77 preprocessing)                                   │
│   ├─ 12kb window                                                │
│   ├─ 92% coverage                                               │
│   └─ Literal/offset encoding                                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ COMPRESSION STAGE (actual codec)                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ PPMd:16:384mb (Prediction by Partial Matching)                  │
│   ├─ Order 16 (context window)                                  │
│   └─ 384MB memory limit                                         │
│                                                                 │
│ This is what compresses the preprocessed data                   │
│ Output: 3152 bytes (from original 11044 bytes)                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ ENCRYPTION STAGE (cipher layer)                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ Blowfish-448 (Cipher algorithm)                                 │
│   ├─ 448-bit key (56 bytes)                                     │
│   └─ 8-byte blocks                                              │
│                                                                 │
│ CTR mode (Cipher mode)                                          │
│   ├─ Counter mode (stream cipher)                               │
│   ├─ 64-bit counter                                             │
│   └─ Little-endian (Ctr64LE)                                    │
│                                                                 │
│ n1000 (Key derivation iterations)                               │
│   └─ PBKDF2 with 1000 rounds of HMAC-SHA512                    │
│                                                                 │
│ r0 (Rounds - Blowfish specific)                                 │
│   └─ Standard Blowfish (16 rounds built-in)                     │
│                                                                 │
│ i4062f442dc757ea7 (Initialization Vector)                       │
│   ├─ 8 bytes (hex decoded)                                      │
│   └─ Used for CTR mode counter start                            │
│                                                                 │
│ s509213c0e61...53 (Salt for key derivation)                     │
│   ├─ 64 bytes (hex decoded)                                     │
│   └─ Mixed with password in PBKDF2                              │
│                                                                 │
│ c3a25 (Verification code)                                       │
│   ├─ 3 bytes (hex decoded)                                      │
│   └─ Used to verify correct password                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow Through Pipeline

### Phase 1: Encrypted Compressed Data

```
Raw from archive (3152 bytes):
┌────────────────────────────────────────────────┐
│ Encrypted ciphertext (Blowfish-CTR encrypted)  │
│ [0x23, 0x4f, 0xa1, 0xd2, 0x45, ...]          │
│                                                │
│ This looks like random garbage                 │
│ Size: 3152 bytes                               │
└────────────────────────────────────────────────┘
```

### Phase 2: Decryption

```
Decrypt with:
  ├─ Password: "Fdhzfc1!"
  ├─ Salt: (64 bytes from archive)
  ├─ Iterations: 1000
  ├─ IV: (8 bytes from archive)
  └─ Algorithm: Blowfish-448-CTR

Output (3152 bytes):
┌────────────────────────────────────────────────┐
│ Decompressed but still compressed data         │
│ [0x5d, 0x00, 0x00, 0x10, 0x00, ...]          │
│                                                │
│ Looks like valid compressed data               │
│ Size: 3152 bytes (unchanged)                   │
└────────────────────────────────────────────────┘
```

### Phase 3: Decompression

```
Decompress with method: "ppmd:16:384mb"
  ├─ Reverse DICT preprocessing (if needed)
  ├─ Reverse LZP preprocessing (if needed)
  └─ PPMd decompression (order 16)

Output (11044 bytes):
┌────────────────────────────────────────────────┐
│ Final plaintext                                │
│ "# Encryption Implementation..."               │
│ "## Overview"                                  │
│ "FreeARC uses advanced encryption..."          │
│                                                │
│ This is readable markdown!                     │
│ Size: 11044 bytes                              │
└────────────────────────────────────────────────┘
```

## Code Organization

```
src/
├── codecs/
│   ├── mod.rs                    ← Export crypto
│   │   pub mod crypto;
│   │   pub use crypto::EncryptionInfo;
│   │
│   ├── crypto.rs                 ← ALREADY EXISTS ✓
│   │   pub struct EncryptionInfo
│   │   pub fn from_method_string()
│   │   pub struct PasswordDeriver
│   │   pub fn derive_key()
│   │   pub struct BlowfishCipher
│   │   pub fn decrypt()
│   │   pub struct CascadedDecryptor
│   │   pub fn new()
│   │
│   ├── lzma.rs
│   ├── ppmd.rs
│   └── ...
│
├── formats/
│   └── freearc.rs                ← NEEDS CHANGES
│       pub fn decompress_data()
│           ├─ split_encryption_from_compression()  [NEW]
│           ├─ is_cipher_method()                    [NEW]
│           ├─ decrypt_data()                        [NEW]
│           │   └─ EncryptionInfo::from_method_string()
│           │   └─ CascadedDecryptor::new()
│           │   └─ CascadedDecryptor::decrypt()
│           │
│           └─ decompress_compression_chain()       [MODIFY]
│               ├─ PPMd decompression
│               └─ LZMA decompression
│
└── main.rs
```

## Test Points

```
Test 1: Method String Parsing
────────────────────────────
Input:  "blowfish-448/ctr:n1000:r0:i...:s...:c..."
        ↓
        EncryptionInfo::from_method_string()
        ↓
Output: ✓ Algorithm: Blowfish
        ✓ Key size: 56 bytes
        ✓ Iterations: 1000
        ✓ IV parsed
        ✓ Salt parsed


Test 2: Password Derivation
────────────────────────────
Input:  password="Fdhzfc1!", salt=(64 bytes), iterations=1000
        ↓
        PasswordDeriver::derive_key()
        ↓
Output: ✓ Key (56 bytes)
        ✓ Key is deterministic (same key for same inputs)
        ✓ Key is different from password


Test 3: Decryption
────────────────────────────
Input:  ciphertext (3152 bytes), key (56 bytes), iv (8 bytes)
        ↓
        BlowfishCipher::decrypt()
        ↓
Output: ✓ Plaintext (3152 bytes)
        ✓ No errors
        ✓ Decrypted data is valid (not garbage)


Test 4: Full Integration
────────────────────────────
Input:  test.arc + password "Fdhzfc1!"
        ↓
        decompress_data("dict:...+blowfish-...", ...)
        ↓
Output: ✓ File: CRYPTO_IMPLEMENTATION.md
        ✓ Size: 11044 bytes
        ✓ Content: Valid markdown
```

## Execution Timeline

```
Timeline (on your encrypted test.arc)
────────────────────────────────────────

T=0ms     Start extraction
          ├─ Open archive
          └─ Read footer

T=10ms    Parse directory block
          ├─ Decompress dir (LZMA)
          └─ Find CRYPTO_IMPLEMENTATION.md

T=50ms    Parse metadata
          ├─ Size: 11044 bytes
          ├─ Compressed: 3152 bytes
          └─ Method: "dict:...+blowfish-..."

T=60ms    Decrypt data (KEY EVENT)
          ├─ Derive key from password (PBKDF2 - ~100ms)
          └─ Blowfish-CTR decrypt (fast - <1ms)

T=165ms   Decompress data
          ├─ PPMd decompression (SLOW - ~800ms)
          └─ Output: 11044 bytes

T=1000ms  Write to disk
          └─ File ready!

Total: ~1 second
```

## Success Criteria Checklist

```
✓ Code compiles
  └─ No compilation errors
  └─ No breaking warnings

✓ Unit tests pass
  └─ test_encryption_info_parsing
  └─ test_password_derivation
  └─ test_split_encryption_from_compression

✓ Decryption executes
  └─ Message: "Decryption successful!"
  └─ Plaintext size: 11044 bytes

✓ File is extracted
  └─ File: CRYPTO_IMPLEMENTATION.md
  └─ Size: 11044 bytes (exact)
  └─ Format: Valid markdown

✓ Content is correct
  └─ First line: "# Encryption Implementation..."
  └─ Readable text (not garbage)
  └─ Can open with text editor
```

---

Use this as a reference while implementing!
