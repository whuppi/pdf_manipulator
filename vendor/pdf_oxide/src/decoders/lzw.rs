//! LZWDecode implementation for PDF.
//!
//! Decompresses data using the Lempel-Ziv-Welch (LZW) algorithm as specified
//! in the PDF Reference (Section 7.4.4).
//!
//! PDF's LZW implementation:
//! - Uses MSB-first bit ordering
//! - Starts with 9-bit codes
//! - Increases code size when table fills up
//! - Uses EarlyChange=1 by default (change code size one code earlier than
//!   GIF); a `/EarlyChange 0` entry in DecodeParms selects the GIF switch
//! - Clear code is 256, EOD code is 257
//! - First available code is 258

use crate::decoders::StreamDecoder;
// ── pdf_manipulator patch: the filter is a reader ──
use crate::error::Result;
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: one weezl decoder, early or late switch per /EarlyChange ──
/// LZWDecode filter implementation with PDF's default, `/EarlyChange 1`.
///
/// Decompresses data using the LZW algorithm. A stream that sets
/// `/EarlyChange 0` goes through the filter pipeline, which reads the entry.
pub struct LzwDecoder;

impl StreamDecoder for LzwDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        ConfiguredLzwDecoder { early_change: true }.decode(input)
    }

    fn name(&self) -> &str {
        "LZWDecode"
    }
}

/// LZWDecode with the `/EarlyChange` value the stream's DecodeParms name.
pub(crate) struct ConfiguredLzwDecoder {
    pub(crate) early_change: bool,
}

impl StreamDecoder for ConfiguredLzwDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        decode_lzw(input, self.early_change)
    }

    fn name(&self) -> &str {
        "LZWDecode"
    }
}

/// Decodes with weezl through the filter's reader: MSB bit order, 8-bit
/// minimum code size. The code width grows one code early (TIFF size switch)
/// with `/EarlyChange 1` and at the GIF boundary with `/EarlyChange 0`;
/// reading one as the other decodes the wrong bytes without an error.
fn decode_lzw(input: &[u8], early_change: bool) -> Result<Vec<u8>> {
    let params = crate::decoders::DecodeParams { early_change, ..Default::default() };
    crate::decoders::stream::decode_slice(input, &["LZWDecode"], Some(&params))
}
// ── end pdf_manipulator patch ──

#[cfg(test)]
mod tests {
    use super::*;
    use weezl::{encode::Encoder as LzwEncoder, BitOrder};

    fn early(data: &[u8]) -> Vec<u8> {
        LzwEncoder::with_tiff_size_switch(BitOrder::Msb, 8).encode(data).unwrap()
    }

    #[test]
    fn test_lzw_decode_simple() {
        let original = b"ABCABCABCABC";
        assert_eq!(LzwDecoder.decode(&early(original)).unwrap(), original);
    }

    #[test]
    fn test_lzw_decode_empty() {
        assert_eq!(LzwDecoder.decode(&early(b"")).unwrap(), b"");
    }

    #[test]
    fn test_lzw_decode_repeated_pattern() {
        let original = b"The quick brown fox jumps over the lazy dog. ".repeat(10);
        assert_eq!(LzwDecoder.decode(&early(&original)).unwrap(), original);
    }

    #[test]
    fn test_lzw_decode_invalid_data() {
        let invalid = b"This is not LZW compressed data";
        assert!(LzwDecoder.decode(invalid).is_err());
    }

    #[test]
    fn test_lzw_decoder_name() {
        assert_eq!(LzwDecoder.name(), "LZWDecode");
    }
}
