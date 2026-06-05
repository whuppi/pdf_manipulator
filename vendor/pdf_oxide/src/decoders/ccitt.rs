//! CCITTFaxDecode implementation.
//!
//! CCITT (Comité Consultatif International Téléphonique et Télégraphique)
//! Group 3 and Group 4 fax compression for monochrome images.
//!
//! This is a pass-through decoder for CCITT Fax data. CCITT images are
//! binary image compression formats typically used for scanned documents.
//! For text extraction purposes, we keep the data in compressed format.
//! Full image decompression will be handled in Phase 5 (image extraction).
//!
//! PDF Spec: ISO 32000-1:2008, Section 7.4.6 - CCITTFaxDecode Filter

use crate::decoders::StreamDecoder;
use crate::error::Result;

/// CCITTFaxDecode filter implementation.
///
/// Pass-through for CCITT Fax data - no actual decoding performed.
/// CCITT images are kept in their compressed format for later extraction.
///
/// CCITT compression is used for black-and-white images, commonly in
/// scanned documents and faxes. The format supports Group 3 and Group 4
/// compression schemes.
pub struct CcittFaxDecoder;

impl StreamDecoder for CcittFaxDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        // CCITT Fax data is kept in compressed format.
        // Text extraction doesn't require image decompression.
        // Phase 5 will handle actual image extraction/decoding if needed.
        log::debug!("CCITTFaxDecode: Pass-through {} bytes", input.len());
        Ok(input.to_vec())
    }

    fn name(&self) -> &str {
        "CCITTFaxDecode"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccitt_decode_passthrough() {
        let decoder = CcittFaxDecoder;
        let ccitt_data = b"\x00\x01\x02\x03"; // Mock CCITT data
        let output = decoder.decode(ccitt_data).unwrap();
        assert_eq!(output, ccitt_data);
    }

    #[test]
    fn test_ccitt_decode_empty() {
        let decoder = CcittFaxDecoder;
        let input = b"";
        let output = decoder.decode(input).unwrap();
        assert_eq!(output, b"");
    }

    #[test]
    fn test_ccitt_decoder_name() {
        let decoder = CcittFaxDecoder;
        assert_eq!(decoder.name(), "CCITTFaxDecode");
    }
}
