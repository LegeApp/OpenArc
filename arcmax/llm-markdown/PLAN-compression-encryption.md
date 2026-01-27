# Implementation Plan: FreeARC Encryption & Compression for arcmax

## Overview

Add encryption and compression capabilities to the arcmax Rust FreeARC port, enabling full archive creation with cascading encryption/compression as used by FreeARC.

## Current State Analysis

### Already Implemented (Decryption/Decompression)
- **crypto.rs** (878 lines): Full decryption for Blowfish, AES, CTR mode
- **crypto.rs**: `encrypt()` methods already exist on BlowfishCipher, AesCipher
- **crypto.rs**: `CascadedDecryptor::encrypt()` already exists for cascaded encryption
- **crypto.rs**: PBKDF2-HMAC-SHA512 key derivation complete
- **crypto.rs**: `EncryptionInfo` parsing complete
- **Codecs**: Decompression for LZMA, LZMA2, PPMd, Zstd
- **free_arc.rs**: Archive reading and extraction
- **Twofish/Serpent**: Stubs exist but not fully implemented

### Missing (Encryption/Compression)
- EncryptionGenerator to create method strings with random IV/salt
- Compression codec implementations (need lzma-rs encoder feature)
- Varint encoding (only decode_varint_from_slice exists in free_arc.rs:1644)
- Archive writer (FreeArcWriter) for creating archives
- CLI integration for Create command

## Implementation Plan

### Phase 1: Core Compression Support

**File: `src/codecs/mod.rs`**
- Add compression function exports alongside existing decompression

**File: `src/codecs/lzma2.rs`** (New compression functions)
```rust
// Add LZMA compression using lzma-rs crate
pub fn compress_lzma(data: &[u8], dict_size: u32, lc: u32, lp: u32, pb: u32) -> Result<Vec<u8>>
```

**File: `src/codecs/zstd.rs`** (New compression functions)
```rust
// Add Zstd compression using zstd crate
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>>
```

**File: `src/codecs/ppmd.rs`** (New compression functions)
```rust
// Add PPMII compression encoder
pub fn compress_ppmii(data: &[u8], max_order: usize, mem_size: usize) -> Result<Vec<u8>>
```

### Phase 2: Encryption Generation Module

**File: `src/core/crypto.rs`** - Add encryption generation (~100 lines):

The existing cipher `encrypt()` methods are already implemented. We need to add:
1. Random IV/salt generation
2. Key derivation with check code generation
3. Method string formatting for archive storage

```rust
/// Hex encoding helper (inverse of hex_decode)
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate encryption parameters for archive creation
pub struct EncryptionGenerator {
    algorithm: CipherAlgorithm,
    key_bits: usize,  // 128, 192, 256, or 448 for blowfish
    iterations: u32,
}

impl EncryptionGenerator {
    pub fn blowfish_448() -> Self { /* ... */ }
    pub fn aes_256() -> Self { /* ... */ }

    fn iv_size(&self) -> usize {
        match self.algorithm {
            CipherAlgorithm::Blowfish => 8,  // 64-bit block
            _ => 16,  // 128-bit block for AES/Twofish/Serpent
        }
    }

    /// Generate encryption setup for archive creation
    /// Returns: (archive_method_string, CascadedDecryptor_for_encryption)
    pub fn generate(&self, password: &str) -> Result<(String, CascadedDecryptor)> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();

        let mut iv = vec![0u8; self.iv_size()];
        let mut salt = vec![0u8; self.key_bits / 8];
        rng.fill_bytes(&mut iv);
        rng.fill_bytes(&mut salt);

        let check_code_size = 2;  // FreeARC default
        let key_size = self.key_bits / 8;

        // Derive key + check_code
        let mut derived = vec![0u8; key_size + check_code_size];
        pbkdf2_hmac::<Sha512>(password.as_bytes(), &salt, self.iterations, &mut derived);

        let check_code = &derived[key_size..];

        // Format: algorithm-bits/ctr:nITER:s<salt>:c<code>:i<iv>:f
        let method_string = format!(
            "{}-{}/ctr:n{}:s{}:c{}:i{}:f",
            self.algorithm_name(),
            self.key_bits,
            self.iterations,
            hex_encode(&salt),
            hex_encode(check_code),
            hex_encode(&iv)
        );

        // Create decryptor (works for encryption too in CTR mode)
        let enc_info = EncryptionInfo::from_method_string(&method_string, None)?;
        let encryptor = CascadedDecryptor::new(&enc_info, password)?;

        Ok((method_string, encryptor))
    }
}

/// Create encryptor for archive creation
pub fn create_encryptor(encryption_spec: &str, password: &str) -> Result<(String, CascadedDecryptor)> {
    let generator = match encryption_spec.to_lowercase().as_str() {
        "blowfish" | "blowfish-448" => EncryptionGenerator::blowfish_448(),
        "aes" | "aes-256" => EncryptionGenerator::aes_256(),
        "aes-128" => EncryptionGenerator::aes_128(),
        _ => return Err(anyhow!("Unknown encryption: {}", encryption_spec)),
    };
    generator.generate(password)
}
```

### Phase 3: Varint Encoding

**File: `src/core/varint.rs`** (New file)
```rust
/// FreeARC variable-length integer encoding
pub fn encode_varint(value: u64) -> Vec<u8>

/// Decode varint (move from free_arc.rs)
pub fn decode_varint(data: &[u8]) -> Result<(u64, usize)>
```

### Phase 4: Archive Writer Module

**File: `src/formats/free_arc_writer.rs`** (New file)

```rust
pub struct FreeArcWriter<W: Write + Seek> {
    writer: W,
    password: Option<String>,
    compression_method: String,
    encryption_method: Option<String>,
    blocks: Vec<WrittenBlock>,
    files: Vec<FileEntry>,
}

impl FreeArcWriter {
    /// Create new archive writer
    pub fn new(writer: W, options: ArchiveOptions) -> Self

    /// Add a file to the archive
    pub fn add_file(&mut self, path: &Path, name: &str) -> Result<()>

    /// Add data directly to archive
    pub fn add_data(&mut self, name: &str, data: &[u8]) -> Result<()>

    /// Write a data block (compress + encrypt)
    fn write_data_block(&mut self, data: &[u8]) -> Result<WrittenBlock>

    /// Write directory block
    fn write_directory_block(&mut self) -> Result<()>

    /// Write footer block and finalize
    pub fn finish(mut self) -> Result<()>
}

struct WrittenBlock {
    block_type: u8,
    compressor: String,
    encryption: String,
    position: u64,
    original_size: u64,
    compressed_size: u64,
    crc: u32,
}
```

### Phase 5: Block Format Implementation

**Directory Block Format** (matching FreeARC):
```
[num_data_blocks: varint]
[files_per_block[]: varints]
[compressors[]: null-terminated strings]
[offsets[]: varints]  // relative to directory position
[comp_sizes[]: varints]
[num_dirs: varint]
[dirs[]: null-terminated strings]
[file_names[]: null-terminated strings]  // NOT count-prefixed
[dir_numbers[]: varints]
[file_sizes[]: varints]
[mod_times[]: 4-byte little-endian]
[is_dirs[]: 1-byte flags]
[crcs[]: 4-byte little-endian]
```

**Footer Block Format**:
```
[num_control_blocks: varint]
For each block:
  [block_type: varint]
  [compressor: null-terminated string]
  [offset_from_footer: varint]
  [original_size: varint]
  [compressed_size: varint]
  [crc: 4 bytes little-endian]
[locked: varint (0/1)]
[comment: null-terminated string]
```

**Block Descriptor Format**:
```
[signature: "ArC\x01" - 4 bytes]
[block_type: varint]
[compressor+encryption: null-terminated string]
[original_size: varint]
[compressed_size: varint]
[crc: 4 bytes little-endian]
```

### Phase 6: CLI Integration

**File: `src/main.rs`** - Update Create command:

```rust
Command::Create { inputs, output, method, threads, password, encryption } => {
    let options = ArchiveOptions {
        compression_method: method,
        encryption_method: encryption,
        password,
        threads,
    };

    let file = File::create(&output)?;
    let mut writer = FreeArcWriter::new(file, options)?;

    for input in inputs {
        writer.add_file(&input, input.file_name().unwrap())?;
    }

    writer.finish()?;
}
```

## File Changes Summary

| File | Action | Lines Est. | Description |
|------|--------|------------|-------------|
| `src/core/mod.rs` | Modify | +2 | Add varint module export |
| `src/core/varint.rs` | Create | ~80 | Varint encode + move decode from free_arc.rs |
| `src/core/crypto.rs` | Modify | +100 | Add EncryptionGenerator, hex_encode, create_encryptor |
| `src/codecs/mod.rs` | Modify | +10 | Export compression functions |
| `src/codecs/lzma2.rs` | Modify | +50 | Add compress_lzma using lzma-rs encoder |
| `src/codecs/zstd.rs` | Modify | +30 | Add compress_zstd (zstd crate already supports encoding) |
| `src/formats/mod.rs` | Modify | +2 | Add free_arc_writer module |
| `src/formats/free_arc_writer.rs` | Create | ~400 | Archive writer with FreeARC format |
| `src/main.rs` | Modify | +50 | Update Create command for compression/encryption |
| `Cargo.toml` | Modify | +2 | Add rand, update lzma-rs features |

**Total estimated new code: ~720 lines**

## Implementation Order

1. **Varint encoding** - Foundation for all block writing
2. **Compression codecs** - LZMA first (most common), then Zstd
3. **EncryptionGenerator** - Extend existing crypto.rs
4. **FreeArcWriter** - Core archive creation
5. **CLI integration** - Wire everything together
6. **Testing** - Create archives and verify with FreeARC GUI/unarc

## Verification Plan

1. **Unit tests**: Varint round-trip, compression round-trip, encryption round-trip
2. **Integration test**: Create archive with arcmax, extract with original FreeARC
3. **Compatibility test**: Create archives with different compression/encryption combos:
   - `arcmax create -m lzma test.arc files/`
   - `arcmax create -m lzma --encryption blowfish-448 -p password test.arc files/`
   - `arcmax create -m lzma --encryption aes-256 -p password test.arc files/`
4. **Extract verification**: Use original FreeARC to extract arcmax-created archives

## Dependencies to Add (Cargo.toml)

```toml
[dependencies]
# Current: lzma-rs = { version = "0.3", features = ["raw_decoder"] }
# Change to: add raw_encoder for LZMA compression
lzma-rs = { version = "0.3", features = ["raw_decoder", "raw_encoder"] }

# Add rand for random IV/salt generation
rand = "0.8"
```

Note: `zstd = "0.13"` already supports encoding - no changes needed for Zstd compression.

## Cascading Encryption Format

FreeARC cascading format: `compression+encryption`

Example: `lzma:96m:normal:bt4:32+blowfish-448/ctr:n1000:s<salt>:c<code>:i<iv>`

For archive creation:
1. Compress data with LZMA
2. Encrypt compressed data with Blowfish
3. Write encrypted data to block
4. Store method string in block descriptor

## Notes

- Encryption uses CTR mode (stream cipher) - encrypt() is same as decrypt() in CTR
- Salt and IV must be randomly generated per block
- Check code allows quick password verification without decryption
- Block offsets are stored relative to directory/footer position (not absolute)
