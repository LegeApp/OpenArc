use thiserror::Error;

#[derive(Error, Debug)]
pub enum SrepError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid archive format: {0}")]
    Format(&'static str),

    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    #[error("allocation failed for {component}: requested {bytes} bytes")]
    Allocation { component: &'static str, bytes: u64 },

    #[error("integer overflow in {0}")]
    Overflow(&'static str),
}
