# Quick Start Implementation Checklist

## Phase 1: Setup (2 minutes)

- [ ] Open `src/codecs/mod.rs`
- [ ] Open `src/formats/freearc.rs`
- [ ] Open `crypto.rs` in a reference window
- [ ] Create backup: `git commit -am "backup before crypto integration"`

## Phase 2: Export Crypto Module (1 minute)

**File: `src/codecs/mod.rs`**

Find this section:
```rust
pub mod ppmd;
pub mod lzma;
```

Add after it:
```rust
pub mod crypto;
```

Also add at the end:
```rust
pub use crypto::{EncryptionInfo, CascadedDecryptor};
```

**Verification:**
```bash
cargo check 2>&1 | grep -i "error" || echo "✓ Module exports OK"
```

## Phase 3: Add Helper Functions (3 minutes)

**File: `src/formats/freearc.rs`**

Find the section with `pub fn decompress_data()`.

Add these three functions BEFORE it:

```rust
/// Check if a method string represents a cipher algorithm
fn is_cipher_method(method: &str) -> bool {
    let cipher_names = ["blowfish", "aes", "twofish", "serpent"];
    cipher_names.iter().any(|&name| {
        method.to_lowercase().starts_with(name)
    })
}

/// Check if a method is a single compression codec
fn is_single_compression_method(method: &str) -> bool {
    let methods = ["lzma", "lz4", "zstd", "deflate", "bzip2", "ppmd"];
    methods.iter().any(|&m| method.to_lowercase().starts_with(m))
}

/// Separate encryption from compression in a method string
fn split_encryption_from_compression(compressor: &str) -> (String, Option<String>) {
    let stages: Vec<&str> = compressor.split('+').collect();
    
    if stages.is_empty() {
        return (compressor.to_string(), None);
    }
    
    // Check if last stage is encryption
    if let Some(last_stage) = stages.last() {
        if is_cipher_method(last_stage) {
            // Last stage is encryption - separate it
            let compression = stages[..stages.len()-1].join("+");
            let encryption = last_stage.to_string();
            return (compression, Some(encryption));
        }
    }
    
    // No encryption found
    (compressor.to_string(), None)
}
```

**Verification:**
```bash
cargo check 2>&1 | grep -i "error" || echo "✓ Helper functions OK"
```

## Phase 4: Replace decompress_data() (5 minutes)

**File: `src/formats/freearc.rs`**

Find the existing:
```rust
pub fn decompress_data(compressor: &str, data: &[u8], expected_size: usize, password: Option<&str>) -> Result<Vec<u8>> {
```

Replace the ENTIRE function with:

```rust
pub fn decompress_data(
    compressor: &str,
    data: &[u8],
    expected_size: usize,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    eprintln!("decompress_data: compressor='{}', data_len={}, expected_size={}",
        compressor, data.len(), expected_size);
    
    // Step 1: Separate encryption from compression
    let (compression_chain, encryption_method) = 
        split_encryption_from_compression(compressor);
    
    if let Some(ref enc) = encryption_method {
        eprintln!("Detected encryption method: {}", enc);
    }
    
    if !compression_chain.is_empty() {
        eprintln!("Compression chain: '{}'", compression_chain);
    }
    
    // Step 2: Decompress using compression chain
    let mut decompressed = if compression_chain.is_empty() {
        eprintln!("No compression in chain, using raw data");
        data.to_vec()
    } else {
        decompress_compression_chain(&compression_chain, data, expected_size)?
    };
    
    // Step 3: Decrypt if encryption exists
    if let Some(enc_method) = encryption_method {
        if let Some(pwd) = password {
            eprintln!("\nApplying decryption...");
            decompressed = decrypt_data(&enc_method, &decompressed, pwd)?;
        } else {
            eprintln!("\nERROR: Archive is encrypted but no password provided");
            return Err(anyhow!("Encrypted archive requires --password argument"));
        }
    }
    
    eprintln!("Final decompressed size: {} bytes", decompressed.len());
    
    Ok(decompressed)
}
```

**Verification:**
```bash
cargo check 2>&1 | grep -i "error" || echo "✓ decompress_data OK"
```

## Phase 5: Add decrypt_data() Function (5 minutes)

**File: `src/formats/freearc.rs`**

Add this import at the top if not present:
```rust
use crate::codecs::crypto::{EncryptionInfo, CascadedDecryptor};
```

Then add this function AFTER decompress_data():

```rust
fn decrypt_data(
    encryption_method: &str,
    ciphertext: &[u8],
    password: &str,
) -> Result<Vec<u8>> {
    eprintln!("\n=== DECRYPTION START ===");
    eprintln!("Method: {}", encryption_method);
    eprintln!("Ciphertext size: {} bytes", ciphertext.len());
    eprintln!("Password: [{}]", "*".repeat(password.len()));
    
    // Parse encryption method string
    let enc_info = EncryptionInfo::from_method_string(encryption_method)
        .map_err(|e| anyhow!("Failed to parse encryption method '{}': {}", encryption_method, e))?;
    
    eprintln!("\nParsed encryption parameters:");
    eprintln!("  Algorithm: {:?}", enc_info.algorithms);
    eprintln!("  Mode: {}", enc_info.mode);
    eprintln!("  Key size: {} bytes", enc_info.key_size);
    eprintln!("  PBKDF2 iterations: {}", enc_info.iterations);
    
    if let Some(ref salt) = enc_info.salt {
        eprintln!("  Salt: {} bytes", salt.len());
    }
    
    if let Some(ref iv) = enc_info.iv {
        eprintln!("  IV: {} bytes", iv.len());
    }
    
    // Create cascaded decryptor and decrypt
    eprintln!("\nInitializing decryptor...");
    let decryptor = CascadedDecryptor::new(&enc_info, password)
        .map_err(|e| anyhow!("Failed to initialize decryptor: {}", e))?;
    
    eprintln!("Decrypting {} bytes...", ciphertext.len());
    let plaintext = decryptor.decrypt(ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;
    
    eprintln!("\n✓ Decryption successful!");
    eprintln!("  Plaintext size: {} bytes", plaintext.len());
    eprintln!("=== DECRYPTION END ===\n");
    
    Ok(plaintext)
}
```

**Verification:**
```bash
cargo check 2>&1 | grep -i "error" || echo "✓ decrypt_data OK"
```

## Phase 6: Build & Test (5 minutes)

**Step 1: Full build**
```bash
cargo build 2>&1 | head -30
```

Expected:
```
   Compiling arcmax v0.1.0
    Finished `dev` profile
```

If errors, check:
- Are imports at top of freearc.rs correct?
- Are function signatures complete?
- Are curly braces balanced?

**Step 2: Run on test archive**
```bash
cargo run -- extract test.arc -o extracted_crypto --password "Fdhzfc1!" 2>&1 | tail -30
```

Expected output:
```
Detected encryption method: blowfish-448/ctr:...
Applying decryption...
=== DECRYPTION START ===
...
✓ Decryption successful!
  Plaintext size: 11044 bytes
=== DECRYPTION END ===
```

**Step 3: Verify extraction**
```bash
ls -lh extracted_crypto/
cat extracted_crypto/CRYPTO_IMPLEMENTATION.md | head -5
```

Expected:
```
-rw-r--r-- 1 user user 11K ... CRYPTO_IMPLEMENTATION.md

# Encryption Implementation
## Overview
...
```

## Phase 7: Troubleshooting (If needed)

### Compilation Errors

**Error: "cannot find type `EncryptionInfo`"**
- Check `src/codecs/mod.rs` has `pub use crypto::EncryptionInfo;`
- Check `src/formats/freearc.rs` has `use crate::codecs::crypto::{EncryptionInfo, CascadedDecryptor};`

**Error: "cannot find function `is_cipher_method`"**
- Make sure the helper functions are BEFORE decompress_data(), not after
- Check function is not nested inside another function

**Error: "`decompress_data` called with wrong number of arguments"**
- Check you're calling it with exactly 4 arguments: `(compressor, data, expected_size, password)`
- Some call sites may need to pass `password` parameter

### Runtime Errors

**Error: "Unsupported final compression method: blowfish-448/ctr"**
- This means the integration didn't work
- Did your binary use the NEW code? Try: `cargo clean && cargo build`
- Check `split_encryption_from_compression()` is being called

**Error: "Failed to parse encryption method"**
- Check crypto.rs `from_method_string()` is working
- Try running: `cargo test crypto_tests -- --nocapture` (from CRYPTO_DEBUG_GUIDE.md)

**Error: "Invalid password"**
- Wrong password provided
- Check password is exact: "Fdhzfc1!" (capital F, lowercase dhzfc, capital c, digit 1)

**Error: "Blowfish IV must be 8 bytes, got X"**
- IV parsing failed in encryption method string
- Check the IV hex string is correct length (should decode to 8 bytes)

## Phase 8: Success Verification

Run this script:

```bash
#!/bin/bash
echo "✓ CRYPTO INTEGRATION VERIFICATION"
echo "─────────────────────────────────"

# 1. Compilation check
echo -n "[1] Compilation... "
cargo build 2>&1 | grep -q "Finished" && echo "✓" || echo "✗"

# 2. File extraction check
echo -n "[2] Extraction... "
rm -rf test_output
cargo run -- extract test.arc -o test_output --password "Fdhzfc1!" > /dev/null 2>&1
[ -f "test_output/CRYPTO_IMPLEMENTATION.md" ] && echo "✓" || echo "✗"

# 3. File size check
echo -n "[3] File size... "
SIZE=$(wc -c < test_output/CRYPTO_IMPLEMENTATION.md 2>/dev/null)
[ "$SIZE" = "11044" ] && echo "✓ ($SIZE bytes)" || echo "✗ ($SIZE bytes, expected 11044)"

# 4. Content check
echo -n "[4] Content... "
grep -q "Encryption" test_output/CRYPTO_IMPLEMENTATION.md && echo "✓" || echo "✗"

echo "─────────────────────────────────"
echo "Integration complete!"
```

Save as `verify_integration.sh` and run:
```bash
chmod +x verify_integration.sh
./verify_integration.sh
```

## Total Time Estimate

- Phase 1: 2 min (Setup)
- Phase 2: 1 min (Export module)
- Phase 3: 3 min (Helper functions)
- Phase 4: 5 min (Replace decompress_data)
- Phase 5: 5 min (Add decrypt_data)
- Phase 6: 5 min (Build & test)
- Phase 7: 5 min (If troubleshooting)

**Total: 25-30 minutes for success** ✓

## Key Reminders

✓ Copy code EXACTLY as shown - no modifications
✓ Test after each phase
✓ Use `cargo check` frequently to catch errors early
✓ If stuck, see CRYPTO_DEBUG_GUIDE.md for detailed help
✓ Your password is "Fdhzfc1!" (exact case!)
✓ Expected file size is 11044 bytes (verify this!)

## Success Signal

When you see this, integration is complete:

```
$ cargo run -- extract test.arc -o output --password "Fdhzfc1!" 2>&1 | grep -A3 "Decryption successful"
✓ Decryption successful!
  Plaintext size: 11044 bytes
=== DECRYPTION END ===

$ cat output/CRYPTO_IMPLEMENTATION.md | head -1
# Encryption Implementation Guide
```

Good luck! 🚀
