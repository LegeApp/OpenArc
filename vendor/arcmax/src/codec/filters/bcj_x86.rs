use crate::codec::filters::Filter;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BcjX86Options;

#[derive(Debug, Clone, Copy, Default)]
pub struct BcjX86Filter {
    options: BcjX86Options,
}

impl BcjX86Filter {
    pub fn new(options: BcjX86Options) -> Self {
        Self { options }
    }

    pub fn options(&self) -> BcjX86Options {
        self.options
    }
}

impl Filter for BcjX86Filter {
    fn name(&self) -> &'static str {
        "exe"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        output.extend_from_slice(input);
        x86_convert(output.as_mut_slice(), 0, true);
        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let start = output.len();
        output.extend_from_slice(input);
        x86_convert(&mut output[start..], 0, false);
        Ok(())
    }
}

const MASK_TO_ALLOWED_STATUS: [u8; 8] = [1, 1, 1, 0, 1, 0, 0, 0];
const MASK_TO_BIT_NUMBER: [u8; 8] = [0, 1, 2, 2, 3, 3, 3, 3];

fn test_86_ms_byte(b: u8) -> bool {
    b == 0 || b == 0xFF
}

fn x86_convert(data: &mut [u8], ip: u32, encoding: bool) -> usize {
    let size = data.len();
    if size < 5 {
        return 0;
    }

    let mut buffer_pos = 0usize;
    let mut prev_pos = usize::MAX;
    let mut prev_mask = 0u32;
    let ip = ip.wrapping_add(5);

    loop {
        let limit = size - 4;
        while buffer_pos < limit && (data[buffer_pos] & 0xFE) != 0xE8 {
            buffer_pos += 1;
        }

        if buffer_pos >= limit {
            break;
        }

        let mut prev_pos_delta = buffer_pos.wrapping_sub(prev_pos);
        if prev_pos_delta > 3 {
            prev_mask = 0;
        } else {
            prev_mask = (prev_mask << (prev_pos_delta - 1)) & 0x7;
            if prev_mask != 0 {
                let index = 4 - MASK_TO_BIT_NUMBER[prev_mask as usize] as usize;
                let b = data[buffer_pos + index];
                if MASK_TO_ALLOWED_STATUS[prev_mask as usize] == 0 || test_86_ms_byte(b) {
                    prev_pos = buffer_pos;
                    prev_mask = ((prev_mask << 1) & 0x7) | 1;
                    buffer_pos += 1;
                    continue;
                }
            }
        }

        prev_pos = buffer_pos;

        if test_86_ms_byte(data[buffer_pos + 4]) {
            let mut src = u32::from_le_bytes([
                data[buffer_pos + 1],
                data[buffer_pos + 2],
                data[buffer_pos + 3],
                data[buffer_pos + 4],
            ]);
            let mut dest;
            loop {
                dest = if encoding {
                    ip.wrapping_add(buffer_pos as u32).wrapping_add(src)
                } else {
                    src.wrapping_sub(ip.wrapping_add(buffer_pos as u32))
                };

                if prev_mask == 0 {
                    break;
                }

                let index = MASK_TO_BIT_NUMBER[prev_mask as usize] as u32 * 8;
                let b = (dest >> (24 - index)) as u8;
                if !test_86_ms_byte(b) {
                    break;
                }

                src = dest ^ ((1u32 << (32 - index)) - 1);
            }

            data[buffer_pos + 4] = !(((dest >> 24) as u8 & 1).wrapping_sub(1));
            data[buffer_pos + 3] = (dest >> 16) as u8;
            data[buffer_pos + 2] = (dest >> 8) as u8;
            data[buffer_pos + 1] = dest as u8;
            buffer_pos += 5;
        } else {
            prev_mask = ((prev_mask << 1) & 0x7) | 1;
            buffer_pos += 1;
        }

        prev_pos_delta = buffer_pos.wrapping_sub(prev_pos);
        let _state = if prev_pos_delta > 3 {
            0
        } else {
            (prev_mask << (prev_pos_delta - 1)) & 0x7
        };
    }

    buffer_pos
}
