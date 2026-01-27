# Integrating Crypto.rs into Decompression Pipeline

## Problem Analysis

The archive shows the decryption IS recognized:
- Cipher: `blowfish-448/ctr` ✓
- Iterations: `n1000` ✓  
- Salt: `s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53` ✓
- IV: `i4062f442dc757ea7` ✓

But the system treats it as "Unsupported final compression method" instead of handling it as an encryption layer.

## Current Code Flow

### In `src/formats/freearc.rs` decompress_data() function:

```rust
"Processing cascaded compression with 4 stages"
    ↓
"Skipping DICT preprocessing stage"
    ↓
"Skipping LZP preprocessing stage"
    ↓
"Unknown preprocessing stage: ppmd:16:384mb"  ← STOPS HERE
    ↓
"Processing final compression stage: blowfish-448/ctr:..."
    ↓
"Unsupported final compression method" ← ERROR
```

The issue: The code detects `blowfish-...` as a compression method, not as encryption.

## Solution: Reorder Processing

The correct order should be:

```
1. Decompress data (using actual compression codecs: lzma, ppmd, etc.)
2. Apply REVERSE of preprocessing (lzp reverse, dict reverse, etc.)
3. DECRYPT final result (if encryption layer exists)
```

BUT the current structure has encryption mixed into the method string with compression methods.

## Key Insight: Parsing Encrypted Methods

FreeARC method strings can be:
- **Pure compression**: `lzma:1mb:normal:bt4:32`
- **Compression + preprocessing**: `dict:12kb:80%:...+lzp:12kb:92%:...+ppmd:16:384mb`
- **Compression + preprocessing + encryption**: `dict:...+lzp:...+ppmd:...+blowfish-448/ctr:...`

The `+` separates stages. The LAST stage after all `+` that starts with a cipher name is encryption.

## Updated Implementation Steps

### Step 1: Update decompress_data() to separate encryption from compression

In `src/formats/freearc.rs`:

```rust
pub fn decompress_data(
    compressor: &str,
    data: &[u8],
    expected_size: usize,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    eprintln!("decompress_data: compressor='{}', data_len={}, expected_size={}",
        compressor, data.len(), expected_size);
    
    // Split by '+' to separate stages
    let stages: Vec<&str> = compressor.split('+').collect();
    
    // Identify the final stage - check if it's encryption
    let (compression_chain, encryption_method) = 
        if let Some(last_stage) = stages.last() {
            if is_cipher_method(last_stage) {
                // Last stage is encryption
                let compression = stages[..stages.len()-1].join("+");
                (compression, Some(last_stage.to_string()))
            } else {
                (compressor.to_string(), None)
            }
        } else {
            (compressor.to_string(), None)
        };
    
    eprintln!("Compression chain: '{}'", compression_chain);
    eprintln!("Encryption method: {:?}", encryption_method);
    
    // First: decompress using compression chain
    let mut decompressed = if compression_chain.is_empty() {
        data.to_vec()
    } else {
        decompress_compression_chain(&compression_chain, data, expected_size)?
    };
    
    // Second: decrypt if encryption method exists
    if let Some(enc_method) = encryption_method {
        if let Some(pwd) = password {
            eprintln!("Decrypting with method: {}", enc_method);
            decompressed = decrypt_data(&enc_method, &decompressed, pwd)?;
        } else {
            return Err(anyhow!("Encrypted data requires password"));
        }
    }
    
    Ok(decompressed)
}

fn is_cipher_method(method: &str) -> bool {
    let cipher_names = ["blowfish", "aes", "twofish", "serpent"];
    cipher_names.iter().any(|&name| method.starts_with(name))
}
```

### Step 2: Create decrypt_data() function

Add this to `src/formats/freearc.rs`:

```rust
use crate::codecs::crypto::{EncryptionInfo, CascadedDecryptor};

fn decrypt_data(encryption_method: &str, ciphertext: &[u8], password: &str) -> Result<Vec<u8>> {
    eprintln!("\n=== DECRYPTION ===");
    eprintln!("Method: {}", encryption_method);
    eprintln!("Ciphertext size: {} bytes", ciphertext.len());
    
    // Parse encryption method string
    let enc_info = EncryptionInfo::from_method_string(encryption_method)
        .map_err(|e| anyhow!("Failed to parse encryption method: {}", e))?;
    
    eprintln!("Parsed encryption info:");
    eprintln!("  Algorithm: {:?}", enc_info.algorithms);
    eprintln!("  Mode: {}", enc_info.mode);
    eprintln!("  Key size: {} bytes", enc_info.key_size);
    eprintln!("  Iterations: {}", enc_info.iterations);
    eprintln!("  Salt: {:?}", enc_info.salt.as_ref().map(|s| format!("{} bytes", s.len())));
    eprintln!("  IV: {:?}", enc_info.iv.as_ref().map(|s| format!("{} bytes", s.len())));
    
    // Create cascaded decryptor and decrypt
    let decryptor = CascadedDecryptor::new(&enc_info, password)
        .map_err(|e| anyhow!("Failed to initialize decryptor: {}", e))?;
    
    let plaintext = decryptor.decrypt(ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;
    
    eprintln!("Decryption successful!");
    eprintln!("Plaintext size: {} bytes\n", plaintext.len());
    
    Ok(plaintext)
}
```

### Step 3: Update decompress_compression_chain()

Modify to skip cipher methods in the chain:

```rust
fn decompress_compression_chain(chain: &str, data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    if chain.is_empty() {
        return Ok(data.to_vec());
    }
    
    let stages: Vec<&str> = chain.split('+').collect();
    eprintln!("Processing cascaded compression with {} stages", stages.len());
    
    let mut compressed_data = data.to_vec();
    
    // Process all stages
    for stage in &stages {
        // Skip cipher methods - they're handled separately
        if is_cipher_method(stage) {
            eprintln!("Skipping cipher stage in compression chain: {}", stage);
            continue;
        }
        
        // Handle each preprocessing/compression method
        if stage.starts_with("dict:") {
            eprintln!("Skipping DICT preprocessing stage: {}", stage);
            // DICT is preprocessing on input, not compression
            // Would be applied BEFORE compression, not after
        } else if stage.starts_with("lzp:") {
            eprintln!("Skipping LZP preprocessing stage: {}", stage);
            // LZP is preprocessing on input
        } else if stage.starts_with("ppmd:") {
            eprintln!("Processing PPMd decompression stage: {}", stage);
            // PPMd IS actual compression - decompress it
            // Parse ppmd:16:384mb format
            let ppmd_parts: Vec<&str> = stage.split(':').collect();
            let order = ppmd_parts.get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(6);
            
            eprintln!("PPMd order: {}", order);
            // Decompress with PPMd
            compressed_data = decompress_ppmd(&compressed_data, order, expected_size)?;
        } else if is_single_compression_method(stage) {
            // Handle single compression (lzma, lz4, etc.)
            compressed_data = decompress_single_method(stage, &compressed_data, expected_size)?;
        } else {
            eprintln!("Unknown preprocessing stage: {}", stage);
        }
    }
    
    Ok(compressed_data)
}

fn is_single_compression_method(method: &str) -> bool {
    let methods = ["lzma", "lz4", "zstd", "deflate", "bzip2"];
    methods.iter().any(|&m| method.starts_with(m))
}
```

### Step 4: Ensure crypto module is exported

In `src/codecs/mod.rs`:

```rust
pub mod crypto;  // Add this line if not present

pub use crypto::{EncryptionInfo, CascadedDecryptor};
```

And in `src/codecs/crypto.rs`, ensure the Trait implementations for decryption:

```rust
/// Trait for pluggable cipher implementations
trait Cipher: Send + Sync {
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
}

// Implement Cipher trait for BlowfishCipher
impl Cipher for BlowfishCipher {
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        BlowfishCipher::decrypt(self, ciphertext)
    }
    
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        BlowfishCipher::encrypt(self, plaintext)
    }
}

// Similar for AesCipher, TwofishCipher, SerpentCipher...
```

## Testing the Integration

### Test 1: Verify Encryption Recognition

```bash
cargo run -- extract test.arc -o output --password "Fdhzfc1!" 2>&1 | grep -i "encryption"
```

Expected output:
```
Encryption method: Some("blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:c3a25")
```

### Test 2: Verify Decryption Execution

```bash
cargo run -- extract test.arc -o output --password "Fdhzfc1!" 2>&1 | grep -A 5 "DECRYPTION"
```

Expected output:
```
=== DECRYPTION ===
Method: blowfish-448/ctr:...
Ciphertext size: 3152 bytes
...
Decryption successful!
Plaintext size: 11044 bytes
```

### Test 3: Verify Full Extraction

```bash
cargo run -- extract test.arc -o output --password "Fdhzfc1!"
```

Should show:
```
✓ File extracted: CRYPTO_IMPLEMENTATION.md (11044 bytes)
```

## Debugging Checklist

- [ ] Verify `EncryptionInfo::from_method_string()` correctly parses all parameters
- [ ] Verify `PasswordDeriver::derive_key()` produces consistent keys with FreeARC
- [ ] Verify `CascadedDecryptor::verify_password()` works (compare against FreeARC)
- [ ] Check that decrypted data has expected size (11044 bytes for CRYPTO_IMPLEMENTATION.md)
- [ ] Verify decrypted markdown starts with `#` (valid markdown file)

## Common Issues & Solutions

### Issue: "Decryption failed" after integration

**Cause**: Key derivation mismatch with FreeARC

**Solution**: 
1. Verify PBKDF2-HMAC-SHA512 implementation
2. Check iteration count (should be 1000 from the parsed method string)
3. Verify salt is correctly hex-decoded
4. Compare derived key with known test vectors

### Issue: Decrypted data is garbage

**Cause**: Wrong IV or key

**Solution**:
1. Print key and IV before/after decryption
2. Verify IV is exactly 8 bytes for Blowfish
3. Check if IV is provided in method string vs derived
4. Test with known test vectors first

### Issue: Verification code doesn't match

**Cause**: Password is incorrect or verification function is wrong

**Solution**:
1. Double-check password string
2. Verify verification code comparison logic
3. Check if FreeARC uses different hash (SHA256 vs SHA512)
4. Try disabling verification temporarily to test rest of flow

## Performance Considerations

- Blowfish-CTR mode is a stream cipher (fast)
- 1000 PBKDF2 iterations is standard but can be slow for large files
- Consider caching derived keys if extracting multiple files
- CTR mode allows parallelization by advancing counter

## Next Steps

1. Apply the integration code above
2. Run test with your encrypted archive
3. If it fails, enable debug output and compare with FreeARC behavior
4. Once Blowfish works, add similar integration for PPMd decompression
5. Then test the full cascaded pipeline: DICT → LZP → PPMd → Blowfish → plaintext
