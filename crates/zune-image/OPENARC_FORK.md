# OpenArc Fork of zune-jpeg - YCbCr Exposure

Wrapper around upstream `zune-jpeg` 0.5.12 that exposes raw YCbCr planes for optimal JPEG→BPG encoding.

## Purpose

The upstream `zune-jpeg` decoder automatically converts YCbCr→RGB during `decode()`. This fork provides:
- `decode_rgb()` - Standard RGB output (same as upstream)
- **`decode_ycbcr()`** - Raw YCbCr planes WITHOUT RGB conversion

## Why This Matters

For JPEG→BPG encoding:
- ❌ **Bad**: JPEG (YCbCr) → RGB → YCbCr → BPG (lossy conversions)
- ✅ **Good**: JPEG (YCbCr) → **no conversion** → BPG (preserve original)

HEIC already provides YCbCr output via `heic-decoder-rs` - this brings JPEG to the same level.

## Current Status

**Phase 1 - Placeholder Implementation** (CURRENT)
- ✓ Fork compiles successfully
- ❌ decode_ycbcr() uses RGB→YCbCr conversion (placeholder)
- TODO: Intercept YCbCr before upstream's RGB conversion

## Usage

```rust
use zune_image::{decode_jpeg_ycbcr, YCbCrImage};

let jpeg_data = std::fs::read("photo.jpg")?;
let ycbcr = decode_jpeg_ycbcr(&jpeg_data)?;

// BPG encoder can use these planes directly
bpg_encode_from_ycbcr420_planar(
    &ycbcr.y_plane, &ycbcr.cb_plane, &ycbcr.cr_plane,
    ycbcr.width, ycbcr.height
);
```

## Integration Steps

1. Add to openarc workspace:
   ```toml
   # openarc/Cargo.toml
   [workspace]
   members = ["zune-image", ...]
   ```

2. Replace zune-jpeg dependency:
   ```toml
   # openarc/Cargo.toml
   [dependencies]
   zune-image = { path = "zune-image" }
   # Remove: zune-jpeg = "0.5.12"
   ```

3. Update imports:
   ```rust
   // src/image_loader.rs
   use zune_image::JpegDecoder;
   ```

## TODO: True YCbCr Interception

Current placeholder in `decoder.rs`:
```rust
// TODO: Patch upstream zune-jpeg to expose YCbCr directly
let (rgb, width, height) = self.decode_rgb()?;
// Convert back RGB→YCbCr (wasteful!)
```

**Approaches**:
1. Create custom zune-jpeg fork that exposes internal YCbCr buffers
2. Use different JPEG library that supports YCbCr output (jpeg-decoder might)
3. Contribute upstream PR to zune-jpeg for optional YCbCr output

## Dependencies

- `zune-jpeg = "0.5.12"` - Upstream JPEG decoder
- `zune-core = "0.5"` - **Must match zune-jpeg's version** (0.4 causes type conflicts)

## License

MIT OR Apache-2.0 (matches upstream zune-jpeg)
