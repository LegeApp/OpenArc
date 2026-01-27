# Diagnostic and Debugging Guide for Crypto Integration

## Quick Diagnostic Test

Add this temporary test function to `src/formats/freearc.rs` to verify crypto module works:

```rust
#[cfg(test)]
mod crypto_tests {
    use super::*;
    use crate::codecs::crypto::{EncryptionInfo, PasswordDeriver};

    #[test]
    fn test_encryption_info_parsing() {
        let method = "blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:c3a25";
        
        match EncryptionInfo::from_method_string(method) {
            Ok(enc_info) => {
                eprintln!("✓ Parsed successfully!");
                eprintln!("  Algorithm: {:?}", enc_info.algorithms);
                eprintln!("  Key size: {} bytes", enc_info.key_size);
                eprintln!("  Iterations: {}", enc_info.iterations);
                eprintln!("  Salt: {:?}", enc_info.salt.as_ref().map(|s| format!("{} bytes", s.len())));
                eprintln!("  IV: {:?}", enc_info.iv.as_ref().map(|s| format!("{} bytes", s.len())));
                eprintln!("  Code: {:?}", enc_info.code.as_ref().map(|s| format!("{} bytes", s.len())));
                assert!(enc_info.key_size == 56, "Key size should be 56 bytes (448 bits / 8)");
                assert!(enc_info.iterations == 1000, "Iterations should be 1000");
                assert!(enc_info.salt.is_some(), "Salt should be present");
                assert!(enc_info.iv.is_some(), "IV should be present");
            }
            Err(e) => panic!("Failed to parse encryption info: {}", e),
        }
    }

    #[test]
    fn test_password_derivation() {
        let password = "Fdhzfc1!";
        let deriver = PasswordDeriver::new();
        
        // Derive a key
        let salt = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        match deriver.derive_key(password, Some(&salt), 56) {
            Ok(key) => {
                eprintln!("✓ Key derived: {} bytes", key.len());
                eprintln!("  First 16 bytes: {:?}", &key[..16]);
                assert_eq!(key.len(), 56, "Key should be 56 bytes");
                assert!(!key.iter().all(|&b| b == 0), "Key should not be all zeros");
            }
            Err(e) => panic!("Failed to derive key: {}", e),
        }
    }

    #[test]
    fn test_is_cipher_method() {
        assert!(is_cipher_method("blowfish-448/ctr"));
        assert!(is_cipher_method("AES-256/ctr"));
        assert!(is_cipher_method("twofish"));
        assert!(is_cipher_method("serpent-256"));
        assert!(!is_cipher_method("lzma:1mb"));
        assert!(!is_cipher_method("ppmd:16"));
    }

    #[test]
    fn test_split_encryption_from_compression() {
        let method = "dict:12kb+lzp:12kb+ppmd:16:384mb+blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:s509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53:c3a25";
        
        let (compression, encryption) = split_encryption_from_compression(method);
        
        eprintln!("Compression: '{}'", compression);
        eprintln!("Encryption: {:?}", encryption);
        
        assert!(compression.contains("dict:"), "Compression chain should contain dict");
        assert!(compression.contains("ppmd:"), "Compression chain should contain ppmd");
        assert!(!compression.contains("blowfish"), "Compression chain should not contain blowfish");
        assert!(encryption.is_some(), "Encryption should be detected");
        assert!(encryption.as_ref().unwrap().contains("blowfish"), "Encryption should be blowfish");
    }
}
```

Run tests with:
```bash
cargo test crypto_tests -- --nocapture
```

## Debugging Output Statements

Add these debug outputs to track execution flow. Add to the `decrypt_data()` function:

```rust
fn decrypt_data(
    encryption_method: &str,
    ciphertext: &[u8],
    password: &str,
) -> Result<Vec<u8>> {
    eprintln!("\n╔════════════════════════════════════════════════════════════════╗");
    eprintln!("║                      DECRYPTION PHASE                          ║");
    eprintln!("╚════════════════════════════════════════════════════════════════╝\n");
    
    eprintln!("[1] Input Parameters:");
    eprintln!("    Method string: {}", encryption_method);
    eprintln!("    Ciphertext size: {} bytes", ciphertext.len());
    eprintln!("    Password length: {} chars", password.len());
    eprintln!("    First 16 bytes of ciphertext: {:?}", 
        &ciphertext[..std::cmp::min(16, ciphertext.len())]);
    
    // Parse encryption method string
    eprintln!("\n[2] Parsing encryption method...");
    let enc_info = match EncryptionInfo::from_method_string(encryption_method) {
        Ok(info) => {
            eprintln!("    ✓ Successfully parsed!");
            info
        }
        Err(e) => {
            eprintln!("    ✗ Parse failed: {}", e);
            return Err(anyhow!("Failed to parse encryption method: {}", e));
        }
    };
    
    eprintln!("\n[3] Encryption Parameters:");
    eprintln!("    Algorithm: {:?}", enc_info.algorithms);
    eprintln!("    Mode: {}", enc_info.mode);
    eprintln!("    Key size: {} bytes", enc_info.key_size);
    eprintln!("    Iterations: {}", enc_info.iterations);
    eprintln!("    Rounds: {}", enc_info.rounds);
    
    if let Some(ref salt) = enc_info.salt {
        eprintln!("    Salt: {} bytes", salt.len());
        if salt.len() <= 64 {
            eprintln!("      (hex) {}", hex_encode(salt));
        }
    }
    
    if let Some(ref iv) = enc_info.iv {
        eprintln!("    IV: {} bytes", iv.len());
        eprintln!("      (hex) {}", hex_encode(iv));
    }
    
    if let Some(ref code) = enc_info.code {
        eprintln!("    Verification: {} bytes", code.len());
        eprintln!("      (hex) {}", hex_encode(code));
    }
    
    eprintln!("\n[4] Initializing decryptor...");
    let decryptor = match CascadedDecryptor::new(&enc_info, password) {
        Ok(d) => {
            eprintln!("    ✓ Decryptor ready");
            d
        }
        Err(e) => {
            eprintln!("    ✗ Failed: {}", e);
            return Err(anyhow!("Decryptor initialization failed: {}", e));
        }
    };
    
    eprintln!("\n[5] Decrypting {} bytes...", ciphertext.len());
    let plaintext = match decryptor.decrypt(ciphertext) {
        Ok(p) => {
            eprintln!("    ✓ Decryption successful!");
            eprintln!("    Output size: {} bytes", p.len());
            eprintln!("    First 20 bytes: {:?}", 
                &p[..std::cmp::min(20, p.len())]);
            p
        }
        Err(e) => {
            eprintln!("    ✗ Decryption failed: {}", e);
            return Err(anyhow!("Decryption failed: {}", e));
        }
    };
    
    eprintln!("\n╔════════════════════════════════════════════════════════════════╗");
    eprintln!("║              ✓ DECRYPTION COMPLETED SUCCESSFULLY                ║");
    eprintln!("╚════════════════════════════════════════════════════════════════╝\n");
    
    Ok(plaintext)
}
```

## Common Issues Debugging Checklist

### Issue: "Failed to parse encryption method"

**Debug steps:**

1. Add this temporary code:
```rust
let method = "blowfish-448/ctr:n1000:r0:i4062f442dc757ea7:...";
eprintln!("Method string length: {}", method.len());
eprintln!("Method contains colons: {}", method.matches(':').count());
eprintln!("Method contains slashes: {}", method.matches('/').count());

let parts: Vec<&str> = method.split(':').collect();
eprintln!("Parts after split by ':':");
for (i, part) in parts.iter().enumerate() {
    eprintln!("  [{}] '{}'", i, part);
}
```

2. Verify `from_method_string()` in crypto.rs:
   - Check slash parsing: `blowfish-448/ctr`
   - Check dash parsing: `blowfish-448`
   - Check parameter parsing: `n1000`, `r0`, `s...`, `c...`, `i...`

### Issue: "Invalid password" during verification

**Debug steps:**

1. Check that `CascadedDecryptor::verify_password()` is working:
```rust
eprintln!("Verification code present: {}", enc_info.code.is_some());
if let Some(ref code) = enc_info.code {
    eprintln!("Code length: {}", code.len());
    eprintln!("Code (hex): {}", hex_encode(code));
    
    // Test hash derivation
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    if let Some(ref salt) = enc_info.salt {
        hasher.update(salt);
    }
    let hash = hasher.finalize();
    eprintln!("Computed hash (first {} bytes): {}", 
        code.len(), 
        hex_encode(&hash[..code.len()]));
}
```

2. Try disabling verification temporarily:
```rust
// In CascadedDecryptor::new()
// Comment out this line for testing:
// if !Self::verify_password(enc_info, password)? {
//     return Err(CryptoError::InvalidPassword.into());
// }
```

### Issue: "Blowfish IV must be 8 bytes, got X"

**Debug steps:**

1. Check IV parsing:
```rust
eprintln!("IV from enc_info: {:?}", enc_info.iv.as_ref().map(|v| v.len()));
eprintln!("Expected: 8 bytes for Blowfish");

// If IV is provided in method string
if let Some(ref iv) = enc_info.iv {
    eprintln!("IV hex: {}", hex_encode(iv));
} else {
    eprintln!("No IV in method string, will derive from password");
}
```

### Issue: "Key derivation failed"

**Debug steps:**

1. Verify PBKDF2 is available:
```bash
grep pbkdf2 Cargo.toml
grep sha2 Cargo.toml
```

2. Test key derivation standalone:
```rust
#[test]
fn test_key_derivation_detailed() {
    let password = "Fdhzfc1!";
    let salt = hex::decode("509213c0e615095e2c447897dffa856c27cb7b30fc9a869e83c296a5ade0dcf227dd3a98ec7fd0c2b5e228bc8da429bc80d99ead0022cf53").unwrap();
    let iterations = 1000;
    let key_len = 56;
    
    eprintln!("Testing PBKDF2 with:");
    eprintln!("  Password: {}", password);
    eprintln!("  Salt length: {}", salt.len());
    eprintln!("  Iterations: {}", iterations);
    eprintln!("  Key length: {}", key_len);
    
    let deriver = PasswordDeriver::new_with_iterations(iterations);
    match deriver.derive_key(password, Some(&salt), key_len) {
        Ok(key) => {
            eprintln!("✓ Success!");
            eprintln!("  Key: {}", hex_encode(&key));
        }
        Err(e) => panic!("✗ Failed: {}", e),
    }
}
```

## Performance Monitoring

Add timing to decrypt_data():

```rust
use std::time::Instant;

let start = Instant::now();

let plaintext = decryptor.decrypt(ciphertext)?;

let elapsed = start.elapsed();
let throughput = ciphertext.len() as f64 / elapsed.as_secs_f64() / 1024.0 / 1024.0;

eprintln!("Decryption timing:");
eprintln!("  Total: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
eprintln!("  Throughput: {:.2} MB/s", throughput);
```

## Memory Analysis

Add memory tracking:

```rust
eprintln!("Memory usage:");
eprintln!("  Input (ciphertext): {} bytes", ciphertext.len());
eprintln!("  Output (plaintext): {} bytes", plaintext.len());
eprintln!("  Expansion ratio: {:.2}%", 
    (plaintext.len() as f64 / ciphertext.len() as f64 - 1.0) * 100.0);
```

## File Verification After Extraction

```bash
# Check file was extracted
ls -lh extracted_encrypted/CRYPTO_IMPLEMENTATION.md

# Verify it's valid markdown
head -20 extracted_encrypted/CRYPTO_IMPLEMENTATION.md

# Check file size
wc -c extracted_encrypted/CRYPTO_IMPLEMENTATION.md
# Should be 11044 bytes

# Search for content
grep -i "blowfish" extracted_encrypted/CRYPTO_IMPLEMENTATION.md
```

## Comparing with Reference Output

If you have access to the decrypted file from another tool, compare:

```bash
# If you have reference file
diff reference_CRYPTO_IMPLEMENTATION.md extracted_encrypted/CRYPTO_IMPLEMENTATION.md

# Or checksums
md5sum reference_CRYPTO_IMPLEMENTATION.md
md5sum extracted_encrypted/CRYPTO_IMPLEMENTATION.md
```

## Full Integration Test Script

Create `test_crypto_integration.sh`:

```bash
#!/bin/bash

echo "═══════════════════════════════════════════════════════════════"
echo "CRYPTO INTEGRATION TEST SUITE"
echo "═══════════════════════════════════════════════════════════════"

echo -e "\n[1] Running crypto unit tests..."
cargo test crypto_tests -- --nocapture --test-threads=1

echo -e "\n[2] Building release binary..."
cargo build --release 2>&1 | grep -i "error\|warning" || echo "✓ Build successful"

echo -e "\n[3] Testing extraction with password..."
./target/release/arcmax extract test.arc -o extracted_test --password "Fdhzfc1!" 2>&1 | tail -20

echo -e "\n[4] Verifying extracted file..."
if [ -f "extracted_test/CRYPTO_IMPLEMENTATION.md" ]; then
    echo "✓ File extracted"
    SIZE=$(wc -c < "extracted_test/CRYPTO_IMPLEMENTATION.md")
    echo "  Size: $SIZE bytes (expected: 11044)"
    echo "  First 50 chars: $(head -c 50 extracted_test/CRYPTO_IMPLEMENTATION.md)"
else
    echo "✗ File not found!"
fi

echo -e "\n═══════════════════════════════════════════════════════════════"
```

Run with:
```bash
chmod +x test_crypto_integration.sh
./test_crypto_integration.sh 2>&1 | tee crypto_test_log.txt
```
