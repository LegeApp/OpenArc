// Native Rust GRZip decompressor.
//
// Ports GRZip_DecompressBlock and all sub-functions from:
//   freearc_cpp_lib/Compression/GRZip/{C_GRZip.cpp,BWT.c,ST4.c,
//   MTF_Ari.c,WFC_Ari.c,Rec_Flt.c}
//
// Compression is not implemented natively; use the ffi-codecs feature for that.

use std::io::{Read, Write};

use crate::codec::framing;
use crate::codec::grzip::GrzipOptions;
use crate::codec::lzp_codec::lzp_decode;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

// ---------------------------------------------------------------------------
// Constants (from libGRZip.h)
// ---------------------------------------------------------------------------

const GRZ_COMPRESSION_MTF: i32 = 0x4;
const GRZ_COMPRESSION_ST4: i32 = 0x2;
const STRONG_BWT_FLAG: i32 = 0x40000000;
const ST_INDIRECT: u32 = 0x800000;

const MODEL_MAX_FREQ: u32 = 1 << 11; // 2048
const LOG2_MAX: usize = 24; // GRZ_Log2MaxBlockSize=23, +1

// From WFC_MTF.h
const WFCMTF_RANK2GRNUM: [u8; 256] = [
    0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
];

const WFCMTF_RANK2GRPOS: [u8; 256] = [
    0, 0, 0, 0, 1, 0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
    23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 0,
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
    75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98,
    99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117,
    118, 119, 120, 121, 122, 123, 124, 125, 126,
];

const WFCMTF_GRNUM2GRBEGIN: [u32; 7] = [3, 5, 9, 17, 33, 65, 129];

#[rustfmt::skip]
const WFCMTF_LOG2RLESIZE: [u32; LOG2_MAX] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
    65536, 131072, 262144, 524288, 1048576, 2097152, 4194304, 8388608,
];

// ---------------------------------------------------------------------------
// LZP mode helpers (from libGRZip.h macros)
// ---------------------------------------------------------------------------

#[inline(always)]
fn lzp_enabled(mode: i32) -> bool {
    mode / 256 != 0
}

#[inline(always)]
fn get_lzp_min_match_len(mode: i32) -> usize {
    ((mode / 65536) % 32767) as usize
}

#[inline(always)]
fn get_lzp_hash_size_log(mode: i32) -> u8 {
    ((mode / 256) % 256) as u8
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

#[cold]
fn err_corrupt(msg: &'static str) -> ArcError {
    ArcError::Codec {
        codec: "grzip",
        message: msg.into(),
    }
}

// ---------------------------------------------------------------------------
// Record filter decode (Rec_Flt.c: GRZip_Rec_Decode)
// ---------------------------------------------------------------------------

fn rec_decode(input: &[u8], size: usize, mode: i32) -> Vec<u8> {
    let mut out = vec![0u8; size];

    if mode == 3 {
        // 2-byte delta decode
        let num_records = size >> 1;
        let mut pred: u16 = 0;
        for r in 0..num_records {
            let hi = input[r];
            let lo = input[r + num_records];
            let mut delta = ((hi as u16) << 8) | (lo as u16);
            if delta & 1 != 0 {
                delta = !(delta >> 1);
            } else {
                delta >>= 1;
            }
            let code = delta.wrapping_add(pred);
            pred = code;
            let bytes = code.to_le_bytes();
            out[2 * r] = bytes[0];
            out[2 * r + 1] = bytes[1];
        }
        // copy tail beyond even boundary
        for i in (2 * num_records)..size {
            out[i] = input[i - num_records];
        }
    } else if mode == 4 {
        // 4-byte delta decode
        let num_records = size >> 2;
        let p1 = num_records;
        let p2 = 2 * num_records;
        let p3 = 3 * num_records;
        let mut pred: u32 = 0;
        for r in 0..num_records {
            let b0 = input[r] as u32;
            let b1 = input[r + p1] as u32;
            let b2 = input[r + p2] as u32;
            let b3 = input[r + p3] as u32;
            let mut delta = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
            // reconstruct: hi byte first in encoded stream
            // C: Delta = *Input; Delta=(Delta<<8)|*(Input+P3); ... Input++
            // Re-read C carefully: *Input=b0(=byte at r), *(Input+P1)=byte at r+P1, etc.
            // Delta = *Input; Delta=(Delta<<8)|*(Input+P3); Delta=(Delta<<8)|*(Input+P2); Delta=(Delta<<8)|*(Input+P1);
            let b0c = input[r];
            let b1c = input[r + p1];
            let b2c = input[r + p2];
            let b3c = input[r + p3];
            let mut d2 = b0c as u32;
            d2 = (d2 << 8) | b3c as u32;
            d2 = (d2 << 8) | b2c as u32;
            d2 = (d2 << 8) | b1c as u32;
            delta = d2;
            if delta & 1 != 0 {
                delta = !(delta >> 1);
            } else {
                delta >>= 1;
            }
            let code = delta.wrapping_add(pred);
            pred = code;
            let bytes = code.to_le_bytes();
            out[4 * r] = bytes[0];
            out[4 * r + 1] = bytes[1];
            out[4 * r + 2] = bytes[2];
            out[4 * r + 3] = bytes[3];
        }
        for i in (4 * num_records)..size {
            out[i] = input[i - num_records];
        }
    } else if mode == 1 {
        // de-interleave 2 channels
        let mut in_pos = 0;
        let mut i = 0;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 2;
        }
        let mut i = 1;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 2;
        }
    } else if mode == 2 {
        // de-interleave 4 channels
        let mut in_pos = 0;
        let mut i = 0;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 4;
        }
        let mut i = 1;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 4;
        }
        let mut i = 2;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 4;
        }
        let mut i = 3;
        while i < size {
            out[i] = input[in_pos];
            in_pos += 1;
            i += 4;
        }
    }

    out
}

// ---------------------------------------------------------------------------
// BWT inverse (BWT.c: GRZip_FastBWT_Decode / GRZip_StrongBWT_Decode)
// ---------------------------------------------------------------------------

fn bwt_fast_decode(data: &mut [u8], fbp: usize) -> Result<()> {
    let size = data.len();
    let mut count = [0u32; 256];
    let mut t: Vec<u32> = vec![0u32; size];

    for i in 0..size {
        let c = data[i] as usize;
        t[i] = ((count[c]) << 8) | (c as u32);
        count[c] += 1;
    }

    // prefix sum (0-based)
    let mut sum: u32 = 0;
    for i in 0..256 {
        let cnt = count[i];
        count[i] = sum;
        sum += cnt;
    }

    let mut cursor = fbp;
    for i in (0..size).rev() {
        let u = t[cursor];
        let c = (u & 0xFF) as usize;
        cursor = ((u >> 8) + count[c]) as usize;
        data[i] = c as u8;
    }

    Ok(())
}

fn bwt_strong_decode(data: &mut [u8], fbp: usize) -> Result<()> {
    let size = data.len();
    // T has size+1 entries; T[fbp] is left zero (the virtual slot).
    let mut count = [0u32; 256];
    let mut t: Vec<u32> = vec![0u32; size + 1];

    for i in 0..fbp {
        let c = data[i] as usize;
        t[i] = (count[c] << 8) | (c as u32);
        count[c] += 1;
    }
    for i in fbp..size {
        let c = data[i] as usize;
        t[i + 1] = (count[c] << 8) | (c as u32);
        count[c] += 1;
    }

    // prefix sum (1-based, so Sum starts at 1)
    let mut sum: u32 = 1;
    for i in 0..256 {
        let cnt = count[i];
        count[i] = sum;
        sum += cnt;
    }

    // cursor starts at 0 (not at fbp)
    let mut cursor: usize = 0;
    for i in (0..size).rev() {
        let u = t[cursor];
        let c = (u & 0xFF) as usize;
        cursor = ((u >> 8) + count[c]) as usize;
        data[i] = c as u8;
    }

    Ok(())
}

fn bwt_decode(data: &mut [u8], fbp: i32) -> Result<()> {
    if fbp & STRONG_BWT_FLAG == 0 {
        bwt_fast_decode(data, fbp as usize)
    } else {
        bwt_strong_decode(data, (fbp & !STRONG_BWT_FLAG) as usize)
    }
}

// ---------------------------------------------------------------------------
// ST4 inverse (ST4.c: GRZip_ST4_Decode)
// ---------------------------------------------------------------------------

fn st4_decode(data: &mut [u8], fbp: usize) -> Result<()> {
    let size = data.len();

    let mut context2 = vec![0i32; 65536];
    let flag_size = (size + 8) >> 3;
    let mut flag = vec![0u8; flag_size];
    let mut table: Vec<u32> = vec![0u32; size + 1];

    // T[c] = start index for char c; initially count then prefix sum
    let mut t = [0i32; 256];
    let mut s = [0i32; 256];
    let mut last_seen = [-1i32; 256];

    for i in 0..size {
        t[data[i] as usize] += 1;
    }

    // Build prefix sums and simultaneously count context2 pairs
    let mut j: usize = 0;
    let mut sum: i32 = 0;
    for i in 0..256usize {
        let cnt = t[i];
        t[i] = sum;
        sum += cnt;
        // for positions j..j+cnt, context2[(data[j]<<8)|i]++
        for k in j..(j + cnt as usize) {
            let pair = ((data[k] as usize) << 8) | i;
            context2[pair] += 1;
        }
        j += cnt as usize;
    }
    s.copy_from_slice(&t);

    // Build flag bits: mark positions that start a new context block
    last_seen.fill(0xFF_u8 as i8 as i32); // -1 as i32
    j = 0;
    sum = 0;
    let mut i2: usize = 0;
    while i2 < 65536 {
        let c_start = sum;
        let cnt = context2[i2];
        sum += cnt;
        for k in (j as i32)..(j as i32 + cnt) {
            let c = data[k as usize] as usize;
            let tc = t[c];
            if last_seen[c] != c_start {
                last_seen[c] = c_start;
                // ST_SetBit(T[c])
                let bit = tc as usize;
                flag[bit >> 3] |= 1 << (bit & 7);
            }
            t[c] += 1;
        }
        j += cnt as usize;
        i2 += 1;
    }

    last_seen.fill(0);

    // Build Table entries: direct or indirect pointers
    let mut c_start: usize = 0;
    for i in 0..size {
        let c = data[i] as usize;
        // ST_GetBit(i)
        if flag[i >> 3] & (1 << (i & 7)) != 0 {
            c_start = i;
        }
        if last_seen[c] <= c_start as i32 {
            table[i] = (s[c] as u32) | ((c as u32) << 24);
            last_seen[c] = i as i32 + 1;
        } else {
            table[i] = ((last_seen[c] as u32 - 1) | ST_INDIRECT) | ((c as u32) << 24);
        }
        s[c] += 1;
    }
    table[size] = ST_INDIRECT;

    // Walk the chain to reconstruct
    let mut j = fbp;
    let mut sum_val = table[fbp];
    for i in 0..size {
        if sum_val & ST_INDIRECT != 0 {
            let idx = (sum_val & (ST_INDIRECT - 1)) as usize;
            table[idx] = table[idx].wrapping_add(1);
            j = ((table[idx] - 1) & (ST_INDIRECT - 1)) as usize;
            sum_val = table[j];
            data[i] = (sum_val >> 24) as u8;
        } else {
            table[j] = table[j].wrapping_add(1);
            j = (sum_val & (ST_INDIRECT - 1)) as usize;
            sum_val = table[j];
            data[i] = (sum_val >> 24) as u8;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Range coder
// ---------------------------------------------------------------------------

struct AriDecoder<'a> {
    input: &'a [u8],
    pos: usize,
    code: u32,
    range: u32,
}

impl<'a> AriDecoder<'a> {
    fn new(input: &'a [u8]) -> Self {
        let mut s = AriDecoder {
            input,
            pos: 0,
            code: 0,
            range: u32::MAX,
        };
        // Read 5 bytes: byte0..3 shift left, byte4 stays
        s.code |= s.read_byte() as u32;
        s.code <<= 8;
        s.code |= s.read_byte() as u32;
        s.code <<= 8;
        s.code |= s.read_byte() as u32;
        s.code <<= 8;
        s.code |= s.read_byte() as u32;
        s.code <<= 8;
        s.code |= s.read_byte() as u32;
        s
    }

    #[inline(always)]
    fn read_byte(&mut self) -> u8 {
        if self.pos < self.input.len() {
            let b = self.input[self.pos];
            self.pos += 1;
            b
        } else {
            0
        }
    }

    // ARI_GetFreq: modifies range (range /= tot), returns code/range.
    #[inline(always)]
    fn get_freq(&mut self, tot: u32) -> u32 {
        self.range /= tot;
        self.code / self.range
    }

    // ARI_Decode: consumes a symbol (range already modified by get_freq).
    #[inline(always)]
    fn decode(&mut self, freq: u32, cum: u32) {
        self.code -= cum * self.range;
        self.range *= freq;
        while self.range < (1u32 << 24) {
            self.code = (self.code << 8) | self.read_byte() as u32;
            self.range <<= 8;
        }
    }

    // Decode bit 0 (freq0 = given, cum=0).
    #[inline(always)]
    fn decode0(&mut self, freq0: u32) {
        self.decode(freq0, 0);
    }

    // Decode bit 1 (freq0 = given threshold, so freq1 = MaxFreq-freq0, cum=freq0).
    #[inline(always)]
    fn decode1(&mut self, freq0: u32) {
        self.decode(MODEL_MAX_FREQ - freq0, freq0);
    }
}

// ---------------------------------------------------------------------------
// Shared model state for MTF/WFC decoders
// ---------------------------------------------------------------------------

struct Models {
    l0_0: [u32; 5],
    l0_1: Vec<[u32; 5]>, // [256][5]
    l0_2: Vec<[u32; 5]>, // [1024][5]
    l1_0: [u32; 8],
    l1_1: [[u32; 8]; 8],
    l2_0: [[u32; 128]; 8],
    log2rle_0: Vec<[u32; LOG2_MAX]>, // [64][24]
    log2rle_1: Vec<[u32; LOG2_MAX]>, // [256][24]
    log2rle_2: [[u32; LOG2_MAX]; LOG2_MAX],
    ctx_rle: usize,
    ctx_l0: usize,
    ctx_l1: usize,
}

impl Models {
    fn new() -> Box<Self> {
        let half = MODEL_MAX_FREQ >> 1;
        Box::new(Models {
            l0_0: [1, 1, 1, 1, 4],
            l0_1: vec![[0u32; 5]; 256],
            l0_2: vec![[0u32; 5]; 1024],
            l1_0: [half; 8],
            l1_1: [[half; 8]; 8],
            l2_0: [[half; 128]; 8],
            log2rle_0: vec![[half; LOG2_MAX]; 64],
            log2rle_1: vec![[half; LOG2_MAX]; 256],
            log2rle_2: [[half; LOG2_MAX]; LOG2_MAX],
            ctx_rle: 0,
            ctx_l0: 0,
            ctx_l1: 0,
        })
    }

    #[inline(always)]
    fn update_l0(&mut self, v_row: usize, u_row: usize) {
        if self.l0_0[4] > 58 {
            self.l0_0[0] = (self.l0_0[0] + 1) >> 1;
            self.l0_0[1] = (self.l0_0[1] + 1) >> 1;
            self.l0_0[2] = (self.l0_0[2] + 1) >> 1;
            self.l0_0[3] = (self.l0_0[3] + 1) >> 1;
            self.l0_0[4] = self.l0_0[0] + self.l0_0[1] + self.l0_0[2] + self.l0_0[3];
        }
        let v = &mut self.l0_1[v_row];
        if v[4] > 62 {
            v[0] >>= 1;
            v[1] >>= 1;
            v[2] >>= 1;
            v[3] >>= 1;
            v[4] = v[0] + v[1] + v[2] + v[3];
        }
        let u = &mut self.l0_2[u_row];
        if u[4] > 204 {
            u[0] >>= 1;
            u[1] >>= 1;
            u[2] >>= 1;
            u[3] >>= 1;
            u[4] = u[0] + u[1] + u[2] + u[3];
        }
    }
}

// ---------------------------------------------------------------------------
// The shared decode loop body (used by both MTF and WFC)
//
// `resolve_and_update(actual_rank, raw_wfcmtf_rank) -> char` does both the
// list lookup AND the list update in one call, letting callers hold exclusive
// access to their list state via a single closure.
// ---------------------------------------------------------------------------

fn ari_decode_inner<F>(input: &[u8], mut resolve_and_update: F) -> Result<Vec<u8>>
where
    F: FnMut(usize, u32) -> u8, // (actual_rank, wfcmtf_rank) -> char
{
    let mut ari = AriDecoder::new(input);
    let mut m = Models::new();

    let mut output: Vec<u8> = Vec::new();
    let mut pred_char: usize = 0;
    let mut char_val: u8 = 0;

    loop {
        let v_row = pred_char;
        let u_row = 4 * m.ctx_l0 + (m.ctx_rle & 3);

        let tot = m.l0_0[4] + m.l0_1[v_row][4] + m.l0_2[u_row][4];
        let frq = ari.get_freq(tot);

        // Find which L0 bucket
        let mut cum: u32 = 0;
        let mut rank: usize = 0;
        while frq >= cum {
            cum += m.l0_0[rank] + m.l0_1[v_row][rank] + m.l0_2[u_row][rank];
            rank += 1;
        }
        rank -= 1;
        cum -= m.l0_0[rank] + m.l0_1[v_row][rank] + m.l0_2[u_row][rank];

        let sym_freq = m.l0_0[rank] + m.l0_1[v_row][rank] + m.l0_2[u_row][rank];
        ari.decode(sym_freq, cum);

        m.l0_0[rank] += 2;
        m.l0_1[v_row][rank] += 2;
        m.l0_2[u_row][rank] += 2;
        m.l0_0[4] += 2;
        m.l0_1[v_row][4] += 2;
        m.l0_2[u_row][4] += 2;
        m.update_l0(v_row, u_row);

        // rank == 3 means "higher group" (extended encoding)
        let mut wfcmtf_rank: u32 = rank as u32;

        if rank == 3 {
            // Decode L1 (group number) and L2 (group position)
            let mut gr_num: usize = 0;
            let ctx_l1 = m.ctx_l1;
            {
                let v1 = &mut m.l1_0;
                let u1 = &mut m.l1_1[ctx_l1];
                // Use raw indexing to avoid simultaneous borrows
                while gr_num != 6 {
                    let thresh = (v1[gr_num] + u1[gr_num]) >> 1;
                    let f = ari.get_freq(MODEL_MAX_FREQ);
                    if f < thresh {
                        ari.decode0(thresh);
                        v1[gr_num] += (MODEL_MAX_FREQ - v1[gr_num]) >> 4;
                        u1[gr_num] += (MODEL_MAX_FREQ - u1[gr_num]) >> 6;
                        break;
                    }
                    ari.decode1(thresh);
                    v1[gr_num] -= v1[gr_num] >> 4;
                    u1[gr_num] -= u1[gr_num] >> 6;
                    gr_num += 1;
                }
            }
            m.ctx_l1 = gr_num;

            // L2: binary tree decode within group
            let mut gr_pos: u32 = 0;
            let mut ctx_l2: usize = 1;
            for _ in 0..=gr_num {
                let thresh = m.l2_0[gr_num][ctx_l2];
                let f = ari.get_freq(MODEL_MAX_FREQ);
                if f < thresh {
                    ari.decode0(thresh);
                    m.l2_0[gr_num][ctx_l2] += (MODEL_MAX_FREQ - m.l2_0[gr_num][ctx_l2]) >> 7;
                    ctx_l2 <<= 1;
                    gr_pos <<= 1;
                } else {
                    ari.decode1(thresh);
                    m.l2_0[gr_num][ctx_l2] -= m.l2_0[gr_num][ctx_l2] >> 7;
                    ctx_l2 = (ctx_l2 << 1) | 1;
                    gr_pos = (gr_pos << 1) | 1;
                }
            }

            // Check end-of-stream sentinel: GrNum==6, GrPos==127
            if gr_num == 6 && gr_pos == 127 {
                break;
            }

            wfcmtf_rank = WFCMTF_GRNUM2GRBEGIN[gr_num] + gr_pos;
        }

        let actual_rank = ((wfcmtf_rank + 1) & 0xFF) as usize;
        char_val = resolve_and_update(actual_rank, wfcmtf_rank);

        let rank_clamped = wfcmtf_rank.min(3) as usize;
        m.ctx_l0 = ((m.ctx_l0 << 2) | rank_clamped) & 0xFF;

        // Decode Log2RLE run length
        let log2rle_0_row = m.ctx_rle + 16 * rank_clamped;
        let log2rle_1_row = char_val as usize;

        let mut log2_run_size: usize = 0;
        loop {
            let thresh = (m.log2rle_0[log2rle_0_row][log2_run_size]
                + m.log2rle_1[log2rle_1_row][log2_run_size])
                >> 1;
            let f = ari.get_freq(MODEL_MAX_FREQ);
            if f < thresh {
                ari.decode0(thresh);
                m.log2rle_0[log2rle_0_row][log2_run_size] +=
                    (MODEL_MAX_FREQ - m.log2rle_0[log2rle_0_row][log2_run_size]) >> 6;
                m.log2rle_1[log2rle_1_row][log2_run_size] +=
                    (MODEL_MAX_FREQ - m.log2rle_1[log2rle_1_row][log2_run_size]) >> 3;
                break;
            }
            ari.decode1(thresh);
            m.log2rle_0[log2rle_0_row][log2_run_size] -=
                m.log2rle_0[log2rle_0_row][log2_run_size] >> 6;
            m.log2rle_1[log2rle_1_row][log2_run_size] -=
                m.log2rle_1[log2rle_1_row][log2_run_size] >> 3;
            log2_run_size += 1;
            if log2_run_size >= LOG2_MAX {
                return Err(err_corrupt("log2rle overflow"));
            }
        }

        if log2_run_size < 2 {
            m.ctx_rle = (m.ctx_rle << 1) & 0xF;
        } else {
            m.ctx_rle = ((m.ctx_rle << 1) | 1) & 0xF;
        }

        // Decode the mantissa bits of the run
        let mut run_size: u32 = 0;
        for k in 0..log2_run_size {
            let thresh = m.log2rle_2[log2_run_size][k];
            let f = ari.get_freq(MODEL_MAX_FREQ);
            if f < thresh {
                ari.decode0(thresh);
                m.log2rle_2[log2_run_size][k] +=
                    (MODEL_MAX_FREQ - m.log2rle_2[log2_run_size][k]) >> 6;
                run_size <<= 1;
            } else {
                ari.decode1(thresh);
                m.log2rle_2[log2_run_size][k] -= m.log2rle_2[log2_run_size][k] >> 6;
                run_size = (run_size << 1) | 1;
            }
        }
        run_size += WFCMTF_LOG2RLESIZE[log2_run_size];

        for _ in 0..run_size {
            output.push(char_val);
        }

        pred_char = char_val as usize;
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// MTF + Arithmetic decoder (MTF_Ari.c: GRZip_MTF_Ari_Decode)
// ---------------------------------------------------------------------------

fn mtf_ari_decode(input: &[u8]) -> Result<Vec<u8>> {
    let mut mtf_list: [u8; 256] = core::array::from_fn(|i| i as u8);

    ari_decode_inner(input, |actual_rank, _wfcmtf_rank| {
        let c = mtf_list[actual_rank];
        if actual_rank > 0 {
            mtf_list.copy_within(0..actual_rank, 1);
            mtf_list[0] = c;
        }
        c
    })
}

// ---------------------------------------------------------------------------
// WFC + Arithmetic decoder (WFC_Ari.c: GRZip_WFC_Ari_Decode)
// ---------------------------------------------------------------------------

const WFC_VALS: [i64; 13] = [
    131072, 114688, 7272, 4240, 2364, 1263, 649, 320, 153, 70, 31, 14, 8,
];
const WFC_POSES: [usize; 12] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048];

fn wfc_ari_decode(input: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut wfc_list: [usize; 257] = core::array::from_fn(|i| i);
    let mut char2index: [usize; 257] = core::array::from_fn(|i| i);
    let mut char_weight: [i64; 257] = [0i64; 257];
    char_weight[256] = -1; // sentinel

    let mut wfc_buf: Vec<u8> = Vec::with_capacity(size);
    let mut wfc_buf_pos: usize = 0;

    ari_decode_inner(input, |actual_rank, _wfcmtf_rank| {
        let c = wfc_list[actual_rank];
        let char_val = c as u8;

        // Update_Weight0(c): add WFC_Val0=131072, move to front
        char_weight[c] += WFC_VALS[0];
        let j = char2index[c];
        for k in (1..=j).rev() {
            let prev = wfc_list[k - 1];
            wfc_list[k] = prev;
            char2index[prev] = k;
        }
        wfc_list[0] = c;
        char2index[c] = 0;

        // Weight decay for previous occurrences in history buffer
        for (idx, &pos) in WFC_POSES.iter().enumerate() {
            if wfc_buf_pos >= pos {
                let prev_c = wfc_buf[wfc_buf_pos - pos] as usize;
                let w = WFC_VALS[idx + 1];
                let new_w = char_weight[prev_c] - w;
                char_weight[prev_c] = new_w;
                if prev_c != c {
                    let mut j2 = char2index[prev_c];
                    let mut list_ptr = j2 + 1;
                    while list_ptr < 257 && char_weight[wfc_list[list_ptr]] > new_w {
                        let moved = wfc_list[list_ptr];
                        wfc_list[list_ptr - 1] = moved;
                        char2index[moved] = j2;
                        j2 += 1;
                        list_ptr += 1;
                    }
                    wfc_list[j2] = prev_c;
                    char2index[prev_c] = j2;
                }
            }
        }

        wfc_buf.push(char_val);
        wfc_buf_pos += 1;
        char_val
    })
}

// ---------------------------------------------------------------------------
// Main block decompressor (C_GRZip.cpp: GRZip_DecompressBlock)
// ---------------------------------------------------------------------------

pub(crate) fn decompress_block(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() < 28 {
        return Err(err_corrupt("block too short"));
    }

    let read_i32 =
        |off: usize| -> i32 { i32::from_le_bytes(input[off..off + 4].try_into().unwrap()) };

    let original_size = read_i32(0);
    let mode = read_i32(4);
    let lzp_size_or_mode = read_i32(8);
    let fbp = read_i32(12);
    let body_size = read_i32(16);
    let reserved1 = read_i32(20);
    let reserved2 = read_i32(24);

    if reserved1 != 0 || reserved2 != 0 {
        return Err(err_corrupt("header integrity check failed"));
    }
    if body_size < 0 || (body_size as usize) + 28 > input.len() {
        return Err(err_corrupt("body_size exceeds input"));
    }

    // --- Stored / LZP-only block (mode == -1) ---
    if mode == -1 {
        let lzp_mode = lzp_size_or_mode;
        let body = &input[28..28 + body_size as usize];
        if lzp_mode == 0 {
            return Ok(body.to_vec());
        }
        let min_match = get_lzp_min_match_len(lzp_mode);
        let hash_size_log = get_lzp_hash_size_log(lzp_mode);
        let decoded = lzp_decode(body, original_size as usize, min_match, hash_size_log)?;
        return Ok(decoded);
    }

    // --- Delta sub-blocks (mode == -2) ---
    if mode == -2 {
        let rec_mode = lzp_size_or_mode;
        let size = original_size as usize;

        // Decompress sub-blocks into a flat buffer
        let mut buf: Vec<u8> = Vec::with_capacity(size);
        let mut pos = 28usize;
        let num_parts = if rec_mode & 1 != 0 { 2 } else { 4 };
        for _ in 0..num_parts {
            if pos + 28 > input.len() {
                return Err(err_corrupt("sub-block header out of bounds"));
            }
            let sub_body_size =
                i32::from_le_bytes(input[pos + 16..pos + 20].try_into().unwrap()) as usize;
            let sub_total = sub_body_size + 28;
            if pos + sub_total > input.len() {
                return Err(err_corrupt("sub-block body out of bounds"));
            }
            let sub_result = decompress_block(&input[pos..pos + sub_total])?;
            buf.extend_from_slice(&sub_result);
            pos += sub_total;
        }

        let out = rec_decode(&buf, size, rec_mode);
        return Ok(out);
    }

    // --- Normal block ---
    let lzp_size = lzp_size_or_mode as usize;
    let body = &input[28..28 + body_size as usize];

    // Step 1: Entropy decode (MTF/WFC + arithmetic coding)
    let mut decoded: Vec<u8> = if mode & GRZ_COMPRESSION_MTF != 0 {
        mtf_ari_decode(body)?
    } else {
        wfc_ari_decode(body, lzp_size)?
    };

    // Verify entropy output size (should match lzp_size, rounded up to 8)
    let expected_lzp_size = (lzp_size + 7) & !7;
    if decoded.len() < lzp_size {
        return Err(err_corrupt("entropy decode produced too few bytes"));
    }
    decoded.truncate(expected_lzp_size);

    // Step 2: Inverse sort (BWT or ST4)
    if mode & GRZ_COMPRESSION_ST4 != 0 {
        st4_decode(&mut decoded, fbp as usize)?;
    } else {
        bwt_decode(&mut decoded, fbp)?;
    }

    // Truncate back to lzp_size after inverse sort
    decoded.truncate(lzp_size);

    // Step 3: Optional LZP decode
    if lzp_enabled(mode) {
        let min_match = get_lzp_min_match_len(mode);
        let hash_size_log = get_lzp_hash_size_log(mode);
        let out = lzp_decode(&decoded, original_size as usize, min_match, hash_size_log)?;
        Ok(out)
    } else {
        decoded.truncate(original_size as usize);
        Ok(decoded)
    }
}

/// Decode a FreeArc GRZip byte stream.
///
/// FreeArc stores GRZip data as a concatenation of raw GRZip blocks. Each
/// block carries its own 28-byte header whose `body_size` field determines
/// how many following bytes belong to that block.
pub(crate) fn decompress_stream(input: &[u8]) -> Result<Vec<u8>> {
    let mut pos = 0usize;
    let mut output = Vec::new();

    while pos < input.len() {
        if input.len() - pos < 28 {
            return Err(err_corrupt("trailing partial block header"));
        }

        let body_size = i32::from_le_bytes(input[pos + 16..pos + 20].try_into().unwrap());
        if body_size < 0 {
            return Err(err_corrupt("negative body_size"));
        }

        let total = 28usize
            .checked_add(body_size as usize)
            .ok_or_else(|| err_corrupt("block size overflow"))?;
        if pos + total > input.len() {
            return Err(err_corrupt("block body out of bounds"));
        }

        let block = decompress_block(&input[pos..pos + total])?;
        output.extend_from_slice(&block);
        pos += total;
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// GrzipCodec — implements Codec trait with native decompression
// ---------------------------------------------------------------------------

pub struct GrzipCodec {
    #[allow(dead_code)]
    options: GrzipOptions,
}

impl GrzipCodec {
    pub fn new(options: GrzipOptions) -> Self {
        Self { options }
    }
}

impl Codec for GrzipCodec {
    fn name(&self) -> &'static str {
        "grzip"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        #[cfg(feature = "ffi-codecs")]
        {
            let mut source = Vec::new();
            input.read_to_end(&mut source)?;
            let uncompressed_len = source.len();

            let compressed = crate::codecs::grzip::grzip_compress(&source, self.options.mode)
                .map_err(|e| ArcError::Codec {
                    codec: "grzip",
                    message: e.to_string(),
                })?;

            framing::write_size_header(uncompressed_len, output)?;
            output.write_all(&compressed)?;

            return Ok(CodecReport {
                bytes_in: uncompressed_len as u64,
                bytes_out: (framing::SIZE_HEADER_LEN + compressed.len()) as u64,
            });
        }

        #[allow(unreachable_code)]
        Err(ArcError::Codec {
            codec: "grzip",
            message: "compression requires the ffi-codecs feature".into(),
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let expected_len = framing::read_size_header(input)?;
        let mut compressed = Vec::new();
        input.read_to_end(&mut compressed)?;
        let bytes_in = (framing::SIZE_HEADER_LEN + compressed.len()) as u64;

        let decompressed = decompress_block(&compressed).map_err(|e| ArcError::Codec {
            codec: "grzip",
            message: e.to_string(),
        })?;

        if decompressed.len() != expected_len {
            return Err(ArcError::Codec {
                codec: "grzip",
                message: format!(
                    "decompressed {} bytes but header expected {}",
                    decompressed.len(),
                    expected_len
                ),
            });
        }

        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: decompressed.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: 8 * 1024 * 1024 * 5,
            ..MemoryUsage::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_block(data: &[u8]) -> Vec<u8> {
        let mut block = Vec::with_capacity(28 + data.len());
        block.extend_from_slice(&(data.len() as i32).to_le_bytes());
        block.extend_from_slice(&(-1i32).to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&(data.len() as i32).to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(data);
        block
    }

    #[test]
    fn raw_stream_decodes_concatenated_stored_blocks() {
        let mut stream = stored_block(b"hello ");
        stream.extend_from_slice(&stored_block(b"world"));

        let decoded = decompress_stream(&stream).unwrap();
        assert_eq!(decoded, b"hello world");
    }
}
