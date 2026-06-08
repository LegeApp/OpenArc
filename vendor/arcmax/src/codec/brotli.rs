/// Brotli codec options.
///
/// Brotli is Google's general-purpose compressor, designed for HTTP delivery.
/// It ships with a 122 KiB built-in static dictionary tuned for HTML/JS/CSS,
/// which lets it beat gzip and often beat zstd:11 on web text payloads.
///
/// Quality range: 0 (fastest, weakest) — 11 (slowest, strongest). Default: 6,
/// a compromise that compresses comparably to gzip:9 at higher speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrotliOptions {
    /// Compression quality 0–11. Default 6.
    pub quality: u32,
    /// Sliding window log size. Valid range 10–24; larger uses more memory
    /// and finds longer matches. Default 22 (4 MiB window).
    pub lgwin: u32,
}

impl Default for BrotliOptions {
    fn default() -> Self {
        Self {
            quality: 6,
            lgwin: 22,
        }
    }
}
