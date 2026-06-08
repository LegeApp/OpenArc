use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

// ── Options ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispackOptions {
    pub min_executable_score: u8,
    /// Assumed load address for the first byte of input (default: 0x00401000).
    pub origin: u32,
}

impl Default for DispackOptions {
    fn default() -> Self {
        Self {
            min_executable_score: 64,
            origin: 0x0040_1000,
        }
    }
}

// ── Instruction format flags ──────────────────────────────────────────────────

const F_NM: u8 = 0x0; // no ModRM
const F_AM: u8 = 0x1; // address mode (jumps/calls)
const F_MR: u8 = 0x2; // ModRM present
const F_MEXTRA: u8 = 0x3; // ModRM + opcode extension in reg field
const F_MODE: u8 = 0x3; // mode mask

const F_NI: u8 = 0x00; // no immediate
const F_BI: u8 = 0x04; // byte immediate
const F_WI: u8 = 0x08; // word immediate
const F_DI: u8 = 0x0c; // dword immediate
const F_TYPE: u8 = 0x0c; // type mask

const F_AD: u8 = 0x00; // absolute dword address
const F_DA: u8 = 0x04; // dword absolute jump target (JMPF / CALLF variant)
const F_BR: u8 = 0x08; // byte relative
const F_DR: u8 = 0x0c; // dword relative

const F_ERR: u8 = 0x0f; // invalid opcode

// ── Special opcodes ───────────────────────────────────────────────────────────

const OP_2BYTE: u8 = 0x0f;
const OP_OSIZE: u8 = 0x66;
const OP_CALLF: u8 = 0x9a;
const OP_RETNI: u8 = 0xc2;
const OP_RETN: u8 = 0xc3;
const OP_ENTER: u8 = 0xc8;
const OP_INT3: u8 = 0xcc;
const OP_CALLN: u8 = 0xe8;
const OP_JMPF: u8 = 0xea;

const ESCAPE: u8 = 0xf1; // OP_ICEBP — encapsulates unrecognized bytes
const JUMPTAB: u8 = 0xce; // OP_INTO — marks a jump/vtable block

// ── Stream indices ────────────────────────────────────────────────────────────

const ST_MAX: usize = 19;
const ST_OP: usize = 0; // opcodes, ModRM, second opcode byte, jumptable count
const ST_SIB: usize = 1;
const ST_CALL_IDX: usize = 2; // MTF index for call/jmptab targets
const ST_DISP8_R0: usize = 3; // 8-bit displacement, one stream per base register
                              // ST_DISP8_R1..R7 = 4..10
const ST_JUMP8: usize = 11;
const ST_IMM8: usize = 12;
const ST_IMM16: usize = 13;
const ST_IMM32: usize = 14;
const ST_DISP32: usize = 15;
const ST_ADDR32: usize = 16; // direct absolute address
const ST_CALL32: usize = 17; // call target (absolute)
const ST_JUMP32: usize = 18; // jump target (absolute)

// Aliases that share a stream
const ST_MODRM: usize = ST_OP;
const ST_OP2: usize = ST_OP;
const ST_AJUMP32: usize = ST_JUMP32;
const ST_JUMPTBL_COUNT: usize = ST_OP;

const HEADER_SIZE: usize = ST_MAX * 4;
const MAXINSTR: usize = 15;

// ── Opcode tables (ported directly from DisPack.cpp) ─────────────────────────

#[rustfmt::skip]
static TABLE1: [u8; 256] = [
    // row 0x0x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    // row 0x1x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    // row 0x2x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    // row 0x3x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI,
    // row 0x4x
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0x5x
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0x6x
    F_NM|F_NI, F_NM|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    F_NM|F_DI, F_MR|F_DI, F_NM|F_BI, F_MR|F_BI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0x7x — Jcc short
    F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR,
    F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR,
    // row 0x8x
    F_MR|F_BI, F_MR|F_DI, F_MR|F_BI, F_MR|F_BI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0x9x
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    F_NM|F_NI, F_NM|F_NI, F_AM|F_DA, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0xax
    F_AM|F_AD, F_AM|F_AD, F_AM|F_AD, F_AM|F_AD, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    F_NM|F_BI, F_NM|F_DI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0xbx
    F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI,
    F_NM|F_DI, F_NM|F_DI, F_NM|F_DI, F_NM|F_DI, F_NM|F_DI, F_NM|F_DI, F_NM|F_DI, F_NM|F_DI,
    // row 0xcx
    F_MR|F_BI, F_MR|F_BI, F_NM|F_WI, F_NM|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_BI, F_MR|F_DI,
    F_NM|F_BI, F_NM|F_NI, F_NM|F_WI, F_NM|F_NI, F_NM|F_NI, F_NM|F_BI, F_ERR,     F_NM|F_NI,
    // row 0xdx
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_BI, F_NM|F_BI, F_NM|F_NI, F_NM|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0xex
    F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_AM|F_BR, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI, F_NM|F_BI,
    F_AM|F_DR, F_AM|F_DR, F_AM|F_AD, F_AM|F_BR, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0xfx
    F_NM|F_NI, F_ERR,     F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_MEXTRA,  F_MEXTRA,
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_MEXTRA,  F_MEXTRA,
];

#[rustfmt::skip]
static TABLE2: [u8; 256] = [
    // row 0x0x
    F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_NM|F_NI, F_ERR,
    F_NM|F_NI, F_NM|F_NI, F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,
    // row 0x1x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,
    // row 0x2x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_ERR,     F_ERR,     F_ERR,     F_ERR,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0x3x
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_ERR,     F_NM|F_NI,
    F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,
    // row 0x4x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0x5x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0x6x
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0x7x
    F_MR|F_BI, F_MR|F_BI, F_MR|F_BI, F_MR|F_BI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_NM|F_NI,
    F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_MR|F_NI, F_MR|F_NI,
    // row 0x8x — Jcc near
    F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR,
    F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR, F_AM|F_DR,
    // row 0x9x — SETcc
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0xax
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_MR|F_NI, F_MR|F_BI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_ERR,     F_ERR,     F_ERR,     F_MR|F_NI, F_MR|F_BI, F_MR|F_NI, F_ERR,     F_MR|F_NI,
    // row 0xbx
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_ERR,     F_ERR,     F_ERR,     F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0xcx
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI, F_NM|F_NI,
    // row 0xdx
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0xex
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // row 0xfx
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_ERR,
];

// Extensions for 0xf6, 0xf7, 0xfe, 0xff (reg field selects variant)
#[rustfmt::skip]
static TABLEX: [u8; 32] = [
    // 0xf6: /0=TEST r/m8,imm8; /2..7=no imm
    F_MR|F_BI, F_ERR,     F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // 0xf7: /0=TEST r/m32,imm32
    F_MR|F_DI, F_ERR,     F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_MR|F_NI,
    // 0xfe: /0=INC /1=DEC r/m8; /2..7=invalid
    F_MR|F_NI, F_MR|F_NI, F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,     F_ERR,
    // 0xff: /0=INC /1=DEC /2=CALLN /3=CALLF /4=JMP /5=JMPF /6=PUSH; /7=invalid
    F_MR|F_NI, F_MR|F_NI, F_MR|F_NI, F_ERR,     F_MR|F_NI, F_ERR,     F_MR|F_NI, F_ERR,
];

// ── MTF helpers ───────────────────────────────────────────────────────────────

/// Shift entries [0..pos] right by 1, place `val` at [0]. Returns `val`.
#[inline]
fn mtf_move_to_front(table: &mut [u32; 256], pos: usize, val: u32) -> u32 {
    for i in (1..=pos).rev() {
        table[i] = table[i - 1];
    }
    table[0] = val;
    val
}

/// Insert `val` at the front (evicts table[255]).
#[inline]
fn mtf_add(table: &mut [u32; 256], val: u32) {
    mtf_move_to_front(table, 255, val);
}

/// Search table[0..255) for `val`. If found, move to front and return index.
/// If not found, add to front and return -1.
fn mtf_find(table: &mut [u32; 256], val: u32) -> i32 {
    for i in 0..255usize {
        if table[i] == val {
            mtf_move_to_front(table, i, val);
            return i as i32;
        }
    }
    mtf_add(table, val);
    -1
}

// ── Encoder context ───────────────────────────────────────────────────────────

struct EncCtx {
    bufs: [Vec<u8>; ST_MAX],
    func_table: [u32; 256],
    next_is_func: bool,
    code_start: u32,
    code_end: u32,
}

impl EncCtx {
    fn new(origin: u32, size: u32) -> Self {
        Self {
            bufs: std::array::from_fn(|_| Vec::new()),
            func_table: [0u32; 256],
            next_is_func: true,
            code_start: origin,
            code_end: origin + size,
        }
    }

    fn put8(&mut self, st: usize, v: u8) {
        self.bufs[st].push(v);
    }
    fn put16be(&mut self, st: usize, v: u16) {
        self.bufs[st].extend_from_slice(&v.to_be_bytes());
    }
    fn put32be(&mut self, st: usize, v: u32) {
        self.bufs[st].extend_from_slice(&v.to_be_bytes());
    }

    fn copy8(&mut self, st: usize, src: &[u8], p: &mut usize) {
        self.put8(st, src[*p]);
        *p += 1;
    }
    fn copy16be(&mut self, st: usize, src: &[u8], p: &mut usize) {
        let v = u16::from_le_bytes([src[*p], src[*p + 1]]);
        self.put16be(st, v);
        *p += 2;
    }
    fn copy32be(&mut self, st: usize, src: &[u8], p: &mut usize) {
        let v = u32::from_le_bytes([src[*p], src[*p + 1], src[*p + 2], src[*p + 3]]);
        self.put32be(st, v);
        *p += 4;
    }

    fn snapshot(&self) -> [usize; ST_MAX] {
        std::array::from_fn(|i| self.bufs[i].len())
    }
    fn restore(&mut self, snap: &[usize; ST_MAX]) {
        for i in 0..ST_MAX {
            self.bufs[i].truncate(snap[i]);
        }
    }

    fn flush(self) -> Vec<u8> {
        let body: usize = self.bufs.iter().map(|b| b.len()).sum();
        let mut out = Vec::with_capacity(HEADER_SIZE + body);
        for b in &self.bufs {
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        }
        for b in self.bufs.iter() {
            out.extend_from_slice(b);
        }
        out
    }
}

// ── Jump-table detector ───────────────────────────────────────────────────────

fn detect_jump_table(src: &[u8], pos: usize, code_start: u32, code_end: u32) -> usize {
    let addr = code_start + pos as u32;
    let max_by_range = ((code_end - addr) / 4) as usize;
    let max_by_src = (src.len() - pos) / 4;
    let n_max = max_by_range.min(max_by_src);
    let mut count = 0;
    while count < n_max {
        let off = pos + count * 4;
        let a = u32::from_le_bytes([src[off], src[off + 1], src[off + 2], src[off + 3]]);
        if a >= code_start && a < code_end {
            count += 1;
        } else {
            break;
        }
    }
    if count >= 3 {
        count
    } else {
        0
    }
}

// ── Process one instruction (encoder) ────────────────────────────────────────
//
// Returns bytes consumed from `src[pos..]`. Assumes the slice is at least
// MAXINSTR bytes long (caller pads the tail).

fn process_instr(ctx: &mut EncCtx, src: &[u8], src_pos: usize, memory: u32) -> usize {
    // Jump-table detection
    let nj = detect_jump_table(src, src_pos, ctx.code_start, ctx.code_end);
    if nj > 0 {
        let mut p = src_pos;
        let mut remaining = nj;
        while remaining > 0 {
            let count = remaining.min(256);
            ctx.put8(ST_OP, JUMPTAB);
            ctx.put8(ST_JUMPTBL_COUNT, (count - 1) as u8);
            for _ in 0..count {
                let target = u32::from_le_bytes([src[p], src[p + 1], src[p + 2], src[p + 3]]);
                p += 4;
                let ind = mtf_find(&mut ctx.func_table, target);
                ctx.put8(ST_CALL_IDX, (ind + 1) as u8);
                if ind < 0 {
                    ctx.put32be(ST_CALL32, target);
                }
            }
            remaining -= count;
        }
        return nj * 4;
    }

    let start = src_pos;
    let mut p = src_pos;
    let first_byte = src[p];
    p += 1;

    if ctx.next_is_func && first_byte != OP_INT3 {
        mtf_add(&mut ctx.func_table, memory);
        ctx.next_is_func = false;
    }

    // Optional operand-size prefix
    let (code, o16) = if first_byte == OP_OSIZE {
        (src[p], true)
    } else {
        (first_byte, false)
    };
    if o16 {
        p += 1;
    }

    // Decode opcode
    let (flags_raw, code2) = if code == OP_2BYTE {
        let c2 = src[p];
        p += 1;
        (TABLE2[c2 as usize], c2)
    } else {
        (TABLE1[code as usize], 0)
    };

    if code == OP_RETNI || code == OP_RETN || code == OP_INT3 {
        ctx.next_is_func = true;
    }

    // MEXTRA: peek at ModRM to resolve actual flags (without consuming yet)
    let flags = if flags_raw == F_MEXTRA {
        let modrm = src[p]; // peek — p not advanced
        let idx = ((modrm >> 3) & 7) as usize
            | ((code as usize & 0x01) << 3)
            | ((code as usize & 0x08) << 1);
        TABLEX[idx]
    } else {
        flags_raw
    };

    if flags == F_ERR {
        // Unrecognized — escape the raw byte
        ctx.put8(ST_OP, ESCAPE);
        ctx.put8(ST_OP, src[start]);
        return 1;
    }

    // Recognized instruction — route bytes to streams
    if o16 {
        ctx.put8(ST_OP, OP_OSIZE);
    }
    ctx.put8(ST_OP, code);
    if code == OP_2BYTE {
        ctx.put8(ST_OP2, code2);
    }

    // Far call/jump and ENTER have an extra 16-bit segment/word before main operand
    if code == OP_CALLF || code == OP_JMPF || code == OP_ENTER {
        ctx.copy16be(ST_IMM16, src, &mut p);
    }

    // ModRM-based instructions
    if (flags & F_MODE) == F_MR {
        let modrm = src[p];
        p += 1;
        ctx.put8(ST_MODRM, modrm);

        let sib = if (modrm & 0x07) == 4 && modrm < 0xc0 {
            let s = src[p];
            p += 1;
            ctx.put8(ST_SIB, s);
            s
        } else {
            0
        };

        if (modrm & 0xc0) == 0x40 {
            let st = ST_DISP8_R0 + (modrm & 0x07) as usize;
            ctx.copy8(st, src, &mut p);
        }

        if (modrm & 0xc0) == 0x80 || (modrm & 0xc7) == 0x05 || (modrm < 0x40 && (sib & 0x07) == 5) {
            let st = if (modrm & 0xc7) == 0x05 {
                ST_ADDR32
            } else {
                ST_DISP32
            };
            ctx.copy32be(st, src, &mut p);
        }
    }

    // Address-mode or immediate operands
    if (flags & F_MODE) == F_AM {
        match flags & F_TYPE {
            F_AD => ctx.copy32be(ST_ADDR32, src, &mut p),
            F_DA => ctx.copy32be(ST_AJUMP32, src, &mut p),
            F_BR => ctx.copy8(ST_JUMP8, src, &mut p),
            F_DR => {
                let rel = u32::from_le_bytes([src[p], src[p + 1], src[p + 2], src[p + 3]]);
                p += 4;
                // Convert relative → absolute
                let instr_size = p - start;
                let abs = rel.wrapping_add(instr_size as u32).wrapping_add(memory);
                if code == OP_CALLN {
                    let ind = mtf_find(&mut ctx.func_table, abs);
                    ctx.put8(ST_CALL_IDX, (ind + 1) as u8);
                    if ind < 0 {
                        ctx.put32be(ST_CALL32, abs);
                    }
                } else {
                    ctx.put32be(ST_JUMP32, abs);
                }
            }
            _ => {}
        }
    } else {
        match flags & F_TYPE {
            F_BI => ctx.copy8(ST_IMM8, src, &mut p),
            F_WI => ctx.copy16be(ST_IMM16, src, &mut p),
            F_DI => {
                if !o16 {
                    ctx.copy32be(ST_IMM32, src, &mut p);
                } else {
                    ctx.copy16be(ST_IMM16, src, &mut p);
                }
            }
            _ => {}
        }
    }

    p - start
}

// ── dis_filter (encoder) ─────────────────────────────────────────────────────

pub fn dis_filter(src: &[u8], origin: u32) -> Vec<u8> {
    let size = src.len();
    let mut ctx = EncCtx::new(origin, size as u32);
    let mut pos = 0;

    // Main body: stop MAXINSTR bytes before the end to avoid out-of-bounds peeks
    while pos + MAXINSTR < size {
        let mem = ctx.code_start + pos as u32;
        let bytes = process_instr(&mut ctx, src, pos, mem);
        pos += bytes;
    }

    // Tail: pad to MAXINSTR and use checkpoint restore if we'd read past end
    while pos < size {
        let mut buf = [0u8; MAXINSTR];
        let avail = size - pos;
        buf[..avail].copy_from_slice(&src[pos..]);

        let snap = ctx.snapshot();
        let mem = ctx.code_start + pos as u32;
        let bytes = process_instr(&mut ctx, &buf, 0, mem);

        if bytes <= avail {
            pos += bytes;
        } else {
            ctx.restore(&snap);
            break;
        }
    }

    // Any remaining bytes get escaped individually
    while pos < size {
        ctx.put8(ST_OP, ESCAPE);
        ctx.put8(ST_OP, src[pos]);
        pos += 1;
    }

    ctx.flush()
}

// ── dis_unfilter (decoder) ───────────────────────────────────────────────────

fn codec_err(msg: &'static str) -> ArcError {
    ArcError::Codec {
        codec: "dispack",
        message: msg.to_string(),
    }
}

/// Helper: read 2 bytes big-endian from `src[sp..]`, write little-endian to `out`.
#[inline]
fn copy16_be_le(src: &[u8], sp: &mut usize, end: usize, out: &mut Vec<u8>) -> Result<()> {
    if *sp + 2 > end {
        return Err(codec_err("truncated 16-bit field"));
    }
    let v = u16::from_be_bytes([src[*sp], src[*sp + 1]]);
    out.extend_from_slice(&v.to_le_bytes());
    *sp += 2;
    Ok(())
}

/// Helper: read 4 bytes big-endian from `src[sp..]`, write little-endian to `out`.
#[inline]
fn copy32_be_le(src: &[u8], sp: &mut usize, end: usize, out: &mut Vec<u8>) -> Result<()> {
    if *sp + 4 > end {
        return Err(codec_err("truncated 32-bit field"));
    }
    let v = u32::from_be_bytes([src[*sp], src[*sp + 1], src[*sp + 2], src[*sp + 3]]);
    out.extend_from_slice(&v.to_le_bytes());
    *sp += 4;
    Ok(())
}

/// Read 4 bytes big-endian as u32 without writing to output.
#[inline]
fn fetch32be(src: &[u8], sp: &mut usize, end: usize) -> Result<u32> {
    if *sp + 4 > end {
        return Err(codec_err("truncated 32-bit value"));
    }
    let v = u32::from_be_bytes([src[*sp], src[*sp + 1], src[*sp + 2], src[*sp + 3]]);
    *sp += 4;
    Ok(v)
}

pub fn dis_unfilter(encoded: &[u8], output: &mut Vec<u8>, mem_start: u32) -> Result<()> {
    if encoded.len() < HEADER_SIZE {
        return Err(codec_err("header too short"));
    }

    // Parse header: ST_MAX × u32 LE stream sizes
    let mut sp = [0usize; ST_MAX]; // stream read cursors
    let mut se = [0usize; ST_MAX]; // stream end positions
    let mut cur = HEADER_SIZE;
    for i in 0..ST_MAX {
        let sz = u32::from_le_bytes([
            encoded[i * 4],
            encoded[i * 4 + 1],
            encoded[i * 4 + 2],
            encoded[i * 4 + 3],
        ]) as usize;
        sp[i] = cur;
        cur += sz;
        se[i] = cur;
    }
    if cur != encoded.len() {
        return Err(codec_err("stream size mismatch"));
    }

    let mut func_table = [0u32; 256];
    let mut next_is_func = true;
    let dest_start = output.len();

    macro_rules! chk {
        ($st:expr, $n:expr) => {
            if sp[$st] + $n > se[$st] {
                return Err(codec_err("stream underrun"));
            }
        };
    }
    macro_rules! fetch8 {
        ($st:expr) => {{
            chk!($st, 1);
            let v = encoded[sp[$st]];
            sp[$st] += 1;
            v
        }};
    }

    while sp[ST_OP] < se[ST_OP] {
        let instr_start = output.len();
        let memory = mem_start + (output.len() - dest_start) as u32;

        let code = fetch8!(ST_OP);

        // ── Jump table ────────────────────────────────────────────────────────
        if code == JUMPTAB {
            let count = fetch8!(ST_JUMPTBL_COUNT) as usize + 1;
            for _ in 0..count {
                let ind = fetch8!(ST_CALL_IDX) as usize;
                let target = if ind != 0 {
                    let v = func_table[ind - 1];
                    mtf_move_to_front(&mut func_table, ind - 1, v)
                } else {
                    let t = fetch32be(encoded, &mut sp[ST_CALL32], se[ST_CALL32])?;
                    mtf_add(&mut func_table, t);
                    t
                };
                output.extend_from_slice(&target.to_le_bytes());
            }
            continue;
        }

        if next_is_func && code != OP_INT3 {
            mtf_add(&mut func_table, memory);
            next_is_func = false;
        }

        // ── Escape (raw byte) ─────────────────────────────────────────────────
        if code == ESCAPE {
            chk!(ST_OP, 1);
            output.push(encoded[sp[ST_OP]]);
            sp[ST_OP] += 1;
            continue;
        }

        // ── Normal instruction ────────────────────────────────────────────────
        output.push(code);

        let (code_main, o16) = if code == OP_OSIZE {
            chk!(ST_OP, 1);
            let c2 = encoded[sp[ST_OP]];
            sp[ST_OP] += 1;
            output.push(c2);
            (c2, true)
        } else {
            (code, false)
        };

        if code_main == OP_RETNI || code_main == OP_RETN || code_main == OP_INT3 {
            next_is_func = true;
        }

        let mut flags = if code_main == OP_2BYTE {
            chk!(ST_OP2, 1); // ST_OP2 == ST_OP
            let c2 = encoded[sp[ST_OP2]];
            sp[ST_OP2] += 1;
            output.push(c2);
            TABLE2[c2 as usize]
        } else {
            TABLE1[code_main as usize]
        };

        if code_main == OP_CALLF || code_main == OP_JMPF || code_main == OP_ENTER {
            copy16_be_le(encoded, &mut sp[ST_IMM16], se[ST_IMM16], output)?;
        }

        // ModRM-bearing instructions (fMR=2 and fMEXTRA=3 both have bit 1 set)
        if flags & F_MR != 0 {
            chk!(ST_MODRM, 1);
            let modrm = encoded[sp[ST_MODRM]];
            sp[ST_MODRM] += 1;
            output.push(modrm);

            // MEXTRA: resolve actual flags from ModRM reg field
            if flags == F_MEXTRA {
                let idx = ((modrm >> 3) & 7) as usize
                    | ((code_main as usize & 0x01) << 3)
                    | ((code_main as usize & 0x08) << 1);
                flags = TABLEX[idx];
            }

            let sib = if (modrm & 0x07) == 4 && modrm < 0xc0 {
                chk!(ST_SIB, 1);
                let s = encoded[sp[ST_SIB]];
                sp[ST_SIB] += 1;
                output.push(s);
                s
            } else {
                0
            };

            if (modrm & 0xc0) == 0x40 {
                let st = ST_DISP8_R0 + (modrm & 0x07) as usize;
                chk!(st, 1);
                output.push(encoded[sp[st]]);
                sp[st] += 1;
            }

            if (modrm & 0xc0) == 0x80
                || (modrm & 0xc7) == 0x05
                || (modrm < 0x40 && (sib & 0x07) == 0x05)
            {
                let st = if (modrm & 0xc7) == 0x05 {
                    ST_ADDR32
                } else {
                    ST_DISP32
                };
                copy32_be_le(encoded, &mut sp[st], se[st], output)?;
            }
        }

        if (flags & F_MODE) == F_AM {
            match flags & F_TYPE {
                F_AD => copy32_be_le(encoded, &mut sp[ST_ADDR32], se[ST_ADDR32], output)?,
                F_DA => copy32_be_le(encoded, &mut sp[ST_AJUMP32], se[ST_AJUMP32], output)?,
                F_BR => {
                    chk!(ST_JUMP8, 1);
                    output.push(encoded[sp[ST_JUMP8]]);
                    sp[ST_JUMP8] += 1;
                }
                F_DR => {
                    let target = if code_main == OP_CALLN {
                        let ind = fetch8!(ST_CALL_IDX) as usize;
                        if ind != 0 {
                            let v = func_table[ind - 1];
                            mtf_move_to_front(&mut func_table, ind - 1, v)
                        } else {
                            let t = fetch32be(encoded, &mut sp[ST_CALL32], se[ST_CALL32])?;
                            mtf_add(&mut func_table, t);
                            t
                        }
                    } else {
                        fetch32be(encoded, &mut sp[ST_JUMP32], se[ST_JUMP32])?
                    };

                    // Convert absolute → relative:
                    // rel = target - (memory + bytes_written_so_far + 4)
                    let written = (output.len() - instr_start) as u32;
                    let rel = target
                        .wrapping_sub(memory)
                        .wrapping_sub(written)
                        .wrapping_sub(4);
                    output.extend_from_slice(&rel.to_le_bytes());
                }
                _ => {}
            }
        } else {
            match flags & F_TYPE {
                F_BI => {
                    chk!(ST_IMM8, 1);
                    output.push(encoded[sp[ST_IMM8]]);
                    sp[ST_IMM8] += 1;
                }
                F_WI => copy16_be_le(encoded, &mut sp[ST_IMM16], se[ST_IMM16], output)?,
                F_DI => {
                    if !o16 {
                        copy32_be_le(encoded, &mut sp[ST_IMM32], se[ST_IMM32], output)?;
                    } else {
                        copy16_be_le(encoded, &mut sp[ST_IMM16], se[ST_IMM16], output)?;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

// ── DispackFilter ─────────────────────────────────────────────────────────────

pub struct DispackFilter {
    opts: DispackOptions,
}

impl DispackFilter {
    pub fn new(opts: DispackOptions) -> Self {
        Self { opts }
    }
}

impl Filter for DispackFilter {
    fn name(&self) -> &'static str {
        "dispack"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let encoded = dis_filter(input, self.opts.origin);
        output.extend_from_slice(&encoded);
        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        dis_unfilter(input, output, self.opts.origin)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{dis_filter, dis_unfilter};

    const DEFAULT_EXE_PATH: &str = r"D:\Rust-projects\SinoRAG\target\release\sinorag.exe";
    const ORIGIN: u32 = 0x0040_1000;

    fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
        a.iter()
            .zip(b.iter())
            .position(|(left, right)| left != right)
            .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
    }

    fn describe_window(bytes: &[u8], pos: usize) -> String {
        let start = pos.saturating_sub(16);
        let end = (pos + 16).min(bytes.len());
        format!("{:02x?}", &bytes[start..end])
    }

    fn roundtrip(input: &[u8]) -> Vec<u8> {
        let encoded = dis_filter(input, ORIGIN);
        let mut decoded = Vec::with_capacity(input.len());
        dis_unfilter(&encoded, &mut decoded, ORIGIN).expect("dispack decode");
        decoded
    }

    #[test]
    fn dispack_roundtrips_mixed_x86_forms() {
        let mut input = Vec::new();
        input.extend_from_slice(&[0x90; 16]);
        input.extend_from_slice(&[0xe8, 0x78, 0x56, 0x34, 0x12]);
        input.extend_from_slice(&[0x0f, 0x85, 0x11, 0x22, 0x33, 0x44]);
        input.extend_from_slice(&[0x66, 0xb8, 0xcd, 0xab]);
        input.extend_from_slice(&[0x8b, 0x84, 0x24, 0x20, 0x00, 0x00, 0x00]);
        input.extend_from_slice(&[0xc3, 0xcc, 0x90]);

        let decoded = roundtrip(&input);
        assert_eq!(decoded, input);
    }

    #[test]
    fn dispack_escape_keeps_jump_table_mtf_in_sync() {
        let mut input = vec![super::ESCAPE];
        input.extend_from_slice(&ORIGIN.to_le_bytes());
        input.extend_from_slice(&(ORIGIN + 4).to_le_bytes());
        input.extend_from_slice(&(ORIGIN + 8).to_le_bytes());

        let decoded = roundtrip(&input);
        assert_eq!(decoded, input);
    }

    #[test]
    #[ignore = "set ARCMAX_DISPACK_ROUNDTRIP_PATH or keep the local sinorag.exe path to reproduce large EXE Dispack failures"]
    fn dispack_roundtrips_real_exe() {
        let path = std::env::var("ARCMAX_DISPACK_ROUNDTRIP_PATH")
            .or_else(|_| std::env::var("ARCMAX_EXE_BENCH_PATH"))
            .unwrap_or_else(|_| DEFAULT_EXE_PATH.to_string());
        let input = std::fs::read(&path).expect("read EXE fixture");

        let decoded = roundtrip(&input);
        if let Some(pos) = first_mismatch(&decoded, &input) {
            panic!(
                "dispack roundtrip mismatch at byte {pos}: decoded_window={}, expected_window={}",
                describe_window(&decoded, pos),
                describe_window(&input, pos)
            );
        }
    }
}
