use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check the signature of test.arc
    let mut file = File::open("test.arc")?;
    
    // Read first 4 bytes to check signature
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;
    
    println!("First 4 bytes of test.arc: {:?}", header);
    println!("As ASCII: {}", String::from_utf8_lossy(&header));
    
    // Check if it matches FreeARC signature
    let free_arc_sig = [0x41, 0x72, 0x43, 0x01]; // "ArC\x01"
    if header == free_arc_sig {
        println!("✓ This appears to be a FreeARC archive");
    } else {
        println!("✗ This does not appear to be a FreeARC archive (expected ArC\\x01)");
        println!("Expected FreeARC signature: {:?}", free_arc_sig);
    }
    
    // Check file size
    let size = file.seek(SeekFrom::End(0))?;
    println!("File size: {} bytes", size);
    
    // Go back to beginning and read first 20 bytes
    file.seek(SeekFrom::Start(0))?;
    let mut first_20 = vec![0u8; std::cmp::min(20, size as usize)];
    file.read_exact(&mut first_20)?;
    println!("First 20 bytes: {:?}", first_20);
    
    Ok(())
}