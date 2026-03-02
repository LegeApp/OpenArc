use thiserror::Error;

pub type Result<T> = std::result::Result<T, ThumbnailError>;

#[derive(Error, Debug)]
pub enum ThumbnailError {
    #[error("D3D12 initialization failed: {0}")]
    InitFailed(String),

    #[error("D3D12 HRESULT error: 0x{0:08X}")]
    HResult(i32),

    #[error("GPU timeout after {0}ms")]
    GpuTimeout(u32),

    #[error("Atlas full: all {0} tiles occupied, LRU eviction exhausted")]
    AtlasFull(u32),

    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    #[error("JPEG decode failed: {0}")]
    DecodeFailed(String),

    #[error("Job cancelled")]
    Cancelled,

    #[error("Staging ring exhausted: all {0} slots in flight")]
    StagingExhausted(usize),

    #[error("Resource creation failed: {0}")]
    ResourceFailed(String),

    #[error("Shader not compiled (empty bytecode). Build with dxc or disable `no-include-shaders`).")]
    ShaderNotCompiled,

    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),
}
