// YCbCr 4:2:0 → RGB bilinear resize compute shader
//
// Reads planar YCbCr data from a ByteAddressBuffer (Y, Cb, Cr planes concatenated),
// performs bilinear-filtered resize, BT.601 YCbCr→RGB color conversion,
// and writes RGBA8 pixels to the atlas buffer at the designated tile position.
//
// Thread group: [16, 16, 1]  —  one thread per output tile pixel.

struct YCbCrResizeParams {
    uint y_width;        //  0: Y plane width (full resolution)
    uint y_height;       //  4: Y plane height
    uint uv_width;       //  8: Cb/Cr plane width (y_width / 2 for 4:2:0)
    uint uv_height;      // 12: Cb/Cr plane height (y_height / 2)
    uint tile_x;         // 16: tile pixel-X offset in atlas
    uint tile_y;         // 20: tile pixel-Y offset in atlas
    uint tile_size;      // 24: tile edge length (e.g. 256)
    uint atlas_stride;   // 28: atlas width in pixels (e.g. 4096)
    uint y_offset;       // 32: byte offset to Y  plane in srcBuffer
    uint cb_offset;      // 36: byte offset to Cb plane
    uint cr_offset;      // 40: byte offset to Cr plane
    uint _pad;           // 44: pad to 48 bytes (3×16-byte alignment)
};

cbuffer Constants : register(b0) {
    YCbCrResizeParams params;
};

ByteAddressBuffer   srcBuffer : register(t0);   // [Y | Cb | Cr] concatenated
RWByteAddressBuffer dstBuffer : register(u0);   // atlas RGBA8 buffer

// ---------------------------------------------------------------------------
// Byte-level sampling from the ByteAddressBuffer.
// ByteAddressBuffer.Load() requires 4-byte-aligned addresses and returns a
// uint32.  We extract individual bytes by aligning down and shifting.
// ---------------------------------------------------------------------------
float sample_byte(uint plane_offset, uint x, uint y, uint stride) {
    uint byte_addr    = plane_offset + y * stride + x;
    uint aligned_addr = byte_addr & ~3u;
    uint shift        = (byte_addr & 3u) * 8u;
    uint dword_val    = srcBuffer.Load(aligned_addr);
    return (float)((dword_val >> shift) & 0xFFu) / 255.0f;
}

// ---------------------------------------------------------------------------
// Bilinear-filtered sample of a single 8-bit plane.
// ---------------------------------------------------------------------------
float bilinear_sample_plane(
    uint plane_offset,
    uint plane_width,
    uint plane_height,
    float coord_x,
    float coord_y)
{
    int x0 = (int)floor(coord_x);
    int y0 = (int)floor(coord_y);
    int x1 = x0 + 1;
    int y1 = y0 + 1;

    // Clamp to plane bounds
    x0 = clamp(x0, 0, (int)plane_width  - 1);
    x1 = clamp(x1, 0, (int)plane_width  - 1);
    y0 = clamp(y0, 0, (int)plane_height - 1);
    y1 = clamp(y1, 0, (int)plane_height - 1);

    float fx = coord_x - floor(coord_x);
    float fy = coord_y - floor(coord_y);

    float s00 = sample_byte(plane_offset, (uint)x0, (uint)y0, plane_width);
    float s10 = sample_byte(plane_offset, (uint)x1, (uint)y0, plane_width);
    float s01 = sample_byte(plane_offset, (uint)x0, (uint)y1, plane_width);
    float s11 = sample_byte(plane_offset, (uint)x1, (uint)y1, plane_width);

    return lerp(lerp(s00, s10, fx), lerp(s01, s11, fx), fy);
}

// ---------------------------------------------------------------------------
// Main: each thread writes one RGBA8 pixel into the atlas tile.
// ---------------------------------------------------------------------------
[numthreads(16, 16, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x >= params.tile_size || id.y >= params.tile_size)
        return;

    // Scale factors: source plane pixels per output tile pixel
    float y_scale_x  = (float)params.y_width  / (float)params.tile_size;
    float y_scale_y  = (float)params.y_height / (float)params.tile_size;
    float uv_scale_x = (float)params.uv_width / (float)params.tile_size;
    float uv_scale_y = (float)params.uv_height / (float)params.tile_size;

    // Source coordinates (center-aligned mapping)
    float y_x  = (id.x + 0.5f) * y_scale_x  - 0.5f;
    float y_y  = (id.y + 0.5f) * y_scale_y  - 0.5f;
    float uv_x = (id.x + 0.5f) * uv_scale_x - 0.5f;
    float uv_y = (id.y + 0.5f) * uv_scale_y - 0.5f;

    // Bilinear sample each plane
    float Y  = bilinear_sample_plane(params.y_offset,  params.y_width,  params.y_height,  y_x,  y_y);
    float Cb = bilinear_sample_plane(params.cb_offset,  params.uv_width, params.uv_height, uv_x, uv_y);
    float Cr = bilinear_sample_plane(params.cr_offset,  params.uv_width, params.uv_height, uv_x, uv_y);

    // BT.601 YCbCr → RGB  (values scaled to 0–255 range for conversion)
    float Y_val  = Y  * 255.0f;
    float Cb_val = Cb * 255.0f - 128.0f;
    float Cr_val = Cr * 255.0f - 128.0f;

    float R = Y_val + 1.402f    * Cr_val;
    float G = Y_val - 0.344136f * Cb_val - 0.714136f * Cr_val;
    float B = Y_val + 1.772f    * Cb_val;

    // Pack RGBA8 (little-endian: R in lowest byte)
    uint r = (uint)clamp(R, 0.0f, 255.0f);
    uint g = (uint)clamp(G, 0.0f, 255.0f);
    uint b = (uint)clamp(B, 0.0f, 255.0f);
    uint packed = r | (g << 8u) | (b << 16u) | (0xFFu << 24u);

    // Write to atlas at tile position
    uint atlas_x = params.tile_x + id.x;
    uint atlas_y = params.tile_y + id.y;
    uint atlas_addr = (atlas_y * params.atlas_stride + atlas_x) * 4u;
    dstBuffer.Store(atlas_addr, packed);
}
