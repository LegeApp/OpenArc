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
    /// Whether the :f flag was present (uses correct hex decoding)
    /// FreeARC had a bug in hex decoding where a-f mapped to 0-5 instead of 10-15
    /// Archives without :f flag use the buggy decoder
    pub fixed: bool,
}

impl EncryptionInfo {
    /// Parse encryption method string from block descriptor
    ///
    /// Format: "blowfish-448/ctr:n1000:r0:s<hex_salt>:c<hex_code>:i<hex_iv>"
    pub fn from_method_string(method: &str, crypto_flags: Option<&str>) -> Result<Self> {
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
                fixed: true, // empty method, use correct decoding
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

        // Parse cipher algorithms (support cascaded encryption like "aes+serpent")
        let algorithms = if cipher_name.contains('+') {
            let mut algos = Vec::new();
            for alg in cipher_name.split('+') {
                let algorithm = match alg.trim().to_lowercase().as_str() {
                    "blowfish" => CipherAlgorithm::Blowfish,
                    "aes" => CipherAlgorithm::AES,
                    "twofish" => CipherAlgorithm::Twofish,
                    "serpent" => CipherAlgorithm::Serpent,
                    "none" => CipherAlgorithm::None,
                    other => return Err(CryptoError::UnknownMethod(other.to_string()).into()),
                };
                algos.push(algorithm);
            }
            algos
        } else {
            let algorithm = match cipher_name.to_lowercase().as_str() {
                "blowfish" => CipherAlgorithm::Blowfish,
                "aes" => CipherAlgorithm::AES,
                "twofish" => CipherAlgorithm::Twofish,
                "serpent" => CipherAlgorithm::Serpent,
                "none" => CipherAlgorithm::None,
                other => return Err(CryptoError::UnknownMethod(other.to_string()).into()),
            };
            vec![algorithm]
        };

        // Parse parameters
        let mut iterations = 1000u32;
        let mut rounds = 0u32;
        let mut salt_hex = None;
        let mut code_hex = None;
        let mut iv_hex = None;
        // The :f flag in FreeARC controls PASSWORD encoding (UTF-8 vs Latin-1),
        // NOT hex encoding. Hex encoding is always correct in FreeARC Haskell code.
        // See: freearc/app/Encryption.hs line 85:
        //   real_password = if params `contains` "f" then unicode2utf8 password else password
        let mut fixed = false; // :f flag means use UTF-8 password encoding

        // Check crypto flags first
        if let Some(flags) = crypto_flags {
            if flags.contains(":c") || flags.contains("c") {
                // User override - not actually needed for hex, but keep for compatibility
                fixed = true;
                eprintln!("Crypto flags contain :c - UTF-8 password encoding enabled");
            } else if flags.contains(":f") || flags.contains("f") {
                fixed = true;
                eprintln!("Crypto flags contain :f - UTF-8 password encoding enabled");
            }
        }

        // First pass: detect the :f flag in the method string
        if !fixed {
            for part in &parts[1..] {
                if *part == "f" {
                    fixed = true;
                    eprintln!("Detected :f flag - UTF-8 password encoding enabled");
                    break;
                }
            }
        }

        if !fixed {
            eprintln!("No :f flag - password used as Latin-1 (raw bytes)");
        }

        // Second pass: parse all parameters
        for part in &parts[1..] {
            if *part == "f" {
                // Already handled
                continue;
            } else if part.starts_with('n') {
                iterations = part[1..].parse().unwrap_or(1000);
            } else if part.starts_with('r') {
                rounds = part[1..].parse().unwrap_or(0);
            } else if part.starts_with('s') {
                eprintln!("Parsing salt from: '{}'", &part[1..]);
                salt_hex = Some(part[1..].to_string());
            } else if part.starts_with('c') && part.len() > 1 {
                // This is the verification code (not to be confused with salt which also starts with 's')
                // The format is 'c' + hex_verification_code
                eprintln!("Parsing verification code from: '{}'", &part[1..]);
                code_hex = Some(part[1..].to_string());
            } else if part.starts_with('i') {
                eprintln!("Parsing IV from: '{}'", &part[1..]);
                iv_hex = Some(part[1..].to_string());
            }
            // Special handling for standalone 'c' (which should force correct hex decoding)
            else if *part == "c" {
                // Force correct hex decoding (for archives created with buggy encoder)
                eprintln!("Detected :c flag - forcing correct hex decoding");
                fixed = true;
            }
        }

        // Decode hex values using the appropriate decoder
        let salt = if let Some(ref s) = salt_hex {
            Some(decode_hex(s, fixed)?)
        } else {
            None
        };
        let code = if let Some(ref c) = code_hex {
            Some(decode_hex(c, fixed)?)
        } else {
            None
        };
        let iv = if let Some(ref i) = iv_hex {
            Some(decode_hex(i, fixed)?)
        } else {
            None
        };

        // If no verification code was found with 'c' prefix, check if the last part might be the verification code
        // In some FreeARC formats, the verification code is just appended without a prefix
        // But in the format we're seeing, it's like "c3a25" where 'c' indicates verification code
        let code = if code.is_none() && !parts.is_empty() {
            let last_part = parts.last().unwrap();
            if last_part.starts_with('c') && last_part.len() > 1 {
                // This is a verification code in the format 'c' + hex_digits
                let hex_part = &last_part[1..]; // Remove the 'c' prefix
                if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                    eprintln!("Parsing verification code from: '{}'", hex_part);
                    Some(decode_hex(hex_part, fixed)?)
                } else {
                    code
                }
            } else {
                code
            }
        } else {
            code
        };

        Ok(EncryptionInfo {
            method: method.to_string(),
            algorithms,
            mode,
            key_size,
            iterations,
            rounds,
            salt,
            code,
            iv,
            fixed,
        })
    }
}

/// Decode hex string to bytes (correct implementation)
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

/// FreeARC's buggy hex decoder
///
/// FreeARC had a bug where hex characters a-f mapped to 0-5 instead of 10-15.
/// Archives created without the :f flag use this buggy encoding.
/// See FreeARC source: Common.h buggy_char2int()
fn buggy_hex_decode(s: &str) -> Result<Vec<u8>> {
    fn buggy_char2int(c: char) -> u8 {
        if c.is_ascii_digit() {
            c as u8 - b'0'
        } else {
            // BUG: FreeARC maps 'a'->0, 'b'->1, 'c'->2, 'd'->3, 'e'->4, 'f'->5
            // instead of the correct 10, 11, 12, 13, 14, 15
            c.to_ascii_lowercase() as u8 - b'a'
        }
    }

    let mut bytes = Vec::with_capacity(s.len() / 2);
    let chars: Vec<char> = s.chars().collect();
    for i in (0..chars.len()).step_by(2) {
        if i + 1 < chars.len() {
            let high = buggy_char2int(chars[i]);
            let low = buggy_char2int(chars[i + 1]);
            bytes.push(high * 16 + low);
        }
    }
    Ok(bytes)
}

/// Choose the appropriate hex decoder based on fixed flag
///
/// NOTE: The :f flag in FreeARC actually controls PASSWORD encoding (UTF-8 vs Latin-1),
/// NOT hex encoding. The Haskell FreeARC code always uses correct hex encoding/decoding
/// via digitToInt/intToDigit. The "buggy" char2int in Common.h is only used by the
/// C++ unarc code which doesn't support encryption anyway.
///
/// Therefore, we should ALWAYS use correct hex decoding for salt/IV/checkCode.
fn decode_hex(s: &str, _fixed: bool) -> Result<Vec<u8>> {
    // Always use correct hex decoding - the buggy version was a misunderstanding
    let result = hex_decode(s)?;
    eprintln!("decode_hex: '{}' -> {:02x?} (first 8 bytes)", &s[..s.len().min(16)], &result[..result.len().min(8)]);
    Ok(result)
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
        Ok(BlowfishCipher {
            key: key.to_vec(),
            iv: iv.to_vec(),
        })
    }

    /// Decrypt data in CTR mode
    ///
    /// CTR mode turns a block cipher into a stream cipher.
    /// Encryption and decryption are the same operation in CTR mode.
    ///
    /// FreeARC uses LibTomCrypt which increments the entire 8-byte block as a
    /// little-endian counter, so we use Ctr64LE (full block counter for Blowfish).
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use blowfish::cipher::{KeyIvInit, StreamCipher};
        use blowfish::Blowfish;
        use ctr::Ctr64LE;  // Full 64-bit block counter for Blowfish (8-byte block)
        use crypto_common::generic_array::GenericArray;

        // Create cipher instance
        let key = GenericArray::from_slice(&self.key);
        let iv = GenericArray::from_slice(&self.iv);

        // Create CTR mode with full-block little-endian counter (as used by LibTomCrypt/FreeARC)
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
        use ctr::Ctr64LE;  // Full 64-bit block counter for Blowfish
        use crypto_common::generic_array::GenericArray;

        // Create cipher instance
        let key = GenericArray::from_slice(&self.key);
        let iv = GenericArray::from_slice(&self.iv);

        // Create CTR mode with full-block little-endian counter
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
    ///
    /// FreeARC uses LibTomCrypt which increments the entire 16-byte block as a
    /// little-endian counter, so we use Ctr128LE (full block counter for AES).
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use ctr::Ctr128LE;  // Full 128-bit block counter for AES (16-byte block)
        use crypto_common::generic_array::GenericArray;

        let mut buffer = ciphertext.to_vec();

        // Create cipher instance based on key length
        // Using Ctr128LE for LibTomCrypt/FreeARC compatibility
        match self.key.len() {
            16 => {
                use aes::Aes128;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            24 => {
                use aes::Aes192;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes192>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            32 => {
                use aes::Aes256;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes256>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            _ => return Err(anyhow!("Invalid AES key length: {}", self.key.len())),
        }

        Ok(buffer)
    }

    /// Encrypt using AES-CTR mode
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use ctr::Ctr128LE;  // Full 128-bit block counter for AES
        use crypto_common::generic_array::GenericArray;

        let mut buffer = plaintext.to_vec();

        // Create cipher instance based on key length
        // Using Ctr128LE for LibTomCrypt/FreeARC compatibility
        match self.key.len() {
            16 => {
                use aes::Aes128;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            24 => {
                use aes::Aes192;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes192>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            },
            32 => {
                use aes::Aes256;
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.iv);
                let mut cipher = Ctr128LE::<Aes256>::new(key, iv);
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

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the twofish crate
        Err(anyhow!("Twofish decryption not fully implemented"))
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
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

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the serpent crate
        Err(anyhow!("Serpent decryption not fully implemented"))
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a placeholder implementation
        // In a real implementation, we'd use the serpent crate
        Err(anyhow!("Serpent encryption not fully implemented"))
    }
}

/// Generic decryption dispatcher for cascaded ciphers
///
/// FreeARC can chain ciphers: "aes+serpent" means decrypt with serpent first, then AES
pub struct CascadedDecryptor {
    ciphers: Vec<Box<dyn CipherOp>>,
}

impl CascadedDecryptor {
    /// Verify password using the check code
    ///
    /// FreeARC stores a check code in the encryption parameters that allows
    /// quick verification of the password without attempting decryption.
    /// The check code is derived alongside the key using PBKDF2-HMAC-SHA512.
    fn verify_password(enc_info: &EncryptionInfo, password: &str) -> Result<bool> {
        // If no check code or salt is provided, skip verification
        let (check_code, salt) = match (&enc_info.code, &enc_info.salt) {
            (Some(code), Some(salt)) => (code, salt),
            _ => {
                eprintln!("No check code or salt available - skipping password verification");
                return Ok(true);
            }
        };

        let check_code_size = check_code.len();
        if check_code_size == 0 {
            eprintln!("Empty check code - skipping password verification");
            return Ok(true);
        }

        // Derive key + check_code bytes using PBKDF2-HMAC-SHA512
        // FreeARC uses: pbkdf2Hmac password salt numIterations (keySize+checkCodeSize)
        let total_size = enc_info.key_size + check_code_size;

        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        // Get password bytes - handle :f flag for UTF-8 vs Latin-1 encoding
        let password_bytes = if enc_info.fixed {
            // With :f flag, password is UTF-8 encoded
            password.as_bytes().to_vec()
        } else {
            // Without :f flag, password is Latin-1 (raw bytes)
            // For ASCII passwords, this is the same as UTF-8
            password.chars().map(|c| c as u8).collect::<Vec<u8>>()
        };

        let mut derived = vec![0u8; total_size];
        pbkdf2_hmac::<Sha512>(
            &password_bytes,
            salt,
            enc_info.iterations,
            &mut derived
        );

        // The check code is the last check_code_size bytes of the derived data
        let derived_check_code = &derived[enc_info.key_size..];

        eprintln!("Password verification:");
        eprintln!("  Salt (first 8 bytes): {:02x?}", &salt[..salt.len().min(8)]);
        eprintln!("  Iterations: {}", enc_info.iterations);
        eprintln!("  Key size: {} bytes", enc_info.key_size);
        eprintln!("  Check code size: {} bytes", check_code_size);
        eprintln!("  Expected check code: {:02x?}", check_code);
        eprintln!("  Derived check code: {:02x?}", derived_check_code);

        if derived_check_code == check_code.as_slice() {
            eprintln!("  Password verification: SUCCESS");
            Ok(true)
        } else {
            eprintln!("  Password verification: FAILED - wrong password");
            Ok(false)
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

        let mut ciphers: Vec<Box<dyn CipherOp>> = vec![];

        // Use parsed parameters from encryption info
        let salt = enc_info.salt.as_ref().map(|s| s.as_slice());

        // Get password bytes - handle :f flag for UTF-8 vs Latin-1 encoding
        // Must be consistent with verify_password
        let password_bytes = if enc_info.fixed {
            // With :f flag, password is UTF-8 encoded
            password.as_bytes().to_vec()
        } else {
            // Without :f flag, password is Latin-1 (raw bytes)
            // For ASCII passwords, this is the same as UTF-8
            password.chars().map(|c| c as u8).collect::<Vec<u8>>()
        };

        // Derive key using PBKDF2-HMAC-SHA512 with correct password encoding
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        let salt_bytes = salt.unwrap_or(&[]);
        let mut key = vec![0u8; enc_info.key_size];
        pbkdf2_hmac::<Sha512>(
            &password_bytes,
            salt_bytes,
            enc_info.iterations,
            &mut key
        );

        eprintln!("Derived encryption key:");
        eprintln!("  Key size: {} bytes", key.len());
        eprintln!("  Key (first 16 bytes): {:02x?}", &key[..key.len().min(16)]);

        // Get IV from encryption info (required for FreeARC)
        let iv = enc_info.iv.as_ref().ok_or_else(|| {
            anyhow!("No IV provided in encryption parameters")
        })?;

        eprintln!("IV: {:02x?}", iv);

        for algo in &enc_info.algorithms {
            let cipher: Box<dyn CipherOp> = match algo {
                CipherAlgorithm::None => continue,
                CipherAlgorithm::Blowfish => {
                    // Blowfish uses 8-byte IV
                    let blowfish_iv = if iv.len() >= 8 {
                        iv[..8].to_vec()
                    } else {
                        return Err(anyhow!("Blowfish requires 8-byte IV, got {} bytes", iv.len()));
                    };

                    eprintln!("Blowfish decrypt: key_size={}, iv_len={}, iterations={}",
                             key.len(), blowfish_iv.len(), enc_info.iterations);
                    Box::new(BlowfishCipher::new(&key, &blowfish_iv)?)
                }
                CipherAlgorithm::AES => {
                    // AES uses 16-byte IV
                    if iv.len() != 16 {
                        return Err(anyhow!("AES requires 16-byte IV, got {} bytes", iv.len()));
                    }

                    eprintln!("AES decrypt: key_size={}, iv_len={}, iterations={}",
                             key.len(), iv.len(), enc_info.iterations);
                    Box::new(AesCipher::new(&key, iv)?)
                }
                CipherAlgorithm::Twofish => {
                    // Twofish uses 16-byte IV
                    if iv.len() != 16 {
                        return Err(anyhow!("Twofish requires 16-byte IV, got {} bytes", iv.len()));
                    }

                    eprintln!("Twofish decrypt: key_size={}, iv_len={}, iterations={}",
                             key.len(), iv.len(), enc_info.iterations);
                    Box::new(TwofishCipher::new(&key, iv)?)
                }
                CipherAlgorithm::Serpent => {
                    // Serpent uses 16-byte IV
                    if iv.len() != 16 {
                        return Err(anyhow!("Serpent requires 16-byte IV, got {} bytes", iv.len()));
                    }

                    eprintln!("Serpent decrypt: key_size={}, iv_len={}, iterations={}",
                             key.len(), iv.len(), enc_info.iterations);
                    Box::new(SerpentCipher::new(&key, iv)?)
                }
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

/// Create a decryptor from a compression method string with password
///
/// This is the main entry point for integrating decryption into the pipeline.
/// Call this when you detect encryption in the compression method.
pub fn create_decryptor(method_string: &str, password: &str, crypto_flags: Option<&str>) -> Result<CascadedDecryptor> {
    let enc_info = EncryptionInfo::from_method_string(method_string, crypto_flags)?;
    CascadedDecryptor::new(&enc_info, password)
}

/// Decrypt data in the decompression pipeline
///
/// Usage in your decompression code:
/// ```ignore
/// let decryptor = create_decryptor(&compression_method, password, None)?;
/// if decryptor.is_encrypted() {
///     data = decryptor.decrypt(&data)?;
/// }
/// ```
pub fn decrypt_data(
    compression_method: &str,
    encrypted_data: &[u8],
    password: &str,
    crypto_flags: Option<&str>,
) -> Result<Vec<u8>> {
    let decryptor = create_decryptor(compression_method, password, crypto_flags)?;
    decryptor.decrypt(encrypted_data)
}

// ============================================================================
// Encryption Generation (for archive creation)
// ============================================================================

/// Encode bytes as lowercase hex string
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate encryption parameters for archive creation
pub struct EncryptionGenerator {
    /// The encryption algorithm to use
    pub algorithm: CipherAlgorithm,
    /// Key size in bits (e.g., 448 for Blowfish, 256 for AES-256)
    pub key_bits: usize,
    /// Number of PBKDF2 iterations
    pub iterations: u32,
}

impl EncryptionGenerator {
    /// Create a Blowfish-448 encryption generator
    pub fn blowfish_448() -> Self {
        EncryptionGenerator {
            algorithm: CipherAlgorithm::Blowfish,
            key_bits: 448,
            iterations: 1000,
        }
    }

    /// Create an AES-256 encryption generator
    pub fn aes_256() -> Self {
        EncryptionGenerator {
            algorithm: CipherAlgorithm::AES,
            key_bits: 256,
            iterations: 1000,
        }
    }

    /// Create an AES-128 encryption generator
    pub fn aes_128() -> Self {
        EncryptionGenerator {
            algorithm: CipherAlgorithm::AES,
            key_bits: 128,
            iterations: 1000,
        }
    }

    /// Get the IV size for the algorithm
    fn iv_size(&self) -> usize {
        match self.algorithm {
            CipherAlgorithm::Blowfish => 8,  // 64-bit block
            _ => 16,  // 128-bit block for AES/Twofish/Serpent
        }
    }

    /// Get the algorithm name for the method string
    fn algorithm_name(&self) -> &'static str {
        match self.algorithm {
            CipherAlgorithm::Blowfish => "blowfish",
            CipherAlgorithm::AES => "aes",
            CipherAlgorithm::Twofish => "twofish",
            CipherAlgorithm::Serpent => "serpent",
            CipherAlgorithm::None => "none",
        }
    }

    /// Generate encryption setup for archive creation
    ///
    /// Returns: (archive_method_string, CascadedDecryptor_for_encryption)
    ///
    /// The returned method string can be stored in the archive block descriptor.
    /// The CascadedDecryptor can be used for both encryption and decryption
    /// (they're the same operation in CTR mode).
    pub fn generate(&self, password: &str) -> Result<(String, CascadedDecryptor)> {
        use rand::RngCore;
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        let mut rng = rand::thread_rng();

        // Generate random IV and salt
        let mut iv = vec![0u8; self.iv_size()];
        let mut salt = vec![0u8; self.key_bits / 8];
        rng.fill_bytes(&mut iv);
        rng.fill_bytes(&mut salt);

        // FreeARC uses a 2-byte check code by default
        let check_code_size = 2;
        let key_size = self.key_bits / 8;

        // Derive key + check_code using PBKDF2-HMAC-SHA512
        let mut derived = vec![0u8; key_size + check_code_size];
        pbkdf2_hmac::<Sha512>(
            password.as_bytes(),
            &salt,
            self.iterations,
            &mut derived,
        );

        let check_code = &derived[key_size..];

        // Format the method string for archive storage
        // Format: algorithm-bits/ctr:nITER:s<salt>:c<code>:i<iv>:f
        // The :f flag indicates UTF-8 password encoding
        let method_string = format!(
            "{}-{}/ctr:n{}:s{}:c{}:i{}:f",
            self.algorithm_name(),
            self.key_bits,
            self.iterations,
            hex_encode(&salt),
            hex_encode(check_code),
            hex_encode(&iv)
        );

        // Create the encryptor (CascadedDecryptor works for both encrypt/decrypt in CTR mode)
        let enc_info = EncryptionInfo::from_method_string(&method_string, None)?;
        let encryptor = CascadedDecryptor::new(&enc_info, password)?;

        Ok((method_string, encryptor))
    }
}

/// Create an encryptor for archive creation from a simple specification
///
/// # Arguments
/// * `encryption_spec` - Simple encryption name like "blowfish", "aes-256", "aes-128"
/// * `password` - The password to use for encryption
///
/// # Returns
/// * `(method_string, encryptor)` - The method string to store in the archive and the encryptor to use
pub fn create_encryptor(encryption_spec: &str, password: &str) -> Result<(String, CascadedDecryptor)> {
    let generator = match encryption_spec.to_lowercase().as_str() {
        "blowfish" | "blowfish-448" => EncryptionGenerator::blowfish_448(),
        "aes" | "aes-256" => EncryptionGenerator::aes_256(),
        "aes-128" => EncryptionGenerator::aes_128(),
        _ => return Err(anyhow!("Unknown encryption method: {}. Supported: blowfish, aes-256, aes-128", encryption_spec)),
    };
    generator.generate(password)
}

/// Encrypt data for archive storage
///
/// # Arguments
/// * `encryption_spec` - Simple encryption name like "blowfish", "aes-256"
/// * `plaintext` - The data to encrypt
/// * `password` - The password to use for encryption
///
/// # Returns
/// * `(method_string, encrypted_data)` - The method string and encrypted data
pub fn encrypt_data(
    encryption_spec: &str,
    plaintext: &[u8],
    password: &str,
) -> Result<(String, Vec<u8>)> {
    let (method_string, encryptor) = create_encryptor(encryption_spec, password)?;
    let encrypted = encryptor.encrypt(plaintext)?;
    Ok((method_string, encrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_info_parsing() {
        let enc = EncryptionInfo::from_method_string("blowfish", None).unwrap();
        assert_eq!(enc.algorithms, vec![CipherAlgorithm::Blowfish]);

        let enc = EncryptionInfo::from_method_string("aes+serpent", None).unwrap();
        assert_eq!(
            enc.algorithms,
            vec![CipherAlgorithm::AES, CipherAlgorithm::Serpent]
        );
    }

    #[test]
    fn test_empty_encryption() {
        let enc = EncryptionInfo::from_method_string("", None).unwrap();
        assert_eq!(enc.algorithms, vec![CipherAlgorithm::None]);
    }

    #[test]
    fn test_hex_decode() {
        let result = hex_decode("4f62").unwrap();
        assert_eq!(result, vec![0x4f, 0x62]);
    }

    // TODO: Add roundtrip tests once crypto implementations are complete
    // #[test]
    // fn test_blowfish_roundtrip() { ... }
}