//! Rolling hash implementations used by the SREP match-finding engine.
//!
//! Both implementations use wrapping arithmetic throughout — the C++ relies on
//! unsigned overflow, so every arithmetic operation here must be `wrapping_*`.

// ---------------------------------------------------------------------------
// Polynomial rolling hash
// ---------------------------------------------------------------------------

/// Polynomial rolling hash over a fixed-length window of bytes.
///
/// Equivalent to C++ `PolynomialRollingHash<u64>` in `srep.cpp` lines 321–427.
/// The canonical seed for `-m3` and the fixed-block compressor is `PRIME1 = 153191`.
///
/// Invariant: after `move_to` or a series of `update` calls, `value()` is the
/// polynomial hash of the current `window`-byte window.
#[derive(Clone)]
pub struct PolyRolling64 {
    value: u64,
    prime: u64,
    /// `prime ^ window` — the factor for the byte being dropped.
    prime_l: u64,
    window: usize,
}

/// The default prime seed used in the `-m3` compress path (`PRIME1` in the C++).
pub const PRIME_M3: u64 = 153191;
/// Secondary prime (`PRIME2` in the C++).
pub const PRIME_M3_ALT: u64 = 3141601;

impl PolyRolling64 {
    /// Create a new hasher for a window of `window` bytes using `prime` as seed.
    pub fn new(window: usize, prime: u64) -> Self {
        let prime_l = pow_wrapping(prime, window);
        PolyRolling64 {
            value: 0,
            prime,
            prime_l,
            window,
        }
    }

    /// Initialise the hasher by hashing the first `window` bytes of `data`.
    /// Does nothing if `data.len() < window`.
    #[inline]
    pub fn move_to(&mut self, data: &[u8]) {
        self.value = 0;
        for &b in data.iter().take(self.window) {
            self.value = self.value.wrapping_mul(self.prime).wrapping_add(b as u64);
        }
    }

    /// Slide the window one byte forward: remove `sub` (the departing byte)
    /// and add `add` (the new byte entering at the right).
    #[inline(always)]
    pub fn update(&mut self, sub: u8, add: u8) {
        self.value = self
            .value
            .wrapping_mul(self.prime)
            .wrapping_add(add as u64)
            .wrapping_sub(self.prime_l.wrapping_mul(sub as u64));
    }

    #[inline(always)]
    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn window(&self) -> usize {
        self.window
    }
}

/// Compute `base^exp` with wrapping (unsigned overflow) arithmetic.
pub fn pow_wrapping(base: u64, exp: usize) -> u64 {
    let mut result = 1u64;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        b = b.wrapping_mul(b);
        e >>= 1;
    }
    result
}

// ---------------------------------------------------------------------------
// CRC rolling hash
// ---------------------------------------------------------------------------

/// Table-driven CRC rolling hash over a fixed-length window.
///
/// Equivalent to C++ `CrcRollingHash<u32>` in `srep.cpp` lines 465–529.
/// Two CRC polynomials are provided as constants:
/// - `CRC32_CASTAGNOLI` (0x82F63B78) — same as iSCSI/SSE4.2 CRC32C
/// - `CRC32_IEEE` (0xEDB88320) — same as zlib/gzip
#[derive(Clone)]
pub struct CrcRollingHash {
    value: u32,
    /// Standard CRC table (used for adding new bytes).
    crc_tab: Box<[u32; 256]>,
    /// Rolling CRC table (used for subtracting departing bytes).
    rolling_tab: Box<[u32; 256]>,
    window: usize,
}

pub const CRC32_CASTAGNOLI: u32 = 0x82F63B78;
pub const CRC32_IEEE: u32 = 0xEDB88320;

impl CrcRollingHash {
    /// Build a new rolling hasher for a window of `window` bytes using `polynom`.
    pub fn new(window: usize, polynom: u32) -> Self {
        let crc_tab = fast_table_build(polynom, polynom);

        // Seed for the rolling table: update_crc(0, 128) applied L times with 0.
        let mut crc = update_crc_tab(&crc_tab, 0, 128);
        for _ in 0..window {
            crc = update_crc_tab(&crc_tab, crc, 0);
        }
        let rolling_tab = fast_table_build(crc, polynom);

        CrcRollingHash {
            value: 0,
            crc_tab: Box::new(crc_tab),
            rolling_tab: Box::new(rolling_tab),
            window,
        }
    }

    /// Compute the CRC of the first `window` bytes of `data` from scratch.
    #[inline]
    pub fn move_to(&mut self, data: &[u8]) {
        self.value = 0;
        for &b in data.iter().take(self.window) {
            // update with sub=0; rolling_tab[0] = 0, so no rolling term.
            self.value = update_crc_tab(&self.crc_tab, self.value, b);
        }
    }

    /// Slide the window: drop `sub` and add `add`.
    #[inline(always)]
    pub fn update(&mut self, sub: u8, add: u8) {
        self.value =
            update_crc_tab(&self.crc_tab, self.value, add) ^ self.rolling_tab[sub as usize];
    }

    #[inline(always)]
    pub fn value(&self) -> u32 {
        self.value
    }

    pub fn window(&self) -> usize {
        self.window
    }
}

/// One step of the table-driven CRC.
#[inline(always)]
fn update_crc_tab(tab: &[u32; 256], v: u32, c: u8) -> u32 {
    tab[((v ^ c as u32) & 0xFF) as usize] ^ (v >> 8)
}

/// Build a CRC lookup table.
///
/// Port of `FastTableBuild<uint32>` in `srep.cpp` lines 498–508.
fn fast_table_build(seed: u32, polynom: u32) -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut crc = seed;
    // table[0] = 0 (already zeroed)
    table[128] = crc;

    // Powers of 2 from 64 down to 1.
    let mut i = 64usize;
    loop {
        crc = (crc >> 1) ^ (if crc & 1 == 1 { polynom } else { 0 });
        table[i] = crc;
        if i == 1 {
            break;
        }
        i /= 2;
    }

    // Fill composite entries by XOR of their power-of-2 components.
    let mut i = 2usize;
    while i < 256 {
        let mut j = 1usize;
        while j < i {
            table[i + j] = table[i] ^ table[j];
            j += 1;
        }
        i *= 2;
    }
    table
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
