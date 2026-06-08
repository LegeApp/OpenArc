use crate::error::Result;

pub trait Filter {
    fn name(&self) -> &'static str;
    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()>;
    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()>;
}
