# FreeARC Specification Implementation Status

This document describes what's implemented, what's missing, and what needs to be done to complete FreeARC cascading compression support in arcmax.

## Current Implementation Status

### Compression (Archive Creation) - NEW

| Method | Status | Notes |
|--------|--------|-------|
| LZMA | **Working** | Uses lzma-rs crate |
| Zstd | **Working** | Uses zstd crate |
| Store | **Working** | Pass-through |
| Encryption (Blowfish) | **Working** | Full CTR mode support |
| Encryption (AES) | **Working** | 128/192/256-bit support |

### Decompression (Archive Extraction)

| Method | Status | Notes |
|--------|--------|-------|
| LZMA | **Working** | Custom headerless decoder for FreeARC format |
| LZMA2 | **Working** | Standard format via xz2 crate |
| PPMd | **Working** | FreeARC PPMII variant with carryless rangecoder |
| Store | **Working** | Pass-through |
| Decryption (Blowfish/AES) | **Working** | Full cascading support |

### Preprocessing Filters - Decompression

| Filter | Code Exists | Integrated | Notes |
|--------|-------------|------------|-------|
| Delta | Yes | **NO** | `codecs/dict.rs` - reverse delta transform |
| E8E9 (EXE) | Yes | **NO** | `codecs/dict.rs` - x86 call/jmp reverse |
| Intel | Yes | **NO** | `codecs/dict.rs` - x86 instruction reverse |
| LZP | Yes | **NO** | `codecs/lzp.rs` - basic implementation exists |
| Dict | Yes | **NO** | `codecs/dict.rs` - parsing exists |

---

## What's Missing for Full ARC Spec

### Priority 1: Cascading Pipeline Integration (CRITICAL)

**Problem**: Filter codecs exist but are NOT connected to the decompression pipeline.

**Location**: `src/formats/free_arc.rs` lines 2608-2616

```rust
// Current code - filters pass through without processing:
"lzp" => {
    eprintln!("LZP decompression - passing through");
    current_data = current_data;  // NO-OP!
}
"dict" => {
    eprintln!("DICT reverse processing - passing through");
    current_data = current_data;  // NO-OP!
}
```

**Required Fix**:
1. Parse filter parameters from method string (e.g., `dict:p:64m:85%`, `lzp:64m:24:h20`)
2. Call appropriate reverse functions from `codecs/dict.rs` and `codecs/lzp.rs`
3. Process in correct order (compression reverses left-to-right)

**Files to modify**:
- `src/formats/free_arc.rs` - integrate filter calls in `decompress_data_static()`

---

### Priority 2: Missing Decompression Algorithms

These algorithms are used in FreeARC but have NO implementation:

#### Tornado (`tor`) - HIGH PRIORITY
- **Usage**: `1binary = tor:4`, `2binary = tor:6`, `tor:7:96m:h64m`
- **Description**: Fast LZ-based compression, common in fast modes
- **Source**: `freearc/clibs/Compression/Tornado/`
- **Difficulty**: Medium - well-documented LZ variant

#### GRZip (`grzip`) - HIGH PRIORITY
- **Usage**: `grzip:m4:8m:32:h15`, text modes like `3t`
- **Description**: BWT-based compression optimized for text
- **Source**: `freearc/clibs/Compression/GRZip/`
- **Difficulty**: Medium-Hard - BWT implementation required

#### REP (`rep`) - MEDIUM PRIORITY
- **Usage**: `rep:96m:256:c256`, `#rep+exe+#xb`
- **Description**: Long-range match finder / repetition detection
- **Source**: `freearc/clibs/Compression/REP/`
- **Difficulty**: Medium - hash-based duplicate finder

#### SREP (`srep`) - LOW PRIORITY
- **Usage**: Secondary repetition compression
- **Description**: Works with REP for additional compression
- **Source**: `freearc/clibs/Compression/SREP/`
- **Difficulty**: Medium

#### LZ4 (`lz4`) - LOW PRIORITY
- **Usage**: `lz4:hc`, `4x4:lz4`
- **Description**: Very fast compression
- **Rust crate available**: `lz4` or `lz4_flex`
- **Difficulty**: Easy - use existing crate

---

### Priority 3: Preprocessing Filters

#### MM Filter (Multimedia) - MEDIUM PRIORITY
- **Usage**: `mm + grzip:m1:l2048:a`, BMP/WAV optimization
- **Description**: Reorders bytes for better compression of multimedia
- **Source**: `freearc/clibs/Compression/MM/`
- **Difficulty**: Medium

#### 4x4 Block Processor - LOW PRIORITY
- **Usage**: `4x4:lz4`, `4x4:i0:lzma:32mb:max`
- **Description**: Multi-threaded block processing wrapper
- **Source**: `freearc/clibs/Compression/4x4/`
- **Difficulty**: Medium - threading coordination

#### DisPack/EXE Filter Enhancement - LOW PRIORITY
- **Usage**: `dispack`, `dispack070`, `exe`
- **Description**: Advanced executable preprocessing
- **Note**: Basic E8E9 exists, but full dispack is more complex
- **Source**: `freearc/clibs/Compression/DisPack/`

---

### Priority 4: Specialized Codecs (Optional)

These are for specific file types and less commonly used:

| Codec | Usage | Notes |
|-------|-------|-------|
| TTA | Audio compression | Lossless audio codec |
| JPG | JPEG recompression | Specialized JPEG optimizer |
| ECM | CD image compression | Error correction removal |
| PRECOMP | Precompressed detection | Handles already-compressed data |
| BMF | Bitmap-specific | Specialized for BMP files |

---

## Cascading Method Format

Understanding the method string is critical for correct implementation:

```
method_string = method1 [+ method2 [+ method3 [+ encryption]]]
```

**Examples**:
```
lzma:1mb:normal:bt4:32
dict:p:64m:85% + lzp:64m:24:h20 + ppmd:8:96m
rep:96m + exe + lzma:96m:normal
lzma:1mb:normal + blowfish-448/ctr:n1000:s...:c...:i...:f
```

**Processing Order**:
- **Compression**: Left → Right (dict first, then lzp, then ppmd)
- **Decompression**: Right → Left (ppmd first, then lzp-reverse, then dict-reverse)

---

## Implementation Roadmap

### Phase 1: Complete Filter Integration (Estimated: 1-2 days)
1. Connect `dict.rs` filters to `free_arc.rs` decompression pipeline
2. Connect `lzp.rs` filter to decompression pipeline
3. Parse filter parameters from method strings
4. Test with real FreeARC archives using these filters

### Phase 2: Core Algorithm Implementations (Estimated: 3-5 days)
1. Implement Tornado decompression
2. Implement GRZip decompression (BWT-based)
3. Add LZ4 decompression via existing crate

### Phase 3: Additional Filters (Estimated: 2-3 days)
1. Implement REP decompression
2. Implement MM filter reverse
3. Enhance EXE/DisPack filter

### Phase 4: Compression Equivalents (Estimated: 3-5 days)
1. Add Tornado compression
2. Add GRZip compression
3. Add preprocessing filters for compression path

---

## Testing Strategy

### Test Archives Needed
Create test archives with FreeARC GUI using:
1. Simple LZMA only: `arc a test1.arc -m=lzma files/`
2. LZMA + encryption: `arc a test2.arc -m=lzma -ae=blowfish -p=test files/`
3. Cascading: `arc a test3.arc -m=dict+lzp+ppmd files/`
4. Tornado mode: `arc a test4.arc -m=tor:4 files/`
5. Text mode: `arc a test5.arc -m=3t files/`

### Verification
1. Extract with arcmax
2. Compare file contents and sizes
3. Verify CRCs match

---

## Files Reference

| File | Purpose |
|------|---------|
| `src/formats/free_arc.rs` | Main archive reader, decompression dispatch |
| `src/formats/free_arc_writer.rs` | NEW: Archive creation |
| `src/codecs/lzma2.rs` | LZMA decompression + compression |
| `src/codecs/ppmd.rs` | PPMd decompression |
| `src/codecs/zstd.rs` | Zstd compression/decompression |
| `src/codecs/dict.rs` | Delta/E8E9/Intel filters |
| `src/codecs/lzp.rs` | LZP filter |
| `src/core/crypto.rs` | Encryption/decryption |
| `src/core/varint.rs` | Variable-length integer encoding |

---

## Summary

**What Works Now**:
- Creating archives with LZMA, Zstd, or Store compression
- Creating archives with Blowfish or AES encryption
- Extracting simple LZMA/LZMA2/PPMd archives
- Extracting encrypted archives (Blowfish/AES)

**Critical Gap**:
- Filter integration (LZP, Dict, Delta) - code exists but not wired up

**Major Missing Algorithms**:
- Tornado (common in fast modes)
- GRZip (common in text modes)
- REP (common in high-compression modes)

**For Most ARC Files to Work**:
Completing Phase 1 (filter integration) will enable extraction of most real-world FreeARC archives that use the `dict+lzp+ppmd` or `delta+lzma` patterns.
