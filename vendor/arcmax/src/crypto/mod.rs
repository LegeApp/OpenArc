/// Key-derivation function selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KdfAlgorithm {
    #[default]
    Pbkdf2,
    Argon2id,
}

/// Derive a key from `password` using the selected KDF.
///
/// Returns `key_len` bytes. `salt` should be at least 16 bytes of random data.
/// `iterations` maps to PBKDF2 iterations or Argon2id time_cost.
pub fn derive_key(
    kdf: KdfAlgorithm,
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    key_len: usize,
) -> crate::error::Result<Vec<u8>> {
    use crate::error::ArcError;
    match kdf {
        KdfAlgorithm::Pbkdf2 => {
            use pbkdf2::pbkdf2_hmac;
            use sha2::Sha512;
            let mut key = vec![0u8; key_len];
            pbkdf2_hmac::<Sha512>(password, salt, iterations, &mut key);
            Ok(key)
        }
        KdfAlgorithm::Argon2id => {
            use argon2::{Argon2, Params, Version};
            let params = Params::new(
                64 * 1024,  // m_cost: 64 MiB
                iterations, // t_cost
                1,          // p_cost
                Some(key_len),
            )
            .map_err(|e| ArcError::InvalidMethod(e.to_string()))?;
            let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
            let mut key = vec![0u8; key_len];
            argon2
                .hash_password_into(password, salt, &mut key)
                .map_err(|e| ArcError::InvalidMethod(e.to_string()))?;
            Ok(key)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherAlgorithm {
    Aes256,
    Blowfish448,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherMode {
    Ctr,
    Cfb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionOptions {
    pub cipher: CipherAlgorithm,
    pub mode: CipherMode,
    pub kdf: KdfAlgorithm,
    pub iterations: u32,
    pub rounds: u32,
    pub salt: Vec<u8>,
    pub iv: Vec<u8>,
    pub verification_code: Vec<u8>,
    pub fixed_hex: bool,
}

impl Default for EncryptionOptions {
    fn default() -> Self {
        Self {
            cipher: CipherAlgorithm::Aes256,
            mode: CipherMode::Ctr,
            kdf: KdfAlgorithm::Pbkdf2,
            iterations: 1000,
            rounds: 0,
            salt: Vec::new(),
            iv: Vec::new(),
            verification_code: Vec::new(),
            fixed_hex: false,
        }
    }
}
