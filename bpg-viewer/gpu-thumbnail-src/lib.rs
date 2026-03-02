//! GPU-accelerated thumbnail pipeline using D3D12 multi-queue architecture.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────┐   staging   ┌────────────┐   compute   ┌──────────────┐
//! │  CPU     │──(upload)──▶│  COPY queue │──(fence)──▶│ COMPUTE queue│
//! │  decode  │             │  (future)   │            │  YCbCr→RGB   │
//! └──────────┘             └────────────┘            │  + resize    │
//!                                                     │  → atlas     │
//!                                                     └──────────────┘
//! ```
//!
//! The atlas is a GPU buffer of RGBA8 pixels divided into fixed-size tiles.
//! Each tile holds one resized thumbnail.

pub mod error;
pub mod dx12_multi_queue;
pub mod staging_ring;
pub mod atlas;
pub mod job_queue;
pub mod compute;
pub mod pipeline;

// ── Compiled shader bytecode ────────────────────────────────────────────────

mod shaders_include {
    #[cfg(not(any(doc, test, feature = "no-include-shaders")))]
    include!(env!("SHADER_INCLUDE_PATH"));

    #[cfg(any(doc, test, feature = "no-include-shaders"))]
    pub const YCBCR_RESIZE_SHADER: &[u8] = &[];
}

pub use shaders_include::YCBCR_RESIZE_SHADER;

// ── Re-exports ──────────────────────────────────────────────────────────────

pub use error::{Result, ThumbnailError};
pub use dx12_multi_queue::{MultiQueueContext, CommandQueue, QueueType};
pub use staging_ring::StagingRing;
pub use atlas::{ThumbnailAtlas, TileIndex, TileState};
pub use job_queue::{JobQueue, ThumbnailJob, Priority, CancelToken};
pub use pipeline::{ThumbnailPipeline, YCbCrData};
pub use compute::YCbCrResizeParams;

// ── Default constants ───────────────────────────────────────────────────────

/// Default atlas edge length in pixels.
pub const DEFAULT_ATLAS_SIZE: u32 = 4096;

/// Default tile edge length in pixels.
pub const DEFAULT_TILE_SIZE: u32 = 256;

/// GPU fence timeout in milliseconds.
/// Increased from 5000 to 10000 to handle heavy loads and prevent stalls
pub const DEFAULT_GPU_TIMEOUT_MS: u32 = 10000;
