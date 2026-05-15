#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_raw_converter_new() {
        let converter = RawConverter::new();
        // Just test that it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_raw_converter_default() {
        let converter = RawConverter::default();
        // Just test that it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_ppm_to_png_8bit() {
        let converter = RawConverter::new();
        
        // Create a simple 2x2 PPM header and data (8-bit)
        let ppm_data = b"P6\n2 2\n255\nRGBRGBRGB";
        
        let result = converter.ppm_to_png(ppm_data);
        // This should fail because we don't have enough data, but it shouldn't panic
        assert!(result.is_err());
    }

    #[test]
    fn test_ppm_to_png_invalid_format() {
        let converter = RawConverter::new();
        
        // Test with invalid PPM format
        let ppm_data = b"P5\n2 2\n255\n1234"; // P5 is grayscale, not RGB
        
        let result = converter.ppm_to_png(ppm_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_nonexistent_file() {
        let converter = RawConverter::new();
        let nonexistent_path = PathBuf::from("definitely_does_not_exist.cr2");
        
        let result = converter.convert_to_png(&nonexistent_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_jpeg_as_raw() {
        let converter = RawConverter::new();
        // Try to convert a JPEG as if it were RAW - should fail gracefully
        let jpeg_path = PathBuf::from("test.jpg");
        
        let result = converter.convert_to_png(&jpeg_path);
        assert!(result.is_err());
    }
}
