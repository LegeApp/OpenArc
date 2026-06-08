/// Snappy codec options.
///
/// Snappy is Google's fast byte-level compressor — typically 2-3× faster than
/// LZ4 to compress, with similar ratios. It's the de-facto default in big-data
/// ecosystems (Parquet/ORC/Avro column compression, Hadoop, Cassandra, Kafka).
///
/// We use the **raw block format** (no frame header). For the framed `.sz`
/// format used by Snappy as a stream wrapper, see `snap::read::FrameDecoder`
/// (not currently exposed here).
///
/// Snappy has no tunable knobs — it's "one speed, one ratio."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SnappyOptions;
