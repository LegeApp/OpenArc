use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArcError>;

#[derive(Error, Debug)]
pub enum ArcError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid archive: {0}")]
    InvalidArchive(&'static str),

    #[error("invalid method: {0}")]
    InvalidMethod(String),

    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("codec error in {codec}: {message}")]
    Codec {
        codec: &'static str,
        message: String,
    },

    #[error("checksum mismatch: expected {expected:#x}, got {actual:#x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("password required")]
    PasswordRequired,

    #[error("wrong password or corrupted encrypted data")]
    CryptoVerificationFailed,
}

impl From<crate::srep::SrepError> for ArcError {
    fn from(err: crate::srep::SrepError) -> Self {
        ArcError::Codec {
            codec: "srep",
            message: err.to_string(),
        }
    }
}
