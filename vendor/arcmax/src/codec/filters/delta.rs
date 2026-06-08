use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaOptions {
    pub stride: usize,
}

impl Default for DeltaOptions {
    fn default() -> Self {
        Self { stride: 1 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeltaFilter {
    options: DeltaOptions,
}

impl DeltaFilter {
    pub fn new(options: DeltaOptions) -> Result<Self> {
        validate_stride(options.stride)?;
        Ok(Self { options })
    }

    pub fn options(&self) -> DeltaOptions {
        self.options
    }
}

impl Filter for DeltaFilter {
    fn name(&self) -> &'static str {
        "delta"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        validate_stride(self.options.stride)?;
        output.reserve(input.len());

        for i in 0..input.len() {
            let b = if i < self.options.stride {
                input[i]
            } else {
                input[i].wrapping_sub(input[i - self.options.stride])
            };
            output.push(b);
        }

        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        validate_stride(self.options.stride)?;
        let start = output.len();
        output.reserve(input.len());

        for (i, &encoded) in input.iter().enumerate() {
            let b = if i < self.options.stride {
                encoded
            } else {
                encoded.wrapping_add(output[start + i - self.options.stride])
            };
            output.push(b);
        }

        Ok(())
    }
}

fn validate_stride(stride: usize) -> Result<()> {
    if stride == 0 {
        return Err(ArcError::InvalidMethod(
            "delta stride must be greater than zero".to_string(),
        ));
    }

    Ok(())
}
