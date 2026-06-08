use crate::codec::tornado::format::TableKind;
use crate::error::{ArcError, Result};

pub const REP_DIST_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LzCommand {
    Literal(u8),
    Match { len: usize, dist: usize },
    Table { kind: TableKind, len: usize },
    End,
}

#[derive(Debug, Clone)]
pub struct LzWindow {
    buf: Vec<u8>,
    pos: usize,
    produced: u64,
}

impl LzWindow {
    pub fn new(size: usize) -> Self {
        Self {
            buf: vec![0; size.max(1)],
            pos: 0,
            produced: 0,
        }
    }

    pub fn produced(&self) -> u64 {
        self.produced
    }

    pub fn push_literal<W: std::io::Write>(&mut self, b: u8, out: &mut W) -> Result<()> {
        out.write_all(&[b])?;
        self.buf[self.pos] = b;
        self.pos = (self.pos + 1) % self.buf.len();
        self.produced += 1;
        Ok(())
    }

    pub fn copy_match<W: std::io::Write>(
        &mut self,
        dist: usize,
        len: usize,
        out: &mut W,
    ) -> Result<()> {
        if dist == 0 || dist > self.buf.len() {
            return Err(ArcError::Codec {
                codec: "tornado",
                message: format!("invalid match distance: {dist}"),
            });
        }

        for _ in 0..len {
            let src = (self.pos + self.buf.len() - dist) % self.buf.len();
            let b = self.buf[src];
            self.push_literal(b, out)?;
        }

        Ok(())
    }
}
