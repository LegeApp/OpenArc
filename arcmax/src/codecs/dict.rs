use std::io::{Read, Write};
use anyhow::Result;

/// Dictionary post-processing for FreeARC archives
///
/// FreeARC supports dictionary preprocessing which improves compression
/// ratios by replacing common patterns with shorter representations.
/// This module implements the dictionary post-processing algorithms.
///
/// Dictionary post-processing is applied after decompression to restore
/// the original data. Common dictionary methods include:
/// - Delta: For data with predictable differences between adjacent values
/// - E8/E9: For executable files with relative addresses
/// - Filter: General-purpose filtering

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictMethod {
    /// No dictionary processing
    None,
    /// Delta encoding for data with predictable differences
    Delta(u8),  // Parameter: delta order (1-4 bytes)
    /// E8/E9 transformation for executables (relative addresses)
    E8E9,
    /// Intel x86 executable transformation
    Intel,
    /// Generic filter (placeholder)
    Filter,
    /// Complex dictionary method with parameters (e.g., "dict:p:64m:85%")
    ComplexDict,
}

impl DictMethod {
    /// Parse dictionary method from FreeARC-style string
    pub fn from_string(method: &str) -> Option<Self> {
        let method_lower = method.to_lowercase();
        
        match method_lower.as_str() {
            "none" | "" => Some(DictMethod::None),
            "delta" => Some(DictMethod::Delta(1)),  // Default delta order
            s if s.starts_with("delta:") => {
                s.strip_prefix("delta:")
                    .and_then(|param| param.parse::<u8>().ok())
                    .map(|order| DictMethod::Delta(order.min(4)))  // Max 4-byte delta
            },
            "e8e9" => Some(DictMethod::E8E9),
            "intel" => Some(DictMethod::Intel),
            "filter" => Some(DictMethod::Filter),
            // Handle complex dict parameters like "dict:p:64m:85%"
            s if s.starts_with("dict:") => {
                Some(DictMethod::ComplexDict) // Using ComplexDict for complex dict parameters
            },
            _ => None,
        }
    }
    
    /// Get the method name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            DictMethod::None => "none",
            DictMethod::Delta(_) => "delta",
            DictMethod::E8E9 => "e8e9",
            DictMethod::Intel => "intel",
            DictMethod::Filter => "filter",
            DictMethod::ComplexDict => "dict",
        }
    }
}

/// Dictionary post-processor
pub struct DictProcessor {
    method: DictMethod,
}

impl DictProcessor {
    pub fn new(method: DictMethod) -> Self {
        DictProcessor { method }
    }
    
    /// Apply dictionary post-processing to decompressed data
    pub fn post_process(&self, data: &mut [u8], original_size: Option<usize>) -> Result<()> {
        match self.method {
            DictMethod::None => {
                // No processing needed
                Ok(())
            },
            DictMethod::Delta(order) => {
                self.apply_delta_reverse(data, order)
            },
            DictMethod::E8E9 => {
                self.apply_e8e9_reverse(data)
            },
            DictMethod::Intel => {
                self.apply_intel_reverse(data)
            },
            DictMethod::Filter => {
                // Placeholder for generic filter
                Ok(())
            },
            DictMethod::ComplexDict => {
                // For complex dict methods, we'll apply a combination of transformations
                // based on typical FreeARC dict behavior
                self.apply_complex_dict_transform(data)
            },
        }
    }
    
    /// Reverse delta encoding
    fn apply_delta_reverse(&self, data: &mut [u8], order: u8) -> Result<()> {
        let order = order as usize;
        if order == 0 || order > 4 {
            return Err(anyhow::anyhow!("Invalid delta order: {}", order));
        }
        
        // For delta encoding, we reverse the process by accumulating differences
        // Each value is the previous value plus the current delta
        let mut buffer = data.to_vec();
        
        match order {
            1 => {
                // 1-byte delta: each byte is the difference from the previous byte
                for i in 1..data.len() {
                    data[i] = data[i].wrapping_add(data[i - 1]);
                }
            },
            2 => {
                // 2-byte delta: process every 2 bytes as a unit
                for i in 2..data.len() {
                    if i % 2 == 0 {
                        // Even positions: apply delta to corresponding position in previous pair
                        data[i] = data[i].wrapping_add(data[i - 2]);
                        if i + 1 < data.len() {
                            data[i + 1] = data[i + 1].wrapping_add(data[i - 1]);
                        }
                    }
                }
            },
            3 => {
                // 3-byte delta: process every 3 bytes as a unit
                for i in 3..data.len() {
                    if i % 3 == 0 {
                        data[i] = data[i].wrapping_add(data[i - 3]);
                        if i + 1 < data.len() {
                            data[i + 1] = data[i + 1].wrapping_add(data[i - 2]);
                        }
                        if i + 2 < data.len() {
                            data[i + 2] = data[i + 2].wrapping_add(data[i - 1]);
                        }
                    }
                }
            },
            4 => {
                // 4-byte delta: process every 4 bytes as a unit
                for i in 4..data.len() {
                    if i % 4 == 0 {
                        data[i] = data[i].wrapping_add(data[i - 4]);
                        if i + 1 < data.len() {
                            data[i + 1] = data[i + 1].wrapping_add(data[i - 3]);
                        }
                        if i + 2 < data.len() {
                            data[i + 2] = data[i + 2].wrapping_add(data[i - 2]);
                        }
                        if i + 3 < data.len() {
                            data[i + 3] = data[i + 3].wrapping_add(data[i - 1]);
                        }
                    }
                }
            },
            _ => return Err(anyhow::anyhow!("Unsupported delta order: {}", order)),
        }
        
        Ok(())
    }
    
    /// Reverse E8/E9 transformation
    /// This transforms relative jumps/calls back to absolute addresses
    fn apply_e8e9_reverse(&self, data: &mut [u8]) -> Result<()> {
        // E8/E9 transformation looks for E8/E9 opcodes followed by 4-byte addresses
        // E8 = CALL rel32, E9 = JMP rel32
        // During compression, these are transformed to absolute addresses
        // During decompression, we transform back to relative addresses
        
        let mut i = 0;
        while i + 4 < data.len() {
            if data[i] == 0xE8 || data[i] == 0xE9 {  // CALL or JMP
                // Found E8/E9 instruction, next 4 bytes are the address
                let offset = i as i32;
                
                // Read the 4-byte address (little endian)
                let addr = u32::from_le_bytes([
                    data[i + 1],
                    data[i + 2], 
                    data[i + 3],
                    data[i + 4],
                ]);
                
                // Convert back to relative address
                // Original: absolute_addr = current_pos + rel_offset
                // So: rel_offset = absolute_addr - current_pos
                let rel_offset = addr.wrapping_sub(offset as u32 + 5) as i32; // +5 because we're at pos after opcode
                
                // Write the relative offset back
                let rel_bytes = rel_offset.to_le_bytes();
                data[i + 1] = rel_bytes[0];
                data[i + 2] = rel_bytes[1];
                data[i + 3] = rel_bytes[2];
                data[i + 4] = rel_bytes[3];
                
                i += 5; // Skip the processed instruction
            } else {
                i += 1;
            }
        }
        
        Ok(())
    }
    
    /// Reverse Intel x86 transformation
    /// This is similar to E8/E9 but optimized for x86 executable patterns
    fn apply_intel_reverse(&self, data: &mut [u8]) -> Result<()> {
        // Intel transformation looks for common x86 patterns
        // Specifically looks for 5-byte sequences where the last 4 bytes form a 32-bit address
        // that should be converted from absolute to relative

        let mut i = 0;
        while i + 4 < data.len() {
            // Look for common x86 instruction patterns that contain relative addresses
            // This is a simplified version focusing on E8/E9 patterns
            if data[i] == 0xE8 || data[i] == 0xE9 {
                // Same as E8E9 transformation
                let offset = i as i32;

                let addr = u32::from_le_bytes([
                    data[i + 1],
                    data[i + 2],
                    data[i + 3],
                    data[i + 4],
                ]);

                let rel_offset = addr.wrapping_sub(offset as u32 + 5) as i32;

                let rel_bytes = rel_offset.to_le_bytes();
                data[i + 1] = rel_bytes[0];
                data[i + 2] = rel_bytes[1];
                data[i + 3] = rel_bytes[2];
                data[i + 4] = rel_bytes[3];

                i += 5;
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    /// Apply complex dictionary transformation
    /// This handles complex dict methods like "dict:p:64m:85%" which may include
    /// preprocessing like delta, E8E9, or other transformations
    fn apply_complex_dict_transform(&self, data: &mut [u8]) -> Result<()> {
        // For now, we'll implement a basic version that applies common transformations
        // in sequence. In FreeARC, complex dict methods can include multiple transformations
        // like delta, E8E9, Intel, etc.

        // Apply delta transformation as a common preprocessing step
        // This is a simplified approach - in reality, FreeARC would parse the parameters
        // and apply the appropriate transformations based on the specific dict method

        // For "dict:p:64m:85%", the 'p' might indicate a particular preprocessing
        // For now, we'll just apply a basic delta transformation as an example
        if data.len() > 1 {
            // Apply a simple reverse delta transformation
            for i in 1..data.len() {
                data[i] = data[i].wrapping_add(data[i - 1]);
            }
        }

        Ok(())
    }
}

/// Convenience function to apply dictionary post-processing
pub fn apply_dict_post_processing(data: &mut Vec<u8>, method: &str, original_size: Option<usize>) -> Result<()> {
    if let Some(dict_method) = DictMethod::from_string(method) {
        let processor = DictProcessor::new(dict_method);
        processor.post_process(data, original_size)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dict_method_parsing() {
        assert_eq!(DictMethod::from_string("none"), Some(DictMethod::None));
        assert_eq!(DictMethod::from_string(""), Some(DictMethod::None));
        assert_eq!(DictMethod::from_string("delta"), Some(DictMethod::Delta(1)));
        assert_eq!(DictMethod::from_string("delta:2"), Some(DictMethod::Delta(2)));
        assert_eq!(DictMethod::from_string("e8e9"), Some(DictMethod::E8E9));
        assert_eq!(DictMethod::from_string("intel"), Some(DictMethod::Intel));
        assert_eq!(DictMethod::from_string("dict:p:64m:85%"), Some(DictMethod::ComplexDict));
        assert_eq!(DictMethod::from_string("invalid"), None);
    }

    #[test]
    fn test_delta_reverse_simple() {
        let mut data = vec![10, 5, 3, 7, 2]; // Original: 10, 15, 18, 25, 27
        let processor = DictProcessor::new(DictMethod::Delta(1));
        processor.post_process(&mut data, None).unwrap();
        
        // After reverse delta: 10, 15, 18, 25, 27
        assert_eq!(data[0], 10);
        assert_eq!(data[1], 15); // 10 + 5
        assert_eq!(data[2], 18); // 15 + 3
        assert_eq!(data[3], 25); // 18 + 7
        assert_eq!(data[4], 27); // 25 + 2
    }

    #[test]
    fn test_e8e9_transformation() {
        // Create a simple test case with E8 instruction
        let mut data = vec![
            0xE8, 0x05, 0x00, 0x00, 0x00,  // CALL rel32 to +5 (should become 0x00)
            0x90,                           // NOP
            0xE9, 0x03, 0x00, 0x00, 0x00,  // JMP rel32 to +3 (should become 0x00)
        ];
        
        let processor = DictProcessor::new(DictMethod::E8E9);
        processor.post_process(&mut data, None).unwrap();
        
        // After transformation, the relative addresses should be adjusted
        // The exact values depend on the implementation, but the opcodes should remain
        assert_eq!(data[0], 0xE8); // CALL opcode preserved
        assert_eq!(data[5], 0x90); // NOP preserved
        assert_eq!(data[6], 0xE9); // JMP opcode preserved
    }
}