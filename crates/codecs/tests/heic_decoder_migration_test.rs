// Quick verification test for HEIC decoder migration
// Run with: cargo test --release test_heic_decoder_migration

#[cfg(test)]
mod heic_decoder_tests {
    use codecs::heic::HeicCodec;
    use std::path::Path;

    #[test]
    fn test_decoder_always_available() {
        // Pure Rust decoder should always be available
        assert!(HeicCodec::is_available());
    }

    #[test]
    fn test_decoder_creation() {
        // Should create without errors
        let codec = HeicCodec::new();
        assert!(
            codec.is_ok(),
            "Failed to create HEIC codec: {:?}",
            codec.err()
        );
    }

    #[test]
    fn test_version_string() {
        // Should return version info
        let version = HeicCodec::get_version();
        assert!(version.is_some());
        let ver_str = version.unwrap();
        assert!(
            ver_str.contains("heic-decoder-rs"),
            "Version string: {}",
            ver_str
        );
        assert!(
            ver_str.contains("pure Rust"),
            "Expected 'pure Rust' in version"
        );
    }

    #[test]
    fn test_is_heic_file_detection() {
        use codecs::heic::is_heic_file;

        // Should detect HEIC files by extension
        assert!(is_heic_file(Path::new("test.heic")));
        assert!(is_heic_file(Path::new("test.HEIC")));
        assert!(is_heic_file(Path::new("photo.heif")));
        assert!(is_heic_file(Path::new("image.HIF")));

        // Should reject non-HEIC files
        assert!(!is_heic_file(Path::new("test.jpg")));
        assert!(!is_heic_file(Path::new("test.png")));
        assert!(!is_heic_file(Path::new("test.bmp")));
    }

    #[test]
    fn test_encoding_not_supported() {
        use codecs::heic::png_to_heic;

        // Encoding should return error (not implemented yet)
        let result = png_to_heic(Path::new("in.png"), Path::new("out.heic"), 90);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("not supported") || err_msg.contains("decoding only"),
            "Expected encoding error, got: {}",
            err_msg
        );
    }

    // If you have a test HEIC file, uncomment and update path:
    //
    // #[test]
    // fn test_decode_real_heic() {
    //     let test_file = Path::new("path/to/test.heic");
    //     if !test_file.exists() {
    //         eprintln!("Skipping: test file not found");
    //         return;
    //     }
    //
    //     let mut codec = HeicCodec::new().expect("Failed to create codec");
    //
    //     // Test RGB decoding
    //     let decoded = codec.decode_file(test_file)
    //         .expect("Failed to decode HEIC to RGB");
    //     assert!(decoded.width > 0);
    //     assert!(decoded.height > 0);
    //     assert!(!decoded.data.is_empty());
    //
    //     // Test YCbCr decoding (for BPG pipeline)
    //     let decoded_ycbcr = codec.decode_file_ycbcr420(test_file)
    //         .expect("Failed to decode HEIC to YCbCr");
    //     assert!(decoded_ycbcr.width > 0);
    //     assert!(decoded_ycbcr.height > 0);
    //     assert!(!decoded_ycbcr.y_plane.is_empty());
    //     assert!(!decoded_ycbcr.cb_plane.is_empty());
    //     assert!(!decoded_ycbcr.cr_plane.is_empty());
    //
    //     println!("✅ Successfully decoded {} ({}x{})",
    //              test_file.display(), decoded.width, decoded.height);
    // }
}
