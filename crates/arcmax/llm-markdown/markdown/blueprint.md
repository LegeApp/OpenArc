<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# Write a longer blueprint file that i can paste in the project folder as a markdown fiel for models to review as a roadmap.

```markdown
# arcmax - Personal High-Compression Archiver
## Project Roadmap and Technical Blueprint

**Status**: MVP - FreeARC extraction, LZMA2 decompression  
**Goal**: Multithreaded, high-ratio personal archiving with format migration from legacy FreeARC

---

## 🎯 Project Goals

1. **Extract FreeARC archives** reliably (your 12GB backups)
2. **Max compression ratios** with ZPAQ/7z/LZMA2
3. **Multithreaded extraction** for solid blocks
4. **Modern CLI** with format auto-detection
5. **Migration path** from FreeARC → 7z/ZPAQ/tar.zst

---

## 📋 Supported Formats (Phased)

| Phase | Read | Write | Priority |
|-------|------|-------|----------|
| **1 (MVP)** | FreeARC (ARC), ZIP, tar | ZIP | 🔴 Critical |
| **2** | 7z, tar.{zst,xz} | 7z, tar.zst | 🟡 High |
| **3** | ZPAQ | ZPAQ | 🟢 Nice |

**Codec backends**: LZMA2 (xz2 crate), zstd, ring (crypto)

---

## 🏗️ Architecture

```

arcmax
├── main.rs (CLI parsing, dispatch)
├── src/core/
│   ├── archive.rs (trait for ArchiveReader/Writer)
│   ├── format.rs (magic detection, format dispatch)
│   └── crypto.rs (AES/Blowfish/PBKDF wrappers)
├── src/formats/
│   ├── free_arc.rs (your MVP target)
│   ├── seven_z.rs
│   ├── zpaq.rs
│   └── zip.rs
├── src/codecs/
│   ├── lzma2.rs
│   ├── zstd.rs
│   └── crc32.rs
└── src/cli/ (subcommands)

```

**Key traits**:
```rust
trait ArchiveReader {
    fn list(&mut self) -> Result<Vec<FileEntry>>;
    fn extract(&mut self, entry: &FileEntry, writer: &mut Sink) -> Result<()>;
}

trait ArchiveWriter {
    fn add_file(&mut self, path: &Path, reader: &mut Source) -> Result<()>;
    fn finalize(&mut self) -> Result<()>;
}
```


---

## 🔍 FreeARC Format Specification (Primary Target)

**Signature**: `ArC\x01` (first 4 bytes) [web:16][web:19][web:29]

**Structure**: HEADER → DATA BLOCKS → DIRECTORY → FOOTER(S)

### 1. Local Descriptor (before every block)

```
Block type (1 byte: 0=data, 1+=control)
Compressed size (varint 1-9 bytes)
Original size (varint)
Compression method (UTF8Z string)
Encryption method (UTF8Z string, empty=none)
CRC32 (4 bytes)
```


### 2. Blocks in order

- **HEADER**: signature + version (uncompressed)
- **DATA BLOCKS**: solid compressed data
- **DIRECTORY BLOCK**:
    - List of solid blocks (num_files, method, offset, sizes)
    - "locked?" flag
    - recovery settings
    - commentary
- **FOOTER**: found by scanning last 4KB for signature, points to directories

**Numbers**: variable 1-9 byte (7z-style)
**Strings**: UTF8 + NUL terminator
**CRC**: PKZIP CRC32 [web:19]

**Encryption**: per-block AES/Blowfish/Twofish/Serpent, password→key via custom PBKDF

---

## 🚀 Development Roadmap

### Phase 1: FreeARC Reader (1-2 weeks)

```
[X] CLI scaffolding (done)
[ ] Magic detection: read first 4 bytes → dispatch FreeARC
[ ] Parse footer (scan last 4096 bytes for signature)
[ ] Parse directory blocks → list files
[ ] Parse local descriptors → decompress solid blocks
[ ] LZMA2 decompression (xz2 crate)
[ ] Password handling + decryption (ring crate)
[ ] Multithread independent solid blocks
[ ] Extract single file end-to-end
```

**Test plan**:

- Hex editor dumps from your 12GB archive
- Small test archives (stored, lzma2, encrypted)
- Roundtrip verification


### Phase 2: 7z + Mainstream (2-3 weeks)

```
[ ] 7z signature detection (7z\xBC\xAF\x27\x1C)
[ ] LZMA2 solid block extraction
[ ] ZIP + tar.{zst,xz} support
[ ] 7z creation (LZMA2 solid)
[ ] tar.zst creation (fast backup mode)
```


### Phase 3: ZPAQ + Polish (3-4 weeks)

```
[ ] ZPAQ journaling format
[ ] Context-mixing models
[ ] Benchmark suite vs 7z/FreeArc
[ ] Encryption write path
[ ] Recovery records
```


---

## ⚙️ Technical Decisions

| Area | Choice | Rationale |
| :-- | :-- | :-- |
| **Language** | Rust | Memory safety for binary parsing/crypto |
| **Async** | Tokio | Parallel I/O + solid block extraction |
| **Crypto** | ring | Audited AES/Blowfish implementations |
| **CLI** | clap | Subcommands, auto-help |
| **Codecs** | xz2/zstd crates | Bind mature liblzma/libzstd |
| **Solid blocks** | rayon/tokio | Parallel decompress independent blocks |

**Threading model**:

```
Archive → SolidBlock[] → File[] 
             ↓ parallel
     Decompress → Decrypt → Output
```


---

## 📊 Performance Targets

| Operation | Target |
| :-- | :-- |
| List 12GB FreeARC | <10s |
| Extract 12GB FreeARC (16 cores) | <30min (vs PeaZip 6+hrs) |
| 7z ultra creation | Competitive with 7-Zip |
| tar.zst backup | 2-5x faster than tar.xz |


---

## 🧪 Testing Strategy

1. **Property tests**: roundtrip stored files
2. **Fuzzed archives**: malformed headers/blocks
3. **Benchmark suite**: Silesia corpus + your real backups
4. **FreeARC corpus**: GitHub issues/samples

---

## 📚 References

- FreeARC format: [GitHub spec][web:19], [HaskellWiki][web:16]
- ZPAQ spec: [Level 2 format][web:78]
- 7z spec: [FileFormat docs][web:79]
- LZMA2: xz2 crate docs
- Crypto: ring crate examples

**Next LLM prompt**: "Implement FreeARC footer parsing from this spec, with varint decoding and CRC validation"

```

Save this as `ROADMAP.md` in your project root. It gives LLMs:
- **Clear milestones** with checkboxes
- **Precise format specs** with citations
- **Architecture diagram** and traits
- **Test strategy** and performance goals
- **Next concrete task** for iterative development
<span style="display:none">[^1][^10][^11][^12][^13][^14][^15][^2][^3][^4][^5][^6][^7][^8][^9]</span>

<div align="center">⁂</div>

[^1]: https://www.haskell.org/haskellwiki/FreeArc/Universal_Archive_Format
[^2]: https://github.com/Bulat-Ziganshin/FA/blob/master/FreeArc-archive-format.md
[^3]: https://freearc.sourceforge.net/FreeArc036-eng.htm
[^4]: http://justsolve.archiveteam.org/wiki/ARC_(FreeArc)
[^5]: https://github.com/Bulat-Ziganshin/FA/blob/master/How-to-improve-the-archive-format.md
[^6]: https://mattmahoney.net/dc/zpaq201.pdf
[^7]: https://docs.fileformat.com/compression/7z/
[^8]: https://peazip.github.io/arc-files-utility.html
[^9]: https://en.wikipedia.org/wiki/ZPAQ
[^10]: https://en.wikipedia.org/wiki/7z
[^11]: https://en.wikipedia.org/wiki/ARC_(file_format)
[^12]: https://peazip.github.io/paq-file-format.html
[^13]: https://peazip.github.io/7z-file-format.html
[^14]: https://freearc.sourceforge.net/FreeArc040-eng.htm
[^15]: https://manpages.ubuntu.com/manpages/xenial/man1/zpaq.1.html```

