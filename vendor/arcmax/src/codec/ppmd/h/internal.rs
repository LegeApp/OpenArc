pub(crate) use super::tagged_offset::*;

pub(crate) const PPMD_INT_BITS: u32 = 7;
pub(crate) const PPMD_PERIOD_BITS: u32 = 7;
pub(crate) const PPMD_BIN_SCALE: u32 = 1 << (PPMD_INT_BITS + PPMD_PERIOD_BITS);

pub(crate) const fn ppmd_get_mean_spec(summ: u32, shift: u32, round: u32) -> u32 {
    (summ + (1 << (shift - round))) >> shift
}

pub(crate) const fn ppmd_get_mean(summ: u32) -> u32 {
    ppmd_get_mean_spec(summ, PPMD_PERIOD_BITS, 2)
}

pub(crate) const fn ppmd_update_prob_1(prob: u32) -> u32 {
    prob - ppmd_get_mean(prob)
}

pub(crate) const PPMD_N1: u32 = 4;
pub(crate) const PPMD_N2: u32 = 4;
pub(crate) const PPMD_N3: u32 = 4;
pub(crate) const PPMD_N4: u32 = (128 + 3 - PPMD_N1 - 2 * PPMD_N2 - 3 * PPMD_N3) / 4;
pub(crate) const PPMD_NUM_INDEXES: u32 = PPMD_N1 + PPMD_N2 + PPMD_N3 + PPMD_N4;

pub(crate) enum SeeSource {
    Dummy,
    Table(usize, usize),
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
pub(crate) struct See {
    pub(crate) summ: u16,
    pub(crate) shift: u8,
    pub(crate) count: u8,
}

impl See {
    #[inline(always)]
    pub(crate) fn update(&mut self) {
        if (self.shift as i32) < 7 && {
            self.count = self.count.wrapping_sub(1);
            self.count as i32 == 0
        } {
            self.summ = ((self.summ as i32) << 1) as u16;
            let fresh = self.shift;
            self.shift = self.shift.wrapping_add(1);
            self.count = (3 << fresh as i32) as u8;
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub(crate) struct State {
    pub(crate) symbol: u8,
    pub(crate) freq: u8,
    pub(crate) successor_0: u16,
    pub(crate) successor_1: u16,
}

impl Pointee for State {
    const TAG: u32 = TAG_STATE;
}

impl State {
    #[inline(always)]
    pub(crate) fn set_successor(&mut self, v: TaggedOffset) {
        let raw = v.as_raw();
        self.successor_0 = raw as u16;
        self.successor_1 = (raw >> 16) as u16;
    }

    #[inline(always)]
    pub(crate) fn get_successor(&self) -> TaggedOffset {
        let raw = self.successor_0 as u32 + ((self.successor_1 as u32) << 16);
        TaggedOffset::from_raw(raw)
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub(crate) struct State2 {
    pub(crate) symbol: u8,
    pub(crate) freq: u8,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub(crate) struct State4 {
    pub(crate) successor_0: u16,
    pub(crate) successor_1: u16,
}

impl State4 {
    #[inline(always)]
    pub(crate) fn get_successor(&self) -> TaggedOffset {
        let raw = self.successor_0 as u32 + ((self.successor_1 as u32) << 16);
        TaggedOffset::from_raw(raw)
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union Union2 {
    pub(crate) summ_freq: u16,
    pub(crate) state2: State2,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union Union4 {
    pub(crate) stats: TaggedOffset,
    pub(crate) state4: State4,
}
