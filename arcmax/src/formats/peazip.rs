//! PEA (PeaZip Archive) format implementation
//!
//! PEA is a native archive format created by PeaZip with the following features:
//! - Multi-level integrity checking (stream, object, volume)
//! - Multiple compression methods (DEFLATE-based PCOMPRESS0-3)
//! - Strong encryption (AES, Twofish, Serpent in EAX mode)
//! - Cascaded encryption support (AES → Twofish → Serpent)
//! - Multi-volume support
//!
//! Format specification:
//! - Archive Header: 10 bytes (magic 0xEA, version, revision, etc.)
//! - Stream Header: 10 bytes (POD trigger, compression, control algorithms)
//! - Crypto Subheader: 16 bytes (salt, password verification)
//! - Data blocks with authentication tags

use std::io::{Read, Seek, SeekFrom, Cursor, Write as IoWrite};
use std::path::Path;
use std::fs::File;
use anyhow::{anyhow, Result};
use crate::core::archive::{ArchiveReader, FileEntry};

// PEA Magic byte
const PEA_MAGIC: u8 = 0xEA;  // 234

// Current supported format version/revision
const PEA_FORMAT_VER: u8 = 1;
const PEA_FORMAT_REV: u8 = 6;

// POD trigger signature (start of stream)
const POD_TRIGGER: [u8; 6] = [0x00, 0x00, 0x50, 0x4F, 0x44, 0x00]; // "\0\0POD\0"

// EOS (End of Stream) trigger
const EOS_TRIGGER: [u8; 2] = [0x00, 0x00];

// Control algorithm codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAlgorithm {
    NoAlgo,      // 0x00
    Adler32,     // 0x01
    Crc32,       // 0x02
    Crc64,       // 0x03
    Md5,         // 0x10
    Ripemd160,   // 0x11
    Sha1,        // 0x12
    Sha256,      // 0x13
    Sha512,      // 0x14
    Whirlpool,   // 0x15
    Sha3_256,    // 0x16
    Sha3_512,    // 0x17
    Blake2s,     // 0x18
    Blake2b,     // 0x19
    Hmac,        // 0x30 - HMAC-SHA1 (requires password)
    Eax,         // 0x31 - AES-128-EAX (requires password)
    Tf,          // 0x32 - Twofish-128-EAX (requires password)
    Sp,          // 0x33 - Serpent-128-EAX (requires password)
    Eax256,      // 0x41 - AES-256-EAX (requires password)
    Tf256,       // 0x42 - Twofish-256-EAX (requires password)
    Sp256,       // 0x43 - Serpent-256-EAX (requires password)
    TriAts,      // 0x44 - Triple cascaded: AES → Twofish → Serpent
    TriTsa,      // 0x45 - Triple cascaded: Twofish → Serpent → AES
    TriSat,      // 0x46 - Triple cascaded: Serpent → AES → Twofish
    // Additional cascaded modes 0x47-0x4C exist
}

impl ControlAlgorithm {
    fn from_byte(b: u8) -> Result<Self> {
        match b {
            0x00 => Ok(Self::NoAlgo),
            0x01 => Ok(Self::Adler32),
            0x02 => Ok(Self::Crc32),
            0x03 => Ok(Self::Crc64),
            0x10 => Ok(Self::Md5),
            0x11 => Ok(Self::Ripemd160),
            0x12 => Ok(Self::Sha1),
            0x13 => Ok(Self::Sha256),
            0x14 => Ok(Self::Sha512),
            0x15 => Ok(Self::Whirlpool),
            0x16 => Ok(Self::Sha3_256),
            0x17 => Ok(Self::Sha3_512),
            0x18 => Ok(Self::Blake2s),
            0x19 => Ok(Self::Blake2b),
            0x30 => Ok(Self::Hmac),
            0x31 => Ok(Self::Eax),
            0x32 => Ok(Self::Tf),
            0x33 => Ok(Self::Sp),
            0x41 => Ok(Self::Eax256),
            0x42 => Ok(Self::Tf256),
            0x43 => Ok(Self::Sp256),
            0x44 => Ok(Self::TriAts),
            0x45 => Ok(Self::TriTsa),
            0x46 => Ok(Self::TriSat),
            0x47..=0x4C => Ok(Self::TriAts), // Map all cascaded modes to TriAts for now
            _ => Err(anyhow!("Unknown control algorithm: 0x{:02X}", b)),
        }
    }

    fn requires_password(&self) -> bool {
        matches!(
            self,
            Self::Hmac
                | Self::Eax
                | Self::Tf
                | Self::Sp
                | Self::Eax256
                | Self::Tf256
                | Self::Sp256
                | Self::TriAts
                | Self::TriTsa
                | Self::TriSat
        )
    }

    fn header_size(&self) -> usize {
        match self {
            Self::NoAlgo => 10,
            Self::Hmac | Self::Eax | Self::Tf | Self::Sp => 10 + 16,
            Self::Eax256 | Self::Tf256 | Self::Sp256 => 10 + 16,
            Self::TriAts | Self::TriTsa | Self::TriSat => 10 + 48, // 3 x 16 byte subheaders
            _ => 10,
        }
    }

    fn auth_tag_size(&self) -> usize {
        match self {
            Self::NoAlgo => 0,
            Self::Adler32 => 4,
            Self::Crc32 => 4,
            Self::Crc64 => 8,
            Self::Md5 => 16,
            Self::Ripemd160 => 20,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 => 64,
            Self::Whirlpool => 64,
            Self::Sha3_256 => 32,
            Self::Sha3_512 => 64,
            Self::Blake2s => 32,
            Self::Blake2b => 64,
            Self::Hmac => 16,
            Self::Eax | Self::Tf | Self::Sp => 16,
            Self::Eax256 | Self::Tf256 | Self::Sp256 => 16,
            Self::TriAts | Self::TriTsa | Self::TriSat => 48, // SHA3-384 hash of 3 tags
        }
    }
}

// Compression algorithm codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    PCompress0, // 0 - Stored (no compression)
    PCompress1, // 1 - DEFLATE level 3
    PCompress2, // 2 - DEFLATE level 6
    PCompress3, // 3 - DEFLATE level 9 (best)
}

impl CompressionAlgorithm {
    fn from_byte(b: u8) -> Result<Self> {
        match b {
            0 => Ok(Self::PCompress0),
            1 => Ok(Self::PCompress1),
            2 => Ok(Self::PCompress2),
            3 => Ok(Self::PCompress3),
            _ => Err(anyhow!("Unknown compression algorithm: {}", b)),
        }
    }
}

/// PEA Archive Header (10 bytes)
#[derive(Debug, Clone)]
pub struct PeaArchiveHeader {
    pub magic: u8,                    // 0xEA
    pub version: u8,                  // Format version (1)
    pub revision: u8,                 // Format revision (0-6)
    pub volume_control: ControlAlgorithm, // Volume integrity algorithm
    pub ecc_scheme: u8,               // Reserved (0)
    pub os_id: u8,                    // OS identifier
    pub datetime_encoding: u8,        // Date/time encoding system
    pub char_encoding: u8,            // Character encoding (1 = UTF-8)
    pub cpu_endian: u8,               // CPU type and endianness
    pub iteration_multiplier: u8,     // KDF iteration count multiplier
}

impl PeaArchiveHeader {
    fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 10 {
            return Err(anyhow!("Archive header too short: {} bytes", data.len()));
        }

        let magic = data[0];
        if magic != PEA_MAGIC {
            return Err(anyhow!(
                "Invalid PEA magic byte: 0x{:02X} (expected 0xEA)",
                magic
            ));
        }

        let version = data[1];
        let revision = data[2];

        // Check version compatibility
        if version > PEA_FORMAT_VER || (version == PEA_FORMAT_VER && revision > PEA_FORMAT_REV) {
            eprintln!(
                "Warning: PEA format {}.{} may not be fully supported (max supported: {}.{})",
                version, revision, PEA_FORMAT_VER, PEA_FORMAT_REV
            );
        }

        Ok(PeaArchiveHeader {
            magic,
            version,
            revision,
            volume_control: ControlAlgorithm::from_byte(data[3])?,
            ecc_scheme: data[4],
            os_id: data[5],
            datetime_encoding: data[6],
            char_encoding: data[7],
            cpu_endian: data[8],
            iteration_multiplier: data[9],
        })
    }
}

/// PEA Stream Header (10 bytes, starts with POD trigger)
#[derive(Debug, Clone)]
pub struct PeaStreamHeader {
    pub compression: CompressionAlgorithm,
    pub stream_ecc: u8,              // Reserved (0)
    pub stream_control: ControlAlgorithm,
    pub object_control: ControlAlgorithm,
}

impl PeaStreamHeader {
    fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 10 {
            return Err(anyhow!("Stream header too short: {} bytes", data.len()));
        }

        // Verify POD trigger
        if &data[0..6] != &POD_TRIGGER {
            return Err(anyhow!(
                "Invalid POD trigger: {:02X?} (expected {:02X?})",
                &data[0..6],
                POD_TRIGGER
            ));
        }

        Ok(PeaStreamHeader {
            compression: CompressionAlgorithm::from_byte(data[6])?,
            stream_ecc: data[7],
            stream_control: ControlAlgorithm::from_byte(data[8])?,
            object_control: ControlAlgorithm::from_byte(data[9])?,
        })
    }
}

/// FCA-style Crypto Subheader (16 bytes)
#[derive(Debug, Clone)]
pub struct CryptoSubheader {
    pub fca_sig: u8,    // Signature byte (0xFC in original, 0 in PEA)
    pub flags: u8,      // Flags byte
    pub salt: [u8; 12], // 96-bit salt (3 x 32-bit words)
    pub pw_ver: u16,    // Password verification word
}

impl CryptoSubheader {
    fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(anyhow!("Crypto subheader too short: {} bytes", data.len()));
        }

        let mut salt = [0u8; 12];
        salt.copy_from_slice(&data[2..14]);

        Ok(CryptoSubheader {
            fca_sig: data[0],
            flags: data[1],
            salt,
            pw_ver: u16::from_le_bytes([data[14], data[15]]),
        })
    }
}

/// PEA object metadata (file or directory entry in stream)
#[derive(Debug, Clone)]
pub struct PeaObject {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub mtime: u64,
    pub attributes: u32,
    pub is_dir: bool,
    pub offset: u64,  // Offset in decompressed stream
}

/// AES-EAX encryption context
pub struct AesEaxContext {
    key: Vec<u8>,
    nonce: Vec<u8>,
}

impl AesEaxContext {
    /// Initialize AES-EAX context with password and salt using PBKDF2
    pub fn new(password: &str, salt: &[u8], iterations: u32, key_size: usize) -> Result<Self> {
        use pbkdf2::pbkdf2_hmac;
        use sha2::Sha512;

        // PEA uses PBKDF2-HMAC-SHA512 (or Whirlpool for AES, SHA512 for Twofish, SHA3-512 for Serpent)
        // We derive: key (16 or 32 bytes) + nonce (16 bytes) + pw_ver (2 bytes)
        let derived_len = key_size + 16 + 2;
        let mut derived = vec![0u8; derived_len];

        pbkdf2_hmac::<Sha512>(password.as_bytes(), salt, iterations, &mut derived);

        let key = derived[..key_size].to_vec();
        let nonce = derived[key_size..key_size + 16].to_vec();

        Ok(AesEaxContext { key, nonce })
    }

    /// Decrypt data using AES-EAX mode
    /// For simplicity, we use AES-CTR for now since EAX is CTR + OMAC
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use aes::cipher::{KeyIvInit, StreamCipher};
        use ctr::Ctr64LE;
        use crypto_common::generic_array::GenericArray;

        let mut buffer = ciphertext.to_vec();

        // EAX mode uses CTR internally with the nonce
        match self.key.len() {
            16 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.nonce);
                let mut cipher = Ctr64LE::<aes::Aes128>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            }
            32 => {
                let key = GenericArray::from_slice(&self.key);
                let iv = GenericArray::from_slice(&self.nonce);
                let mut cipher = Ctr64LE::<aes::Aes256>::new(key, iv);
                cipher.apply_keystream(&mut buffer);
            }
            _ => return Err(anyhow!("Invalid AES key size: {}", self.key.len())),
        }

        Ok(buffer)
    }
}

/// Main PEA Archive Reader
pub struct PeaArchive<R: Read + Seek + Send> {
    reader: std::sync::Mutex<R>,
    archive_header: PeaArchiveHeader,
    stream_header: PeaStreamHeader,
    crypto_subheader: Option<CryptoSubheader>,
    password: Option<String>,
    objects: Vec<PeaObject>,
    data_start_pos: u64,
}

impl<R: Read + Seek + Send> PeaArchive<R> {
    /// Create a new PEA archive reader
    pub fn new(mut reader: R, password: Option<String>) -> Result<Self> {
        // Read and parse archive header (10 bytes)
        let mut archive_hdr_buf = [0u8; 10];
        reader.read_exact(&mut archive_hdr_buf)?;
        let archive_header = PeaArchiveHeader::parse(&archive_hdr_buf)?;

        eprintln!(
            "PEA Archive: version {}.{}, volume_control={:?}",
            archive_header.version, archive_header.revision, archive_header.volume_control
        );

        // Read and parse stream header (10 bytes)
        let mut stream_hdr_buf = [0u8; 10];
        reader.read_exact(&mut stream_hdr_buf)?;
        let stream_header = PeaStreamHeader::parse(&stream_hdr_buf)?;

        eprintln!(
            "PEA Stream: compression={:?}, stream_control={:?}, object_control={:?}",
            stream_header.compression, stream_header.stream_control, stream_header.object_control
        );

        // Check if encryption is used
        let crypto_subheader = if stream_header.stream_control.requires_password() {
            if password.is_none() {
                return Err(anyhow!(
                    "Archive is encrypted ({:?}) but no password provided",
                    stream_header.stream_control
                ));
            }

            // Read crypto subheader (16 bytes for single cipher, more for cascaded)
            let subheader_size = match stream_header.stream_control {
                ControlAlgorithm::TriAts | ControlAlgorithm::TriTsa | ControlAlgorithm::TriSat => 48,
                _ => 16,
            };

            let mut crypto_buf = vec![0u8; subheader_size];
            reader.read_exact(&mut crypto_buf)?;

            let subhdr = CryptoSubheader::parse(&crypto_buf)?;
            eprintln!(
                "PEA Crypto: salt={:02X?}, pw_ver=0x{:04X}",
                &subhdr.salt, subhdr.pw_ver
            );

            Some(subhdr)
        } else {
            None
        };

        // Record position where data starts
        let data_start_pos = reader.stream_position()?;

        // Parse the stream to extract object metadata
        let objects = Self::parse_stream(
            &mut reader,
            &archive_header,
            &stream_header,
            crypto_subheader.as_ref(),
            password.as_deref(),
        )?;

        let reader = std::sync::Mutex::new(reader);

        Ok(PeaArchive {
            reader,
            archive_header,
            stream_header,
            crypto_subheader,
            password,
            objects,
            data_start_pos,
        })
    }

    /// Parse the PEA stream to extract object metadata
    fn parse_stream(
        reader: &mut R,
        archive_header: &PeaArchiveHeader,
        stream_header: &PeaStreamHeader,
        crypto_subheader: Option<&CryptoSubheader>,
        password: Option<&str>,
    ) -> Result<Vec<PeaObject>> {
        let mut objects = Vec::new();

        // Get stream data
        let current_pos = reader.stream_position()?;
        reader.seek(SeekFrom::End(0))?;
        let file_size = reader.stream_position()?;
        reader.seek(SeekFrom::Start(current_pos))?;

        // Calculate data size (excluding auth tag)
        let auth_tag_size = stream_header.stream_control.auth_tag_size() as u64;
        let data_size = file_size - current_pos - auth_tag_size;

        eprintln!(
            "Stream data: {} bytes (auth tag: {} bytes)",
            data_size, auth_tag_size
        );

        // Read the entire stream data
        let mut encrypted_data = vec![0u8; data_size as usize];
        reader.read_exact(&mut encrypted_data)?;

        // Decrypt if needed
        let decrypted_data = if let (Some(crypto), Some(pwd)) = (crypto_subheader, password) {
            Self::decrypt_stream(stream_header, crypto, pwd, &encrypted_data, archive_header)?
        } else {
            encrypted_data
        };

        // Decompress if needed
        let decompressed_data = Self::decompress_stream(stream_header, &decrypted_data)?;

        // Parse objects from decompressed data
        objects = Self::parse_objects(&decompressed_data)?;

        Ok(objects)
    }

    /// Decrypt the stream data
    fn decrypt_stream(
        stream_header: &PeaStreamHeader,
        crypto: &CryptoSubheader,
        password: &str,
        data: &[u8],
        archive_header: &PeaArchiveHeader,
    ) -> Result<Vec<u8>> {
        // Calculate iterations based on algorithm and iteration multiplier
        let base_iterations = 1000u32;
        let multiplier = archive_header.iteration_multiplier as u32;
        let iterations = if multiplier > 0 {
            base_iterations * multiplier
        } else {
            base_iterations
        };

        eprintln!("Decrypting with {} iterations", iterations);

        match stream_header.stream_control {
            ControlAlgorithm::Eax => {
                let ctx = AesEaxContext::new(password, &crypto.salt, iterations, 16)?;
                ctx.decrypt(data)
            }
            ControlAlgorithm::Eax256 => {
                let ctx = AesEaxContext::new(password, &crypto.salt, iterations, 32)?;
                ctx.decrypt(data)
            }
            ControlAlgorithm::Tf | ControlAlgorithm::Tf256 => {
                // Twofish - use similar approach
                // For now, we'll use AES as a placeholder until twofish crate is added
                eprintln!("Warning: Twofish not fully implemented, falling back to AES");
                let key_size = if stream_header.stream_control == ControlAlgorithm::Tf256 {
                    32
                } else {
                    16
                };
                let ctx = AesEaxContext::new(password, &crypto.salt, iterations * 2, key_size)?;
                ctx.decrypt(data)
            }
            ControlAlgorithm::Sp | ControlAlgorithm::Sp256 => {
                // Serpent - use similar approach
                eprintln!("Warning: Serpent not fully implemented, falling back to AES");
                let key_size = if stream_header.stream_control == ControlAlgorithm::Sp256 {
                    32
                } else {
                    16
                };
                let ctx = AesEaxContext::new(password, &crypto.salt, iterations * 3, key_size)?;
                ctx.decrypt(data)
            }
            ControlAlgorithm::TriAts | ControlAlgorithm::TriTsa | ControlAlgorithm::TriSat => {
                // Triple cascaded encryption
                // For now, just decrypt with AES
                eprintln!("Warning: Triple cascaded encryption partially implemented");
                let ctx = AesEaxContext::new(password, &crypto.salt, iterations, 32)?;
                ctx.decrypt(data)
            }
            _ => Ok(data.to_vec()),
        }
    }

    /// Decompress the stream data
    fn decompress_stream(stream_header: &PeaStreamHeader, data: &[u8]) -> Result<Vec<u8>> {
        match stream_header.compression {
            CompressionAlgorithm::PCompress0 => {
                // No compression (stored)
                Ok(data.to_vec())
            }
            CompressionAlgorithm::PCompress1
            | CompressionAlgorithm::PCompress2
            | CompressionAlgorithm::PCompress3 => {
                // DEFLATE-based compression
                Self::decompress_deflate(data)
            }
        }
    }

    /// Decompress DEFLATE data
    fn decompress_deflate(data: &[u8]) -> Result<Vec<u8>> {
        use std::io::Read;

        // Try zlib format first (with header)
        let cursor = Cursor::new(data);
        let mut decoder = flate2::read::ZlibDecoder::new(cursor);
        let mut decompressed = Vec::new();

        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => return Ok(decompressed),
            Err(e) => {
                eprintln!("Zlib decompression failed, trying raw deflate: {}", e);
            }
        }

        // Try raw deflate (no header)
        let cursor = Cursor::new(data);
        let mut decoder = flate2::read::DeflateDecoder::new(cursor);
        let mut decompressed = Vec::new();

        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => Ok(decompressed),
            Err(e) => Err(anyhow!("DEFLATE decompression failed: {}", e)),
        }
    }

    /// Parse objects (files/directories) from decompressed stream data
    fn parse_objects(data: &[u8]) -> Result<Vec<PeaObject>> {
        let mut objects = Vec::new();
        let mut cursor = Cursor::new(data);
        let mut offset = 0u64;

        // PEA stream format:
        // For each object:
        //   - 2 bytes: filename length (LE)
        //   - N bytes: filename (UTF-8)
        //   - 8 bytes: file size (LE)
        //   - 4 bytes: file age/mtime
        //   - 4 bytes: attributes
        //   - [file data if not directory]
        //   - [object auth tag if obj_algo != NOALGO]
        //
        // The stream ends with EOS trigger (0x00 0x00)

        loop {
            // Read filename length (2 bytes)
            let mut len_buf = [0u8; 2];
            match cursor.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(_) => break, // End of data
            }

            let filename_len = u16::from_le_bytes(len_buf) as usize;

            // Check for EOS trigger
            if filename_len == 0 {
                eprintln!("Found EOS trigger, ending object parsing");
                break;
            }

            // Read filename
            let mut filename_buf = vec![0u8; filename_len];
            cursor.read_exact(&mut filename_buf)?;
            let filename = String::from_utf8_lossy(&filename_buf).to_string();

            // Read file size (8 bytes)
            let mut size_buf = [0u8; 8];
            cursor.read_exact(&mut size_buf)?;
            let size = u64::from_le_bytes(size_buf);

            // Read mtime (4 bytes)
            let mut mtime_buf = [0u8; 4];
            cursor.read_exact(&mut mtime_buf)?;
            let mtime = u32::from_le_bytes(mtime_buf) as u64;

            // Read attributes (4 bytes)
            let mut attr_buf = [0u8; 4];
            cursor.read_exact(&mut attr_buf)?;
            let attributes = u32::from_le_bytes(attr_buf);

            // Determine if directory (attribute check or size = 0 with special markers)
            let is_dir = filename.ends_with('/') || filename.ends_with('\\');

            let current_pos = cursor.position();

            objects.push(PeaObject {
                name: filename.clone(),
                size,
                compressed_size: size, // PEA uses stream compression, so compressed_size ≈ size
                mtime,
                attributes,
                is_dir,
                offset,
            });

            eprintln!("Found object: {} ({} bytes)", filename, size);

            // Skip file data
            if !is_dir && size > 0 {
                cursor.seek(SeekFrom::Current(size as i64))?;
            }

            offset = cursor.position();

            // Safety check to prevent infinite loops
            if objects.len() > 100000 {
                eprintln!("Warning: Too many objects, stopping parse");
                break;
            }
        }

        Ok(objects)
    }

    /// Extract a specific file entry
    fn extract_file(&self, entry: &FileEntry, writer: &mut dyn IoWrite) -> Result<()> {
        // Find the object in our list
        let obj = self
            .objects
            .iter()
            .find(|o| o.name == entry.name)
            .ok_or_else(|| anyhow!("Object not found: {}", entry.name))?;

        if obj.is_dir {
            return Ok(()); // Nothing to extract for directories
        }

        // Read the stream data and extract the file
        let mut reader = self.reader.lock().unwrap();

        // Seek to data start
        reader.seek(SeekFrom::Start(self.data_start_pos))?;

        // Get stream size
        let current_pos = reader.stream_position()?;
        reader.seek(SeekFrom::End(0))?;
        let file_size = reader.stream_position()?;
        reader.seek(SeekFrom::Start(current_pos))?;

        let auth_tag_size = self.stream_header.stream_control.auth_tag_size() as u64;
        let data_size = file_size - current_pos - auth_tag_size;

        // Read stream data
        let mut encrypted_data = vec![0u8; data_size as usize];
        reader.read_exact(&mut encrypted_data)?;

        // Decrypt if needed
        let decrypted_data = if let (Some(crypto), Some(pwd)) = (
            self.crypto_subheader.as_ref(),
            self.password.as_deref(),
        ) {
            Self::decrypt_stream(
                &self.stream_header,
                crypto,
                pwd,
                &encrypted_data,
                &self.archive_header,
            )?
        } else {
            encrypted_data
        };

        // Decompress if needed
        let decompressed_data = Self::decompress_stream(&self.stream_header, &decrypted_data)?;

        // Extract the specific file from decompressed data
        let start = obj.offset as usize;
        let end = start + obj.size as usize;

        if end > decompressed_data.len() {
            return Err(anyhow!(
                "File data out of bounds: {} (stream size: {})",
                end,
                decompressed_data.len()
            ));
        }

        writer.write_all(&decompressed_data[start..end])?;
        Ok(())
    }
}

impl<R: Read + Seek + Send> ArchiveReader for PeaArchive<R> {
    fn list(&mut self) -> Result<Vec<FileEntry>> {
        Ok(self
            .objects
            .iter()
            .map(|obj| FileEntry {
                name: obj.name.clone(),
                size: obj.size,
                compressed_size: obj.compressed_size,
                mtime: Some(obj.mtime),
                is_dir: obj.is_dir,
            })
            .collect())
    }

    fn extract(&mut self, entry: &FileEntry, writer: &mut dyn IoWrite) -> Result<()> {
        self.extract_file(entry, writer)
    }

    fn extract_all(&mut self, output_dir: &Path) -> Result<()> {
        let entries: Vec<_> = self
            .objects
            .iter()
            .map(|obj| FileEntry {
                name: obj.name.clone(),
                size: obj.size,
                compressed_size: obj.compressed_size,
                mtime: Some(obj.mtime),
                is_dir: obj.is_dir,
            })
            .collect();

        for entry in entries {
            let output_path = output_dir.join(&entry.name);

            if entry.is_dir {
                std::fs::create_dir_all(&output_path)?;
            } else {
                // Create parent directories
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut file = File::create(&output_path)?;
                self.extract(&entry, &mut file)?;
            }
        }

        Ok(())
    }
}

/// Check if a file is a PEA archive
pub fn is_pea_archive(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 1];
    file.read_exact(&mut magic)?;
    Ok(magic[0] == PEA_MAGIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_algorithm_from_byte() {
        assert!(matches!(
            ControlAlgorithm::from_byte(0x00).unwrap(),
            ControlAlgorithm::NoAlgo
        ));
        assert!(matches!(
            ControlAlgorithm::from_byte(0x31).unwrap(),
            ControlAlgorithm::Eax
        ));
        assert!(matches!(
            ControlAlgorithm::from_byte(0x41).unwrap(),
            ControlAlgorithm::Eax256
        ));
        assert!(ControlAlgorithm::from_byte(0xFF).is_err());
    }

    #[test]
    fn test_compression_algorithm_from_byte() {
        assert!(matches!(
            CompressionAlgorithm::from_byte(0).unwrap(),
            CompressionAlgorithm::PCompress0
        ));
        assert!(matches!(
            CompressionAlgorithm::from_byte(3).unwrap(),
            CompressionAlgorithm::PCompress3
        ));
        assert!(CompressionAlgorithm::from_byte(4).is_err());
    }

    #[test]
    fn test_pea_archive_header_parse() {
        let data: [u8; 10] = [0xEA, 1, 6, 0x02, 0, 0, 0, 1, 0, 1];
        let header = PeaArchiveHeader::parse(&data).unwrap();
        assert_eq!(header.magic, 0xEA);
        assert_eq!(header.version, 1);
        assert_eq!(header.revision, 6);
        assert!(matches!(header.volume_control, ControlAlgorithm::Crc32));
    }

    #[test]
    fn test_stream_header_parse() {
        let data: [u8; 10] = [0x00, 0x00, 0x50, 0x4F, 0x44, 0x00, 2, 0, 0x00, 0x02];
        let header = PeaStreamHeader::parse(&data).unwrap();
        assert!(matches!(
            header.compression,
            CompressionAlgorithm::PCompress2
        ));
        assert!(matches!(header.stream_control, ControlAlgorithm::NoAlgo));
        assert!(matches!(header.object_control, ControlAlgorithm::Crc32));
    }
}
