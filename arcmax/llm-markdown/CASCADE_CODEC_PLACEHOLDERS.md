# Cascade Codec Implementation Placeholders

This document details the placeholder code areas in the newly implemented codecs that require further development for full functionality.

## 1. Tornado Codec (`src/codecs/tornado.rs`)

### Major Placeholders:

#### A. BWT + WFC Decompression
- **Function**: `decompress_bwt_wfc()`
- **Line**: ~180-185
- **Issue**: Currently returns placeholder data instead of actual BWT + WFC decoding
- **Requires**: Implementation of Burrows-Wheeler Transform + Weighted Frequency Coding inverse transform

#### B. BWT + MTF Decompression  
- **Function**: `decompress_bwt_mtf()`
- **Line**: ~205-210
- **Issue**: Returns placeholder data instead of actual BWT + MTF decoding
- **Requires**: Implementation of Burrows-Wheeler Transform + Move To Front inverse transform

#### C. ST4 + WFC Decompression
- **Function**: `decompress_st4_wfc()`
- **Line**: ~215-220
- **Issue**: Returns placeholder data instead of actual ST4 + WFC decoding
- **Requires**: Implementation of Simple Transform 4 + Weighted Frequency Coding inverse transform

#### D. ST4 + MTF Decompression
- **Function**: `decompress_st4_mtf()`
- **Line**: ~225-230
- **Issue**: Returns placeholder data instead of actual ST4 + MTF decoding
- **Requires**: Implementation of Simple Transform 4 + Move To Front inverse transform

#### E. Distance Code Handling
- **Function**: `get_distance_base()` and `get_distance_extra_bits()`
- **Lines**: ~250-280
- **Issue**: Simplified implementation that doesn't match FreeARC's complex distance coding
- **Requires**: Accurate implementation matching FreeARC's distance coder logic

## 2. GRZip Codec (`src/codecs/grzip.rs`)

### Major Placeholders:

#### A. BWT + WFC Inverse Transform
- **Function**: `decompress_bwt_wfc()`
- **Line**: ~200-205
- **Issue**: Completely placeholder implementation returning sliced input data
- **Requires**: Full BWT inverse + WFC arithmetic decoder implementation

#### B. BWT + MTF Inverse Transform
- **Function**: `decompress_bwt_mtf()`
- **Line**: ~225-230
- **Issue**: Completely placeholder implementation returning sliced input data
- **Requires**: Full BWT inverse + MTF inverse + arithmetic decoder implementation

#### C. ST4 + WFC Inverse Transform
- **Function**: `decompress_st4_wfc()`
- **Line**: ~235-240
- **Issue**: Completely placeholder implementation returning sliced input data
- **Requires**: Full ST4 inverse + WFC arithmetic decoder implementation

#### D. ST4 + MTF Inverse Transform
- **Function**: `decompress_st4_mtf()`
- **Line**: ~245-250
- **Issue**: Completely placeholder implementation returning sliced input data
- **Requires**: Full ST4 inverse + MTF inverse + arithmetic decoder implementation

#### E. Recursive Block Handling
- **Function**: `grzip_decompress()` 
- **Line**: ~85-87
- **Issue**: Returns error for recursive blocks (mode -2)
- **Requires**: Implementation of recursive block processing for multi-part archives

#### F. Complex Parameter Parsing
- **Function**: `GrzipParams::from_method_string()`
- **Line**: ~40-60
- **Issue**: Limited parameter parsing support
- **Requires**: Full support for all GRZip parameters like `m4:8m:32:h15`

## 3. LZP Codec (`src/codecs/lzp.rs`) - Existing Issues

### Known Issues (not implemented by me, but relevant):
- **Function**: `apply_lzp_reverse()` 
- **Line**: ~80-140
- **Issue**: Simplified LZP reverse implementation that may not match FreeARC's LZP exactly
- **Requires**: Verification against FreeARC's LZP algorithm

## 4. Dictionary Codec (`src/codecs/dict.rs`) - Existing Issues

### Known Issues (not implemented by me, but relevant):
- **Function**: `apply_complex_dict_transform()`
- **Line**: ~270-285
- **Issue**: Simplified implementation for complex dict methods like "dict:p:64m:85%"
- **Requires**: Full implementation of FreeARC's complex dictionary preprocessing

## 5. Integration Points in FreeARC Parser

### A. Tornado Integration
- **File**: `src/formats/free_arc.rs`
- **Line**: ~2625-2628
- **Issue**: Calls `tornado_decompress()` which has placeholder transforms inside

### B. GRZip Integration
- **File**: `src/formats/free_arc.rs` 
- **Line**: ~2634-2637
- **Issue**: Calls `grzip_decompress()` which has placeholder transforms inside

## Next Steps for Full Implementation

1. **Tornado**: Implement the BWT/ST4 inverse transforms and proper distance coding
2. **GRZip**: Implement full BWT/ST4 inverse transforms, MTF/WFC decoders, and recursive block handling
3. **Parameter Parsing**: Enhance parameter parsing for complex method strings
4. **Testing**: Create test cases with known FreeARC archives using these methods
5. **Verification**: Compare output against FreeARC's reference implementation

The cascading pipeline framework is in place, but these core algorithm implementations need to be completed for full FreeARC compatibility.