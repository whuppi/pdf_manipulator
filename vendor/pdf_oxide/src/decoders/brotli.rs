//! BrotliDecode filter (PDF 2.0, ISO 32000-2:2020).

// ── pdf_manipulator patch: the filter is a reader ──
use crate::error::Result;
// ── end pdf_manipulator patch ──

use super::StreamDecoder;

/// Decoder for BrotliDecode filter (PDF 2.0).
pub struct BrotliDecoder;

impl StreamDecoder for BrotliDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        // ── pdf_manipulator patch: the filter is a reader ──
        super::stream::decode_slice(input, &["BrotliDecode"], None)
        // ── end pdf_manipulator patch ──
    }

    fn name(&self) -> &str {
        "BrotliDecode"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brotli_roundtrip() {
        let original = b"Hello, PDF 2.0 BrotliDecode filter! This is a test.";

        // Compress with brotli
        let mut compressed = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
            std::io::Write::write_all(&mut writer, original).unwrap();
        }

        // Decompress with our decoder
        let decoder = BrotliDecoder;
        let decoded = decoder.decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_brotli_empty_input() {
        // Brotli-compressed empty data
        let mut compressed = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
            std::io::Write::write_all(&mut writer, b"").unwrap();
        }
        let decoder = BrotliDecoder;
        let decoded = decoder.decode(&compressed).unwrap();
        assert!(decoded.is_empty());
    }
}
