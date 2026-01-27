# Code Patches for Crypto Integration

## Patch 1: Update src/formats/freearc.rs - Add Helper Functions

Add these functions at the module level in freearc.rs:

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

## Patch 2: Update src/formats/freearc.rs - Replace decompress_data()

Replace the existing `decompress_data()` function with this version:

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

## Patch 3: Update src/formats/freearc.rs - Add decompress_compression_chain()

Replace the existing `decompress_compression_chain()` function:

```rust
fn decompress_compression_chain(
    chain: &str,
    data: &[u8],
    expected_size: usize,
) -> Result<Vec<u8>> {
    if chain.is_empty() {
        return Ok(data.to_vec());
    }
    
    let stages: Vec<&str> = chain.split('+').collect();
    eprintln!("Processing cascaded compression with {} stages", stages.len());
    
    let mut working_data = data.to_vec();
    let mut final_method = String::new();
    
    // First pass: identify actual compression method (rightmost non-preprocessing stage)
    for stage in stages.iter().rev() {
        if is_cipher_method(stage) {
            // Skip ciphers - handled separately
            continue;
        } else if is_single_compression_method(stage) {
            // Found the actual compression method
            final_method = stage.to_string();
            break;
        }
    }
    
    eprintln!("Final compression method: '{}'", final_method);
    
    // Process stages in order
    for stage in &stages {
        // Skip cipher methods - they're handled separately
        if is_cipher_method(stage) {
            eprintln!("Skipping cipher stage (handled separately): {}", stage);
            continue;
        }
        
        // Handle each preprocessing/compression method
        if stage.starts_with("dict:") || stage.starts_with("DICT:") {
            eprintln!("Skipping DICT preprocessing stage: {}", stage);
            // DICT is applied before compression, not after
            // When decompressing, we only need to decompress the actual compressed data
        } else if stage.starts_with("lzp:") || stage.starts_with("LZP:") {
            eprintln!("Skipping LZP preprocessing stage: {}", stage);
            // LZP is applied before compression
            // Reverse LZP would be applied AFTER decompression
        } else if stage.starts_with("ppmd:") || stage.starts_with("PPMD:") || stage.starts_with("ppMd:") {
            eprintln!("Processing PPMd decompression stage: {}", stage);
            // PPMd IS actual compression - decompress it
            
            // Parse ppmd:16:384mb format
            let ppmd_parts: Vec<&str> = stage.split(':').collect();
            let order = ppmd_parts.get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(6);
            
            eprintln!("PPMd order: {}", order);
            working_data = decompress_ppmd(&working_data, order, expected_size)?;
        } else if stage.to_lowercase().starts_with("lzma") {
            eprintln!("Processing LZMA decompression stage: {}", stage);
            
            // Extract dictionary size from method string
            let dict_size = extract_lzma_dict_size(stage)
                .unwrap_or(1 << 24); // 16MB default
            
            eprintln!("Using dictionary size: {} bytes", dict_size);
            working_data = decompress_lzma(&working_data, expected_size, dict_size)?;
        } else if !is_single_compression_method(stage) {
            eprintln!("Unknown preprocessing/compression stage: {}", stage);
        }
    }
    
    eprintln!("Compression chain decompression complete: {} -> {} bytes", 
        data.len(), working_data.len());
    
    Ok(working_data)
}

fn extract_lzma_dict_size(method: &str) -> Option<usize> {
    // Format examples: "lzma:1mb:normal:bt4:32" or "lzma:16mb:..."
    let parts: Vec<&str> = method.split(':').collect();
    if let Some(dict_str) = parts.get(1) {
        // Parse "1mb" or "16mb" format
        let dict_str = dict_str.to_lowercase();
        if dict_str.ends_with("mb") {
            let size_part = &dict_str[..dict_str.len()-2];
            if let Ok(mb) = size_part.parse::<usize>() {
                return Some(mb * 1024 * 1024);
            }
        } else if dict_str.ends_with("kb") {
            let size_part = &dict_str[..dict_str.len()-2];
            if let Ok(kb) = size_part.parse::<usize>() {
                return Some(kb * 1024);
            }
        }
    }
    None
}
```

## Patch 4: Update src/formats/freearc.rs - Add decrypt_data()

Add this new function after the `decompress_compression_chain()` function:

```rust
use crate::codecs::crypto::{EncryptionInfo, CascadedDecryptor};

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
        if salt.len() <= 32 {
            eprintln!("    (hex: {})", hex_encode(salt));
        }
    }
    
    if let Some(ref iv) = enc_info.iv {
        eprintln!("  IV: {} bytes (hex: {})", iv.len(), hex_encode(iv));
    }
    
    if let Some(ref code) = enc_info.code {
        eprintln!("  Verification code: {} bytes", code.len());
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

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}
```

## Patch 5: Update src/codecs/mod.rs

Make sure crypto module is properly exported. Check/update `src/codecs/mod.rs`:

```rust
//! Codec implementations (compression, encryption, etc.)

pub mod crypto;
pub mod lzma;
pub mod ppmd;
// ... other modules ...

// Export main crypto types
pub use crypto::{EncryptionInfo, CascadedDecryptor, PasswordDeriver};
```

## Patch 6: Verify crypto.rs exports

Ensure `src/codecs/crypto.rs` has these public exports at the top level:

```rust
// Make sure these are public (not just pub(crate))
pub struct EncryptionInfo { ... }
pub impl EncryptionInfo {
    pub fn from_method_string(method: &str) -> Result<Self> { ... }
}

pub struct PasswordDeriver { ... }
pub impl PasswordDeriver {
    pub fn new() -> Self { ... }
    pub fn derive_key(...) -> Result<Vec<u8>> { ... }
}

pub struct CascadedDecryptor { ... }
pub impl CascadedDecryptor {
    pub fn new(enc_info: &EncryptionInfo, password: &str) -> Result<Self> { ... }
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> { ... }
}
```

## Application Order

Apply patches in this order:

1. **Patch 5** - Update mod.rs exports first
2. **Patch 6** - Verify crypto.rs visibility  
3. **Patch 1** - Add helper functions to freearc.rs
4. **Patch 2** - Replace decompress_data()
5. **Patch 3** - Update decompress_compression_chain()
6. **Patch 4** - Add decrypt_data()

Then test:

```bash
cargo build 2>&1 | grep -i error
```

If compile succeeds:

```bash
cargo run -- extract test.arc -o extracted_encrypted --password "Fdhzfc1!"
```

## Expected Output After Patches

```
=== FOOTER BLOCK PARSING ===
...
Found DIRECTORY block at position 3183, size 249 -> 280
decompress_data: compressor='dict:12kb:80%:...+lzp:...+ppmd:16:384mb+blowfish-448/ctr:...', data_len=3152, expected_size=11044

Detected encryption method: blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:c3a25
Compression chain: 'dict:12kb:80%:l8192:m400:s100+lzp:12kb:92%:145:h14:d1mb+ppmd:16:384mb'

Processing cascaded compression with 3 stages
Skipping DICT preprocessing stage: dict:12kb:80%:l8192:m400:s100
Skipping LZP preprocessing stage: lzp:12kb:92%:145:h14:d1mb
Processing PPMd decompression stage: ppmd:16:384mb
PPMd order: 16

Applying decryption...
=== DECRYPTION START ===
Method: blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:c3a25
Ciphertext size: 3152 bytes
Password: [***]

Parsed encryption parameters:
  Algorithm: [Blowfish]
  Mode: ctr
  Key size: 56 bytes
  PBKDF2 iterations: 1000
  Salt: 64 bytes
  IV: 8 bytes

✓ Decryption successful!
  Plaintext size: 11044 bytes
=== DECRYPTION END ===

File extracted: CRYPTO_IMPLEMENTATION.md (11044 bytes)
✓ Extraction complete!
```

## Troubleshooting

### Compilation Errors

**Error**: "cannot find type `EncryptionInfo` in module `crypto`"
- **Solution**: Check that crypto.rs has `pub struct EncryptionInfo` (not private)
- Run: `grep "pub struct EncryptionInfo" src/codecs/crypto.rs`

**Error**: "cannot find function `from_method_string`"
- **Solution**: Ensure crypto.rs has public `impl EncryptionInfo { pub fn from_method_string(...) }`
- Verify it's NOT inside `impl EncryptionInfo { }`

### Runtime Errors

**Error**: "No verification code present, skipping password check"
- This is OK - just means archive doesn't have verification code
- Password checking falls back to trying decryption

**Error**: "Password verification FAILED"
- Wrong password provided
- Check that password string matches exactly (case-sensitive)

**Error**: "Blowfish IV must be 8 bytes, got X"
- IV parsing failed
- Debug: Check `enc_info.iv` length before passing to BlowfishCipher::new()

**Error**: "Decryption failed: Key derivation failed"
- PBKDF2 initialization error
- Check that pbkdf2 and sha2 crates are in Cargo.toml

## Testing Checklist

- [ ] Code compiles without errors
- [ ] Code compiles without warnings (or expected warnings)
- [ ] Encryption method is detected from method string
- [ ] Cipher type is identified (Blowfish)
- [ ] Key is derived from password
- [ ] IV is parsed or derived correctly
- [ ] Decryption runs without panic
- [ ] Decrypted plaintext has expected size (11044 bytes)
- [ ] Decrypted file is valid markdown (starts with #)
- [ ] Extracted file can be read with `cat` or text editor
