pub mod cdc;
pub mod fixed;
pub mod hash_table;
pub mod inmem;
pub mod slice_hash;

pub use cdc::{find_chunks, CdcMatcher, CdcMode};
pub use hash_table::{FixedChunkTable, TableLayout};
pub use inmem::{InMemDeduplicator, PreparedBlock, RingDict};
pub use slice_hash::SliceHash;
