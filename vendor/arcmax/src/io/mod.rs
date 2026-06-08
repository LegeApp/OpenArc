pub mod bounded {
    pub use std::io::{Read, Seek, Write};
}

pub mod counting {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct ByteCounts {
        pub read: u64,
        pub written: u64,
    }
}
