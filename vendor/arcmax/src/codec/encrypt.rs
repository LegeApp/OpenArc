//! Encryption codec — AES-256/CTR and Blowfish-448/CTR wrappers.
//!
//! Framing: the compressed output is raw ciphertext. Salt and IV are stored
//! separately in the archive's method string (`EncryptionOptions`). When
//! compressing, if `options.salt` or `options.iv` are empty the codec generates
//! random values and stores them in `self.options` so the caller can retrieve
//! them afterwards to embed in the archive header.

use std::io::{Read, Write};

use rand::RngCore;

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::crypto::{CipherAlgorithm, CipherMode, EncryptionOptions};
use crate::error::{ArcError, Result};

/// Encryption key lengths in bytes.
const AES256_KEY_LEN: usize = 32;
const BLOWFISH_KEY_LEN: usize = 56; // 448-bit

/// IV lengths in bytes (= block size for CTR mode).
const AES_IV_LEN: usize = 16;
const BLOWFISH_IV_LEN: usize = 8;

/// A codec that encrypts/decrypts using the cipher described by `EncryptionOptions`.
///
/// The password is passed at construction; it is never stored in the archive.
pub struct EncryptionCodec {
    options: EncryptionOptions,
    password: Vec<u8>,
}

impl EncryptionCodec {
    pub fn new(options: EncryptionOptions, password: Vec<u8>) -> Self {
        Self { options, password }
    }

    /// After `compress()`, these options contain the generated salt and IV.
    /// Embed them in the archive method string so `decompress()` can recover them.
    pub fn options(&self) -> &EncryptionOptions {
        &self.options
    }

    fn key_len(&self) -> usize {
        match self.options.cipher {
            CipherAlgorithm::Aes256 => AES256_KEY_LEN,
            CipherAlgorithm::Blowfish448 => BLOWFISH_KEY_LEN,
        }
    }

    fn iv_len(&self) -> usize {
        match self.options.cipher {
            CipherAlgorithm::Aes256 => AES_IV_LEN,
            CipherAlgorithm::Blowfish448 => BLOWFISH_IV_LEN,
        }
    }

    fn ensure_salt_and_iv(&mut self) {
        if self.options.salt.is_empty() {
            let mut salt = vec![0u8; 32];
            rand::rng().fill_bytes(&mut salt);
            self.options.salt = salt;
        }
        if self.options.iv.is_empty() {
            let mut iv = vec![0u8; self.iv_len()];
            rand::rng().fill_bytes(&mut iv);
            self.options.iv = iv;
        }
    }

    fn derive_key(&self) -> Result<Vec<u8>> {
        crate::crypto::derive_key(
            self.options.kdf,
            &self.password,
            &self.options.salt,
            self.options.iterations,
            self.key_len(),
        )
    }

    fn apply_cipher(&self, data: &[u8], key: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        match self.options.mode {
            CipherMode::Ctr => self.apply_ctr(data, key),
            CipherMode::Cfb if encrypt => self.apply_cfb_encrypt(data, key),
            CipherMode::Cfb => self.apply_cfb_decrypt(data, key),
        }
    }

    fn apply_cfb_encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        use cfb_mode::cipher::{AsyncStreamCipher, KeyIvInit};
        use crypto_common::generic_array::GenericArray;

        let iv = &self.options.iv;
        let mut buf = data.to_vec();

        match self.options.cipher {
            CipherAlgorithm::Aes256 => {
                use aes::Aes256;
                use cfb_mode::Encryptor;
                let k = GenericArray::from_slice(key);
                let iv_arr = GenericArray::from_slice(iv);
                Encryptor::<Aes256>::new(k, iv_arr).encrypt(&mut buf);
            }
            CipherAlgorithm::Blowfish448 => {
                use blowfish::Blowfish;
                use cfb_mode::Encryptor;
                let k = GenericArray::from_slice(key);
                let iv_arr = GenericArray::from_slice(iv);
                Encryptor::<Blowfish>::new(k, iv_arr).encrypt(&mut buf);
            }
        }
        Ok(buf)
    }

    fn apply_cfb_decrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        use cfb_mode::cipher::{AsyncStreamCipher, KeyIvInit};
        use crypto_common::generic_array::GenericArray;

        let iv = &self.options.iv;
        let mut buf = data.to_vec();

        match self.options.cipher {
            CipherAlgorithm::Aes256 => {
                use aes::Aes256;
                use cfb_mode::Decryptor;
                let k = GenericArray::from_slice(key);
                let iv_arr = GenericArray::from_slice(iv);
                Decryptor::<Aes256>::new(k, iv_arr).decrypt(&mut buf);
            }
            CipherAlgorithm::Blowfish448 => {
                use blowfish::Blowfish;
                use cfb_mode::Decryptor;
                let k = GenericArray::from_slice(key);
                let iv_arr = GenericArray::from_slice(iv);
                Decryptor::<Blowfish>::new(k, iv_arr).decrypt(&mut buf);
            }
        }
        Ok(buf)
    }

    fn apply_ctr(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        use blowfish::cipher::{KeyIvInit, StreamCipher};
        use crypto_common::generic_array::GenericArray;

        let iv = &self.options.iv;
        let mut buf = data.to_vec();

        match self.options.cipher {
            CipherAlgorithm::Aes256 => {
                use aes::cipher::{KeyIvInit as _, StreamCipher as _};
                use aes::Aes256;
                use ctr::Ctr128LE;
                let iv_arr = GenericArray::from_slice(iv);
                let k = GenericArray::from_slice(key);
                let mut c = Ctr128LE::<Aes256>::new(k, iv_arr);
                c.apply_keystream(&mut buf);
            }
            CipherAlgorithm::Blowfish448 => {
                use blowfish::Blowfish;
                use ctr::Ctr64LE;
                let k = GenericArray::from_slice(key);
                let iv_arr = GenericArray::from_slice(iv);
                let mut c = Ctr64LE::<Blowfish>::new(k, iv_arr);
                c.apply_keystream(&mut buf);
            }
        }

        Ok(buf)
    }
}

impl Codec for EncryptionCodec {
    fn name(&self) -> &'static str {
        "encryption"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut plaintext = Vec::new();
        input.read_to_end(&mut plaintext)?;
        let bytes_in = plaintext.len() as u64;

        self.ensure_salt_and_iv();
        let key = self.derive_key()?;
        let ciphertext = self.apply_cipher(&plaintext, &key, true)?;
        output.write_all(&ciphertext)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: ciphertext.len() as u64,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        if self.options.salt.is_empty() {
            return Err(ArcError::Codec {
                codec: "encryption",
                message: "salt is empty — cannot derive key without salt".to_string(),
            });
        }
        if self.options.iv.is_empty() {
            return Err(ArcError::Codec {
                codec: "encryption",
                message: "IV is empty — cannot decrypt without IV".to_string(),
            });
        }

        let mut ciphertext = Vec::new();
        input.read_to_end(&mut ciphertext)?;
        let bytes_in = ciphertext.len() as u64;

        let key = self.derive_key()?;
        let plaintext = self.apply_cipher(&ciphertext, &key, false)?;
        output.write_all(&plaintext)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: plaintext.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }
}
