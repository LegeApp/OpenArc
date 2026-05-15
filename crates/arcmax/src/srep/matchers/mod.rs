pub mod fixed;
pub mod hash_table;
pub mod inmem;
pub mod slice_hash;

pub use hash_table::{FixedChunkTable, TableLayout};
pub use inmem::{InMemDeduplicator, PreparedBlock, RingDict};
pub use slice_hash::SliceHash;
