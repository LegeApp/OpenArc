use crate::error::{ArcError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyKind {
    Store,
    Byte,
    Bit,
    Huffman,
    Arithmetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserKind {
    Greedy,
    Lazy,
    Optimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchFinderKind {
    NonCaching,
    Caching { min_len: usize, cycled: bool },
    BinaryTree { min_len: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hash3Mode {
    Disabled,
    Hash2,
    Hash3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TornadoOptions {
    pub level: Option<u8>,
    pub entropy: EntropyKind,
    pub parser: ParserKind,
    pub buffer_size: usize,
    pub hash_size: usize,
    pub hash_row_width: usize,
    pub match_finder: MatchFinderKind,
    pub update_step: usize,
    pub fast_bytes: usize,
    pub find_tables: bool,
    pub aux_hash_size: usize,
    pub aux_hash_row_width: usize,
    pub hash3_mode: Hash3Mode,
}

impl Default for TornadoOptions {
    fn default() -> Self {
        Self::preset(5).expect("default Tornado preset is valid")
    }
}

impl TornadoOptions {
    pub fn store() -> Self {
        Self {
            level: Some(0),
            entropy: EntropyKind::Store,
            parser: ParserKind::Greedy,
            buffer_size: 0,
            hash_size: 0,
            hash_row_width: 0,
            match_finder: MatchFinderKind::NonCaching,
            update_step: 0,
            fast_bytes: 0,
            find_tables: false,
            aux_hash_size: 0,
            aux_hash_row_width: 0,
            hash3_mode: Hash3Mode::Disabled,
        }
    }

    pub fn preset(level: u8) -> Result<Self> {
        if level > 16 {
            return Err(ArcError::InvalidMethod(format!(
                "invalid Tornado preset: {level}"
            )));
        }

        if level == 0 {
            return Ok(Self::store());
        }

        let parser = if level >= 11 {
            ParserKind::Optimal
        } else if level >= 5 {
            ParserKind::Lazy
        } else {
            ParserKind::Greedy
        };
        let match_finder = match level {
            1..=3 => MatchFinderKind::NonCaching,
            4..=6 => MatchFinderKind::Caching {
                min_len: 4,
                cycled: false,
            },
            7..=9 => MatchFinderKind::Caching {
                min_len: 5,
                cycled: true,
            },
            10 => MatchFinderKind::Caching {
                min_len: 7,
                cycled: true,
            },
            11..=13 => MatchFinderKind::Caching {
                min_len: 5,
                cycled: false,
            },
            14..=15 => MatchFinderKind::Caching {
                min_len: 6,
                cycled: false,
            },
            16 => MatchFinderKind::BinaryTree { min_len: 5 },
            _ => unreachable!(),
        };
        // Entropy coder per FreeArc std_Tornado_method[].
        let entropy = match level {
            1 => EntropyKind::Byte,
            2 => EntropyKind::Bit,
            3..=4 => EntropyKind::Huffman,
            5..=16 => EntropyKind::Arithmetic,
            _ => unreachable!(),
        };
        let mb = 1024 * 1024;
        let kb = 1024;

        Ok(Self {
            level: Some(level),
            entropy,
            parser,
            buffer_size: match level {
                1 => mb,
                2 => 2 * mb,
                3 => 4 * mb,
                4 => 8 * mb,
                5 => 16 * mb,
                6 => 64 * mb,
                7 => 256 * mb,
                8..=10 => 1024 * mb,
                11..=16 => 128 * mb,
                _ => unreachable!(),
            },
            hash_size: match level {
                1 => 16 * kb,
                2 => 64 * kb,
                3 => 128 * kb,
                4 => 2 * mb,
                5 => 8 * mb,
                6 => 32 * mb,
                7 => 128 * mb,
                8 => 512 * mb,
                9..=10 => 2048 * mb,
                11..=12 => 256 * mb,
                13 => 384 * mb,
                14..=15 => 512 * mb,
                16 => 128 * mb,
                _ => unreachable!(),
            },
            hash_row_width: match level {
                1..=2 => 1,
                3..=4 => 2,
                5 => 4,
                6 | 11 => 8,
                7 | 14 => 32,
                8 | 16 => 128,
                9..=10 => 256,
                12 => 16,
                13 => 24,
                15 => 64,
                _ => unreachable!(),
            },
            match_finder,
            update_step: match level {
                7..=16 => 1,
                _ => 999,
            },
            fast_bytes: match level {
                11 => 16,
                12 => 32,
                13..=14 => 64,
                16 => 512,
                _ => 128,
            },
            find_tables: level >= 3,
            aux_hash_size: match level {
                7..=8 => 128 * kb,
                9 => 512 * kb,
                10 => 16 * mb,
                11..=12 => 64 * kb,
                13 => 128 * kb,
                14..=15 => 2 * mb,
                16 => 4 * mb,
                _ => 0,
            },
            aux_hash_row_width: match level {
                7..=9 | 15 => 4,
                10 | 16 => 8,
                11..=13 => 1,
                14 => 2,
                _ => 0,
            },
            hash3_mode: match level {
                7..=10 | 14..=16 => Hash3Mode::Hash2,
                5..=6 | 11..=13 => Hash3Mode::Hash3,
                _ => Hash3Mode::Disabled,
            },
        })
    }
}
