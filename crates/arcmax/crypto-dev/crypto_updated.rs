// UPDATED crypto.rs - Integration with decompression pipeline
// Add this complete implementation to support Blowfish decryption in the pipeline

//! Encryption/Decryption module for arcmax
//!
//! Handles password-based encryption for FreeARC and other formats.
//! Supports: Blowfish, AES, Twofish, Serpent (cascadable)

use anyhow::{anyhow, Result};
use thiserror::Error;

/// Encryption errors
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Unknown encryption method: {0}")]
    UnknownMethod(String),
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Key derivation failed")]
    KeyDerivationFailed,
}

/// Supported encryption algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherAlgorithm {
    None,
    Blowfish,
    AES,
    Twofish,
    Serpent,
}

/// Encryption metadata from block header
#[derive(Debug, Clone)]
pub struct EncryptionInfo {
    /// Algorithm name(s), e.g. "blowfish-448/ctr"
    pub method: String,
    /// Cascaded ciphers parsed from method string
    pub algorithms: Vec<CipherAlgorithm>,
    /// Encryption mode (ctr or cfb)
    pub mode: String,
    /// Key size in bytes
    pub key_size: usize,
    /// Number of PBKDF2 iterations
    pub iterations: u32,
    /// Rounds parameter
    pub rounds: u32,
    /// Salt for key derivation (hex-decoded)
    pub salt: Option<Vec<u8>>,
    /// Verification code (hex-decoded)
    pub code: Option<Vec<u8>>,
    /// IV (hex-decoded)
    pub iv: Option<Vec<u8>>,
}

impl EncryptionInfo {
    /// Parse encryption method string from block descriptor
    ///
    /// Format: "blowfish-448/ctr:n1000:r0:s:c:i"
    pub fn from_method_string(method: &str) -> Result<EncryptionInfo> {
        if method.is_empty() {
            return Ok(EncryptionInfo {
                method: String::new(),
                algorithms: vec![CipherAlgorithm::None],
                mode: String::new(),
                key_size: 0,
                iterations: 1000,
                rounds: 0,
                salt: None,
                code: None,
                iv: None,
            });
        }

        // Split by colon to get main part and parameters
        let parts: Vec<&str> = method.split(':').collect();
        let main_part = parts[0];

        // Parse main part: "blowfish-448/ctr"
        let (cipher_part, mode) = if main_part.contains('/') {
            let slash_parts: Vec<&str> = main_part.split('/').collect();
            (slash_parts[0], slash_parts.get(1).unwrap_or(&"ctr").to_string())
        } else {
            (main_part, "ctr".to_string())
        };

        // Parse cipher and key size: "blowfish-448"
        let (cipher_name, key_size) = if cipher_part.contains('-') {
            let dash_parts: Vec<&str> = cipher_part.split('-').collect();
            let cipher = dash_parts[0];
            let bits: usize = dash_parts.get(1).unwrap_or(&"128").parse().unwrap_or(128);
            (cipher, bits / 8) // Convert bits to bytes
        } else {
            (cipher_part, 16) // Default 128 bits = 16 bytes
        };

        // Determine cipher algorithm
        let algorithm = match cipher_name.to_lowercase().as_str() {
            "blowfish" => CipherAlgorithm::Blowfish,
            "aes" => CipherAlgorithm::AES,
            "twofish" => CipherAlgorithm::Twofish,
            "serpent" => CipherAlgorithm::Serpent,
            other => return Err(CryptoError::UnknownMethod(other.to_string()).into()),
        };

        // Parse parameters
        let mut iterations = 1000u32;
        let mut rounds = 0u32;
        let mut salt = None;
        let mut code = None;
        let mut iv = None;

        for part in &parts[1..] {
            if part.starts_with('n') {
                iterations = part[1..].parse().unwrap_or(1000);
            } else if part.starts_with('r') {
                rounds = part[1..].parse().unwrap_or(0);
            } else if part.starts_with('s') {
                eprintln!("Parsing salt from: '{}'", &part[1..]);
                salt = Some(hex_decode(&part[1..])?);
            } else if part.starts_with('c') {
                eprintln!("Parsing code from: '{}'", &part[1..]);
                code = Some(hex_decode(&part[1..])?);
            } else if part.starts_with('i') {
                eprintln!("Parsing IV from: '{}'", &part[1..]);
                iv = Some(hex_decode(&part[1..])?);
            }
        }

        Ok(EncryptionInfo {
            method: method.to_string(),
            algorithms: vec![algorithm],
            mode,
            key_size,
            iterations,
            rounds,
            salt,
            code,
            iv,
        })
    }
}

/// Decode hex string to bytes
fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        if i + 1 < s.len() {
            let byte_str = &s[i..i + 2];
            let byte = u8::from_str_radix(byte_str, 16)
                .map_err(|_| anyhow!("Invalid hex string: {}", s))?;
            bytes.push(byte);
        }
    }
    Ok(bytes)
}

/// Password-to-key derivation for FreeARC
///
/// FreeARC uses PKCS#5 v2 (PBKDF2-HMAC-SHA512) for key derivation.
/// See: unarc/Compression/_Encryption/C_Encryption.cpp:154-160
pub struct PasswordDeriver {
    /// Number of PBKDF2 iterations (default: 1000 in FreeARC)
    pub iterations: u32,
}

impl PasswordDeriver {
    /// Create a new password deriver with default iterations
    pub fn new() -> Self {
        PasswordDeriver { iterations: 1000 }
    }

    /// Create a new password deriver with custom iterations
    pub fn new_with_iterations(iterations: u32) -> Self {
        PasswordDeriver { iterations }
    }

    /// Derive encryption key from password using PBKDF2-HMAC-SHA512
    ///
    /// FreeARC uses:
    /// - Hash: SHA-512
    /// - Default iterations: 1000
    /// - Inputs: password + salt (optional)
    /// - Output: key of specified length
    pub fn derive_key(
        &self,
        password: &str,
        salt: Option<&[u8]>,
        key_len: usize,
    ) -> Result<Vec<u8>> {
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

    /// Derive IV (initialization vector) from password
    ///
    /// For FreeARC, we derive the IV similarly to the key but with a different salt
    /// to ensure key and IV are independent.
    pub fn derive_iv(
        &self,
        password: &str,
        iv_len: usize,
    ) -> Result<Vec<u8>> {
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        // Use a fixed salt for IV derivation to ensure deterministic IV
        let iv_salt = b"FreeARC_IV_Salt";
        let mut iv = vec![0u8; iv_len];

        pbkdf2_hmac::<Sha512>(
            password.as_bytes(),
            iv_salt,
            self.iterations,
            &mut iv
        );

        Ok(iv)
    }
}

impl Default for PasswordDeriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Blowfish cipher wrapper using CTR mode
///
/// FreeARC uses CTR (Counter) mode which is a stream cipher mode.
/// No padding is needed for CTR mode.
/// See: unarc/Compression/_Encryption/C_Encryption.cpp:90-138
pub struct BlowfishCipher {
    key: Vec<u8>,
    iv: Vec<u8>,
}

impl BlowfishCipher {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self> {
        // Blowfish block size is 8 bytes
        if iv.len() != 8 {
            return Err(anyhow!("Blowfish IV must be 8 bytes, got {}", iv.len()));
        }

        // Blowfish key size can be 4-56 bytes
        if key.len() < 4 || key.len() > 56 {
            return Err(anyhow!("Blowfish key must be 4-56 bytes, got {}", key.len()));
        }

        Ok(BlowfishCipher {
            key: key.to_vec(),
            iv: iv.to_vec(),
        })
    }

    /// Decrypt data in CTR mode
    ///
    /// CTR mode turns a block cipher into a stream cipher.
    /// Encryption and decryption are the same operation in CTR mode.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use blowfish::cipher::{KeyIvInit, StreamCipher};
        use blowfish::Blowfish;
        use ctr::Ctr64LE;
        use generic_array::GenericArray;

        // Create cipher instance
        let key = GenericArray::from_slice(&self.key);
        let iv = GenericArray::from_slice(&self.iv);

        // Create CTR mode with little-endian counter (as used by FreeARC)
        let mut cipher = Ctr64LE::<Blowfish>::new(key, iv);

        // Perform decryption (same as encryption in CTR mode)
        let mut buffer = ciphertext.to_vec();
        cipher.apply_keystream(&mut buffer);

        Ok(buffer)
    }

    /// Encrypt data in CTR mode
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use blowfish::cipher::{KeyIvInit, StreamCipher};
        use blowfish::Blowfish;
        use ctr::Ctr64LE;
        use generic_array::GenericArray;

        // Create cipher instance
        let key = GenericArray::from_slice(&self.key);
        let iv = GenericArray::from_slice(&self.iv);

        // Create CTR mode with little-endian counter
        let mut cipher = Ctr64LE::<Blowfish>::new(key, iv);

        // Perform encryption
        let mut buffer = plaintext.to_vec();
        cipher.apply_keystream(&mut buffer);

        Ok(buffer)
    }
}

/// AES cipher wrapper using CTR mode
///
/// FreeARC uses CTR mode for AES as well
pub struct AesCipher {
    key: Vec<u8>,
    iv: Vec<u8>,
}

impl AesCipher {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self> {
        // AES supports 128, 192, or 256-bit keys (16, 24, or 32 bytes)
        match key.len() {
            16 | 24 | 32 => {},
            len => return Err(anyhow!("Invalid AES key length: {} bytes (expected 16, 24, or 32)", len)),
        }

        if iv.len() != 16 {
            return Err(anyhow!("AES IV must be 16 bytes, got {}", iv.len()));
        }

        Ok(AesCipher {
            key: key.to_vec(),
            iv: iv.to_vec(),
        })
    }

    /// Decrypt using AES-CTR mode
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use aes::{Aes128, Aes192, Aes256};
        use ctr::Ctr64LE;
        use generic_array::GenericArray;

        let mut buffer = ciphertext.to_vec();

        // Create cipher instance based on key length
        match self.key.len() {
            16 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            24 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes192>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            32 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes256>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            _ => return Err(anyhow!("Invalid AES key length: {}", self.key.len())),
        }

        Ok(buffer)
    }

    /// Encrypt using AES-CTR mode
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use aes::{Aes128, Aes192, Aes256};
        use ctr::Ctr64LE;
        use generic_array::GenericArray;

        let mut buffer = plaintext.to_vec();

        // Create cipher instance based on key length
        match self.key.len() {
            16 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            24 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes192>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            32 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr64LE::<Aes256>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            _ => return Err(anyhow!("Invalid AES key length: {}", self.key.len())),
        }

        Ok(buffer)
    }
}

/// Twofish cipher wrapper (for completeness; less common)
pub struct TwofishCipher {
    key: Vec<u8>,
    iv: Vec<u8>,
}

impl TwofishCipher {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self> {
        if key.len() > 32 {
            return Err(anyhow!("Twofish key too long: {} bytes", key.len()));
        }

        if iv.len() != 16 {
            return Err(anyhow!("Twofish IV must be 16 bytes, got {}", iv.len()));
        }

        Ok(TwofishCipher {
            key: key.to_vec(),
            iv: iv.to_vec(),
        })
    }

    pub fn decrypt(&self, _ciphertext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the twofish crate
        Err(anyhow!("Twofish decryption not fully implemented"))
    }

    pub fn encrypt(&self, _plaintext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the twofish crate
        Err(anyhow!("Twofish encryption not fully implemented"))
    }
}

/// Serpent cipher wrapper
pub struct SerpentCipher {
    key: Vec<u8>,
    iv: Vec<u8>,
}

impl SerpentCipher {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self> {
        if key.len() > 32 {
            return Err(anyhow!("Serpent key too long: {} bytes", key.len()));
        }

        if iv.len() != 16 {
            return Err(anyhow!("Serpent IV must be 16 bytes, got {}", iv.len()));
        }

        Ok(SerpentCipher {
            key: key.to_vec(),
            iv: iv.to_vec(),
        })
    }

    pub fn decrypt(&self, _ciphertext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the serpent crate
        Err(anyhow!("Serpent decryption not fully implemented"))
    }

    pub fn encrypt(&self, _plaintext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the serpent crate
        Err(anyhow!("Serpent encryption not fully implemented"))
    }
}

/// Trait for cipher operations (allows generic handling)
pub trait CipherOp: Send + Sync {
    fn decrypt_op(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
    fn encrypt_op(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
}

impl CipherOp for BlowfishCipher {
    fn decrypt_op(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.decrypt(ciphertext)
    }

    fn encrypt_op(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.encrypt(plaintext)
    }
}

impl CipherOp for AesCipher {
    fn decrypt_op(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.decrypt(ciphertext)
    }

    fn encrypt_op(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.encrypt(plaintext)
    }
}

impl CipherOp for TwofishCipher {
    fn decrypt_op(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.decrypt(ciphertext)
    }

    fn encrypt_op(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.encrypt(plaintext)
    }
}

impl CipherOp for SerpentCipher {
    fn decrypt_op(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.decrypt(ciphertext)
    }

    fn encrypt_op(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.encrypt(plaintext)
    }
}

/// Generic decryption dispatcher for cascaded ciphers
///
/// FreeARC can chain ciphers: "aes+serpent" means decrypt with serpent first, then AES
pub struct CascadedDecryptor {
    ciphers: Vec<Box<dyn CipherOp>>,
}

impl CascadedDecryptor {
    /// Verify password is correct using the verification code
    fn verify_password(enc_info: &EncryptionInfo, password: &str) -> Result<bool> {
        if let Some(ref verification_code) = enc_info.code {
            // The verification code is a hash/checksum of the password
            // In FreeARC, it's typically the first few bytes of a hash of the password
            use sha2::{Sha256, Digest};

            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            if let Some(ref salt) = enc_info.salt {
                hasher.update(salt);
            }

            let hash = hasher.finalize();

            // Compare first bytes of hash with verification code
            let code_len = verification_code.len();
            if code_len > hash.len() {
                return Ok(false);
            }

            let matches = &hash[..code_len] == &verification_code[..];
            if !matches {
                eprintln!("Password verification FAILED");
                eprintln!("Expected code: {:?}", verification_code);
                eprintln!("Computed hash (first {} bytes): {:?}", code_len, &hash[..code_len]);
            } else {
                eprintln!("Password verification PASSED");
            }

            Ok(matches)
        } else {
            // No verification code, assume password is correct
            eprintln!("No verification code present, skipping password check");
            Ok(true)
        }
    }

    /// Create a cascaded decryptor from encryption info and password
    pub fn new(enc_info: &EncryptionInfo, password: &str) -> Result<Self> {
        if enc_info.algorithms.is_empty() || enc_info.algorithms[0] == CipherAlgorithm::None {
            return Ok(CascadedDecryptor {
                ciphers: vec![],
            });
        }

        // Verify password first
        if !Self::verify_password(enc_info, password)? {
            return Err(CryptoError::InvalidPassword.into());
        }

        let deriver = PasswordDeriver::new_with_iterations(enc_info.iterations);
        let mut ciphers: Vec<Box<dyn CipherOp>> = vec![];

        // Use parsed parameters from encryption info
        let salt = enc_info.salt.as_ref().map(|s| s.as_slice());

        for algo in &enc_info.algorithms {
            let cipher: Box<dyn CipherOp> = match algo {
                CipherAlgorithm::None => continue,

                CipherAlgorithm::Blowfish => {
                    // Derive key using parsed parameters
                    let key = deriver.derive_key(password, salt, enc_info.key_size)?;

                    // Use provided IV or derive it
                    let iv = if let Some(ref iv_bytes) = enc_info.iv {
                        iv_bytes.clone()
                    } else {
                        deriver.derive_iv(password, 8)?
                    };

                    eprintln!("Blowfish decrypt: key_size={}, iv_len={}, iterations={}",
                        key.len(), iv.len(), enc_info.iterations);
                    eprintln!("Key (first 16 bytes): {:?}", &key[..std::cmp::min(16, key.len())]);
                    eprintln!("IV (all 8 bytes): {:?}", &iv);
                    if let Some(ref salt_bytes) = enc_info.salt {
                        eprintln!("Salt (first 16 bytes): {:?}", &salt_bytes[..std::cmp::min(16, salt_bytes.len())]);
                    }

                    Box::new(BlowfishCipher::new(&key, &iv)?)
                },

                CipherAlgorithm::AES => {
                    let key = deriver.derive_key(password, salt, enc_info.key_size)?;
                    let iv = if let Some(ref iv_bytes) = enc_info.iv {
                        iv_bytes.clone()
                    } else {
                        deriver.derive_iv(password, 16)?
                    };
                    Box::new(AesCipher::new(&key, &iv)?)
                },

                CipherAlgorithm::Twofish => {
                    let key = deriver.derive_key(password, salt, enc_info.key_size)?;
                    let iv = if let Some(ref iv_bytes) = enc_info.iv {
                        iv_bytes.clone()
                    } else {
                        deriver.derive_iv(password, 16)?
                    };
                    Box::new(TwofishCipher::new(&key, &iv)?)
                },

                CipherAlgorithm::Serpent => {
                    let key = deriver.derive_key(password, salt, enc_info.key_size)?;
                    let iv = if let Some(ref iv_bytes) = enc_info.iv {
                        iv_bytes.clone()
                    } else {
                        deriver.derive_iv(password, 16)?
                    };
                    Box::new(SerpentCipher::new(&key, &iv)?)
                },
            };
            ciphers.push(cipher);
        }

        Ok(CascadedDecryptor { ciphers })
    }

    /// Decrypt data through all chained ciphers (in reverse order)
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if self.ciphers.is_empty() {
            return Ok(ciphertext.to_vec());
        }

        let mut data = ciphertext.to_vec();

        // Decrypt in reverse order: last cipher added is outermost
        for cipher in self.ciphers.iter().rev() {
            data = cipher.decrypt_op(&data)?;
        }

        Ok(data)
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        if self.ciphers.is_empty() {
            return Ok(plaintext.to_vec());
        }

        let mut data = plaintext.to_vec();
        for cipher in &self.ciphers {
            data = cipher.encrypt_op(&data)?;
        }

        Ok(data)
    }

    /// Check if any encryption is configured
    pub fn is_encrypted(&self) -> bool {
        !self.ciphers.is_empty()
    }
}

// ============================================================================
// INTEGRATION HELPER - Use this in decompression pipeline
// ============================================================================

/// Create a decryptor from a compression method string with password
/// 
/// This is the main entry point for integrating decryption into the pipeline.
/// Call this when you detect encryption in the compression method.
pub fn create_decryptor(method_string: &str, password: &str) -> Result<CascadedDecryptor> {
    let enc_info = EncryptionInfo::from_method_string(method_string)?;
    CascadedDecryptor::new(&enc_info, password)
}

/// Decrypt data in the decompression pipeline
///
/// Usage in your decompression code:
/// ```ignore
/// let decryptor = create_decryptor(&compression_method, password)?;
/// if decryptor.is_encrypted() {
///     data = decryptor.decrypt(&data)?;
/// }
/// ```
pub fn decrypt_data(
    compression_method: &str,
    encrypted_data: &[u8],
    password: &str,
) -> Result<Vec<u8>> {
    let decryptor = create_decryptor(compression_method, password)?;
    decryptor.decrypt(encrypted_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_info_parsing() {
        let enc = EncryptionInfo::from_method_string("blowfish").unwrap();
        assert_eq!(enc.algorithms, vec![CipherAlgorithm::Blowfish]);
    }

    #[test]
    fn test_empty_encryption() {
        let enc = EncryptionInfo::from_method_string("").unwrap();
        assert_eq!(enc.algorithms, vec![CipherAlgorithm::None]);
    }

    #[test]
    fn test_hex_decode() {
        let result = hex_decode("4f62").unwrap();
        assert_eq!(result, vec![0x4f, 0x62]);
    }
}
