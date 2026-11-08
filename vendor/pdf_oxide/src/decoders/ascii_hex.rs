//! ASCIIHexDecode implementation.
//!
//! Decodes hexadecimal-encoded data (e.g., "48656C6C6F" -> "Hello").
//! Whitespace is ignored, and odd-length input is padded with implicit '0'.

use crate::decoders::StreamDecoder;
// ── pdf_manipulator patch: the filter is a reader ──
use crate::error::Result;
// ── end pdf_manipulator patch ──

/// ASCIIHexDecode filter implementation.
///
/// Decodes data encoded as pairs of hexadecimal digits.
pub struct AsciiHexDecoder;

impl StreamDecoder for AsciiHexDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        // ── pdf_manipulator patch: the filter is a reader ──
        crate::decoders::stream::decode_slice(input, &["ASCIIHexDecode"], None)
        // ── end pdf_manipulator patch ──
    }

    fn name(&self) -> &str {
        "ASCIIHexDecode"
    }
}

/// Convert a hexadecimal ASCII character to its numeric value.
// ── pdf_manipulator patch: the reader has its own digit table ──
#[cfg(test)]
// ── end pdf_manipulator patch ──
fn hex_digit_to_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_hex_decode_simple() {
        let decoder = AsciiHexDecoder;
        let input = b"48656C6C6F";
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_ascii_hex_decode_with_whitespace() {
        let decoder = AsciiHexDecoder;
        let input = b"48 65 6C 6C 6F"; // "Hello" with spaces
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_ascii_hex_decode_odd_length() {
        let decoder = AsciiHexDecoder;
        let input = b"486"; // Odd length - should pad with 0 -> "48 60"
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"H`");
    }

    #[test]
    fn test_ascii_hex_decode_with_end_marker() {
        let decoder = AsciiHexDecoder;
        let input = b"48656C6C6F>"; // "Hello" with EOD marker
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_ascii_hex_decode_lowercase() {
        let decoder = AsciiHexDecoder;
        let input = b"48656c6c6f"; // lowercase hex
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_ascii_hex_decode_mixed_case() {
        let decoder = AsciiHexDecoder;
        let input = b"48656C6c6F"; // mixed case
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"Hello");
    }

    #[test]
    fn test_ascii_hex_decode_empty() {
        let decoder = AsciiHexDecoder;
        let input = b"";
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"");
    }

    #[test]
    fn test_ascii_hex_decode_invalid_digit() {
        let decoder = AsciiHexDecoder;
        let input = b"4G"; // 'G' is not a valid hex digit
        let result = decoder.decode(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_ascii_hex_decoder_name() {
        let decoder = AsciiHexDecoder;
        assert_eq!(decoder.name(), "ASCIIHexDecode");
    }

    #[test]
    fn test_hex_digit_to_value() {
        assert_eq!(hex_digit_to_value(b'0'), Some(0));
        assert_eq!(hex_digit_to_value(b'9'), Some(9));
        assert_eq!(hex_digit_to_value(b'A'), Some(10));
        assert_eq!(hex_digit_to_value(b'F'), Some(15));
        assert_eq!(hex_digit_to_value(b'a'), Some(10));
        assert_eq!(hex_digit_to_value(b'f'), Some(15));
        assert_eq!(hex_digit_to_value(b'G'), None);
        assert_eq!(hex_digit_to_value(b'z'), None);
    }
}
