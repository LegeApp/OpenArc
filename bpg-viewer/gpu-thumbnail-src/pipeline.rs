//! Thumbnail pipeline orchestrator.
//!
//! Ties together the multi-queue context, staging ring, atlas, and compute
//! PSO into a single entry-point: `process_thumbnail(source_id, ycbcr)`.
//!
//! Current (prototype) flow — serial, no copy-queue pipelining:
//!   1. Allocate atlas tile
//!   2. Concatenate YCbCr planes → staging slot (upload buffer, SRV-readable)
//!   3. Upload constants to a small CBV buffer
//!   4. Dispatch compute shader (reads staging SRV, writes atlas UAV)
//!   5. Wait for GPU

use std::ffi::c_void;
use std::ptr;

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::atlas::{ThumbnailAtlas, TileIndex};
use crate::compute::{ComputePipelineState, YCbCrResizeParams};
use crate::dx12_multi_queue::MultiQueueContext;
use crate::error::Result;
use crate::staging_ring::StagingRing;
use crate::{DEFAULT_ATLAS_SIZE, DEFAULT_GPU_TIMEOUT_MS, DEFAULT_TILE_SIZE};

// ─── Input data ─────────────────────────────────────────────────────────────

/// Decoded YCbCr 4:2:0 planar data, ready for GPU upload.
pub struct YCbCrData {
    pub y_plane: Vec<u8>,
    pub cb_plane: Vec<u8>,
    pub cr_plane: Vec<u8>,
    /// Y plane width (full resolution).
    pub width: u32,
    /// Y plane height (full resolution).
    pub height: u32,
}

impl YCbCrData {
    /// Total byte size of all three planes concatenated.
    pub fn total_bytes(&self) -> usize {
        self.y_plane.len() + self.cb_plane.len() + self.cr_plane.len()
    }

    /// Concatenate planes into a single buffer: [Y | Cb | Cr].
    pub fn to_concatenated(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.total_bytes());
        buf.extend_from_slice(&self.y_plane);
        buf.extend_from_slice(&self.cb_plane);
        buf.extend_from_slice(&self.cr_plane);
        buf
    }
}

// ─── Pipeline ───────────────────────────────────────────────────────────────

/// GPU thumbnail pipeline — owns all D3D12 resources.
pub struct ThumbnailPipeline {
    pub ctx: MultiQueueContext,
    pub atlas: ThumbnailAtlas,
    pub staging: StagingRing,
    pso: ComputePipelineState,
    /// Small upload buffer for the per-dispatch constant buffer (256 bytes).
    cbv_buffer: ID3D12Resource,
    cbv_mapped: *mut u8,
    /// GPU-local buffer backing the atlas (RGBA8, atlas_size² × 4).
    atlas_buffer: ID3D12Resource,
    /// Atlas size in bytes.
    atlas_buffer_size: usize,
    /// Persistent readback buffer for single-tile readback (tile_size² × 4 bytes).
    tile_readback_buffer: ID3D12Resource,
    /// Size of a single tile in bytes.
    tile_readback_size: usize,
}

// SAFETY: all interior pointers are GPU-mapped and valid for the pipeline lifetime.
unsafe impl Send for ThumbnailPipeline {}
unsafe impl Sync for ThumbnailPipeline {}

impl ThumbnailPipeline {
    /// Initialise the pipeline with default sizes (4096×4096 atlas, 256×256 tiles).
    pub fn new() -> Result<Self> {
        Self::with_sizes(DEFAULT_ATLAS_SIZE, DEFAULT_TILE_SIZE)
    }

    /// Initialise with custom atlas / tile dimensions.
    pub fn with_sizes(atlas_size: u32, tile_size: u32) -> Result<Self> {
        let ctx = MultiQueueContext::new()?;

        // ── Compute PSO ──
        let shader_bytes = crate::YCBCR_RESIZE_SHADER;
        let pso = ComputePipelineState::new(&ctx.device, shader_bytes)?;

        // ── Atlas metadata + GPU buffer ──
        let atlas = ThumbnailAtlas::new(atlas_size, tile_size);
        let atlas_buffer_size = (atlas_size as usize) * (atlas_size as usize) * 4;
        let atlas_buffer = ctx.create_gpu_buffer(atlas_buffer_size)?;

        // ── Staging ring (triple-buffered) ──
        let staging = StagingRing::new(&ctx, crate::staging_ring::DEFAULT_SLOT_CAPACITY)?;

        // ── CBV upload buffer (256 bytes, persistently mapped) ──
        let cbv_buffer = ctx.create_upload_buffer(256)?;
        let cbv_mapped = unsafe {
            let mut p: *mut c_void = ptr::null_mut();
            cbv_buffer.Map(0, None, Some(&mut p))?;
            p as *mut u8
        };

        // ── Persistent tile readback buffer ──
        let tile_readback_size = (tile_size as usize) * (tile_size as usize) * 4;
        let tile_readback_buffer = ctx.create_readback_buffer(tile_readback_size)?;

        if ctx.verbose {
            let grid = atlas_size / tile_size;
            eprintln!(
                "[Pipeline] Ready: {}×{} atlas, {}×{} tiles ({} slots), staging {:.1} MB × {}",
                atlas_size,
                atlas_size,
                tile_size,
                tile_size,
                grid * grid,
                crate::staging_ring::DEFAULT_SLOT_CAPACITY as f64 / (1024.0 * 1024.0),
                crate::staging_ring::SLOT_COUNT,
            );
        }

        Ok(Self {
            ctx,
            atlas,
            staging,
            pso,
            cbv_buffer,
            cbv_mapped,
            atlas_buffer,
            atlas_buffer_size,
            tile_readback_buffer,
            tile_readback_size,
        })
    }

    // ── Main entry point ────────────────────────────────────────────────

    /// Decode-free path: upload pre-decoded YCbCr data and resize into an atlas tile.
    ///
    /// Returns the [`TileIndex`] where the thumbnail was placed.
    pub fn process_thumbnail(
        &mut self,
        source_id: u64,
        ycbcr: &YCbCrData,
    ) -> Result<TileIndex> {
        // 1. Allocate (or reuse) atlas tile.
        let tile = self.atlas.allocate_tile(source_id)?;
        let (tile_px, tile_py) = self.atlas.tile_pixel_offset(tile);

        // 2. Upload concatenated YCbCr planes to staging.
        let concat = ycbcr.to_concatenated();
        let (slot_idx, staging_buf) = self.staging.upload(
            &concat,
            &mut self.ctx.compute_queue,
        )?;
        // Clone the resource handle for descriptor creation.
        let staging_resource = staging_buf.clone();

        // 3. Build constants.
        let y_offset = 0u32;
        let cb_offset = ycbcr.y_plane.len() as u32;
        let cr_offset = cb_offset + ycbcr.cb_plane.len() as u32;
        let uv_w = (ycbcr.width + 1) / 2;
        let uv_h = (ycbcr.height + 1) / 2;

        let params = YCbCrResizeParams {
            y_width: ycbcr.width,
            y_height: ycbcr.height,
            uv_width: uv_w,
            uv_height: uv_h,
            tile_x: tile_px,
            tile_y: tile_py,
            tile_size: self.atlas.tile_size,
            atlas_stride: self.atlas.atlas_size,
            y_offset,
            cb_offset,
            cr_offset,
            _pad: 0,
        };
        self.upload_constants(&params)?;

        // 4. Dispatch compute and get the fence value.
        let fence_val = self.dispatch_resize(&staging_resource, &params)?;

        // 5. Mark staging slot in-flight with the compute-queue fence value.
        self.staging.mark_in_flight(slot_idx, fence_val);

        Ok(tile)
    }

    // ── Read back atlas data (for verification) ─────────────────────────

    /// Flush all pending GPU operations and wait for completion.
    /// Call this before readback to ensure all thumbnails are processed.
    pub fn flush_gpu(&mut self) -> Result<()> {
        self.ctx.compute_queue.wait_for_gpu(DEFAULT_GPU_TIMEOUT_MS)?;
        Ok(())
    }

    /// Read back all or part of the atlas buffer to CPU memory.
    /// Mainly useful for tests and debugging.
    /// IMPORTANT: Calls flush_gpu() to ensure all pending operations complete.
    pub fn readback_atlas(&mut self) -> Result<Vec<u8>> {
        // Ensure all pending GPU operations complete before readback
        self.flush_gpu()?;

        let readback = self.ctx.create_readback_buffer(self.atlas_buffer_size)?;

        // Record copy on compute queue (compute lists can do copies too).
        self.ctx.compute_queue.reset()?;
        unsafe {
            self.ctx.compute_queue.list.CopyBufferRegion(
                &readback,
                0,
                &self.atlas_buffer,
                0,
                self.atlas_buffer_size as u64,
            );
        }
        self.ctx
            .compute_queue
            .execute_and_wait(DEFAULT_GPU_TIMEOUT_MS)?;

        // Map readback.
        let mut data = vec![0u8; self.atlas_buffer_size];
        unsafe {
            let mut p: *mut c_void = ptr::null_mut();
            readback.Map(0, None, Some(&mut p))?;
            ptr::copy_nonoverlapping(p as *const u8, data.as_mut_ptr(), self.atlas_buffer_size);
            readback.Unmap(0, None);
        }
        Ok(data)
    }

    /// Process a thumbnail AND read it back in one call.
    ///
    /// This avoids the double-flush that happens when process_thumbnail() and
    /// readback_tile() are called separately (each does a GPU sync).
    /// The compute dispatch runs async; readback_tile's execute_and_wait
    /// implicitly waits for the preceding dispatch to finish.
    pub fn process_and_readback_tile(
        &mut self,
        source_id: u64,
        ycbcr: &YCbCrData,
    ) -> Result<(TileIndex, Vec<u8>)> {
        let tile = self.process_thumbnail(source_id, ycbcr)?;
        let data = self.readback_tile(tile)?;
        Ok((tile, data))
    }

    /// Read back a single tile as RGBA8 bytes (tile_size × tile_size × 4).
    ///
    /// OPTIMIZED: Copies only the tile region (~256 KB) from the atlas buffer
    /// instead of the entire atlas (64 MB). Uses a persistent readback buffer
    /// and row-by-row GPU copies to extract the tile from the 2D atlas layout.
    ///
    /// NOTE: Does NOT call flush_gpu() separately — the execute_and_wait()
    /// at the end of this method is sufficient to drain the compute queue
    /// (including any preceding async dispatch from process_thumbnail).
    pub fn readback_tile(&mut self, tile: TileIndex) -> Result<Vec<u8>> {
        // Wait for any in-flight async dispatch (from process_thumbnail) to
        // complete before resetting the command allocator.  This is cheaper
        // than a full flush_gpu() because it only waits for the specific
        // fence value that was signalled, not an incremented one.
        let completed = unsafe { self.ctx.compute_queue.fence.GetCompletedValue() };
        let target = self.ctx.compute_queue.fence_value.saturating_sub(1);
        if completed < target {
            self.ctx.compute_queue.wait_for_value(target, DEFAULT_GPU_TIMEOUT_MS)?;
        }

        let (tx, ty) = self.atlas.tile_pixel_offset(tile);
        let ts = self.atlas.tile_size as usize;
        let atlas_stride = self.atlas.atlas_size as usize;
        let row_bytes = ts * 4; // bytes per tile row

        // Record row-by-row copy commands from atlas buffer to readback buffer.
        // The atlas is a flat 1D buffer with stride = atlas_size * 4 bytes per row.
        self.ctx.compute_queue.reset()?;
        unsafe {
            for row in 0..ts {
                let src_offset = ((ty as usize + row) * atlas_stride + tx as usize) * 4;
                let dst_offset = row * row_bytes;
                self.ctx.compute_queue.list.CopyBufferRegion(
                    &self.tile_readback_buffer,
                    dst_offset as u64,
                    &self.atlas_buffer,
                    src_offset as u64,
                    row_bytes as u64,
                );
            }
        }
        self.ctx
            .compute_queue
            .execute_and_wait(DEFAULT_GPU_TIMEOUT_MS)?;

        // Map and read the tile data.
        let mut data = vec![0u8; self.tile_readback_size];
        unsafe {
            let mut p: *mut c_void = ptr::null_mut();
            self.tile_readback_buffer.Map(0, None, Some(&mut p))?;
            ptr::copy_nonoverlapping(p as *const u8, data.as_mut_ptr(), self.tile_readback_size);
            self.tile_readback_buffer.Unmap(0, None);
        }
        Ok(data)
    }

    // ── Internal ────────────────────────────────────────────────────────

    fn upload_constants(&mut self, params: &YCbCrResizeParams) -> Result<()> {
        let bytes = params.as_bytes();
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.cbv_mapped, bytes.len());
        }
        Ok(())
    }

    fn dispatch_resize(
        &mut self,
        staging_buf: &ID3D12Resource,
        params: &YCbCrResizeParams,
    ) -> Result<u64> {
        let list = &self.ctx.compute_queue.list;

        self.ctx.compute_queue.reset()?;

        unsafe {
            // Bind descriptor heap.
            let heaps = [Some(self.ctx.descriptor_heap.clone())];
            list.SetDescriptorHeaps(&heaps);

            // Bind PSO + root signature.
            list.SetPipelineState(&self.pso.pipeline_state);
            list.SetComputeRootSignature(&self.pso.root_signature);

            // Param 0: CBV (constants).
            list.SetComputeRootConstantBufferView(
                0,
                self.cbv_buffer.GetGPUVirtualAddress(),
            );

            // ── SRV for staging buffer (descriptor slot 0) ──
            let staging_size = params.cr_offset as usize
                + (params.uv_width as usize * params.uv_height as usize);
            let srv_elements = ((staging_size + 3) / 4) as u32; // ceil to dwords

            let srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                    Buffer: D3D12_BUFFER_SRV {
                        FirstElement: 0,
                        NumElements: srv_elements,
                        StructureByteStride: 0,
                        Flags: D3D12_BUFFER_SRV_FLAG_RAW,
                    },
                },
            };
            self.ctx.device.CreateShaderResourceView(
                staging_buf,
                Some(&srv_desc),
                self.ctx.cpu_descriptor(0),
            );

            // ── UAV for atlas buffer (descriptor slot 1) ──
            let uav_elements = (self.atlas_buffer_size / 4) as u32;
            let uav_desc = D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
                Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Buffer: D3D12_BUFFER_UAV {
                        FirstElement: 0,
                        NumElements: uav_elements,
                        StructureByteStride: 0,
                        Flags: D3D12_BUFFER_UAV_FLAG_RAW,
                        CounterOffsetInBytes: 0,
                    },
                },
            };
            self.ctx.device.CreateUnorderedAccessView(
                &self.atlas_buffer,
                None,
                Some(&uav_desc),
                self.ctx.cpu_descriptor(1),
            );

            // Param 1: SRV table.
            list.SetComputeRootDescriptorTable(1, self.ctx.gpu_descriptor(0));
            // Param 2: UAV table.
            list.SetComputeRootDescriptorTable(2, self.ctx.gpu_descriptor(1));

            // Dispatch: ceil(tile_size / 16) groups in X and Y.
            let groups = (self.atlas.tile_size + 15) / 16;
            list.Dispatch(groups, groups, 1);
        }

        // OPTIMIZATION: Use async execution instead of execute_and_wait
        // This allows multiple thumbnails to be queued on GPU without blocking CPU
        // We only wait during readback operations
        let fence_val = self.ctx.compute_queue.execute_async()?;

        Ok(fence_val)
    }
}
