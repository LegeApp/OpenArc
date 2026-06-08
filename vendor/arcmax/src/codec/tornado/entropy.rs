use crate::error::Result;

pub trait SymbolDecoder {
    fn decode_symbol(&mut self) -> Result<u32>;
}

#[derive(Debug, Default)]
pub struct EntropyScaffold;

impl SymbolDecoder for EntropyScaffold {
    fn decode_symbol(&mut self) -> Result<u32> {
        Ok(0)
    }
}
