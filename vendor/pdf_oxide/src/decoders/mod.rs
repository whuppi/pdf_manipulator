//! Stream decoder implementations for PDF filters.
//!
//! This module provides decoders for various PDF compression and encoding filters:
//! - FlateDecode (zlib/deflate) - most common
//! - ASCIIHexDecode - hexadecimal encoding
//! - ASCII85Decode - base85 encoding
//! - LZWDecode - LZW compression
//! - RunLengthDecode - run-length encoding
//! - DCTDecode - JPEG (pass-through)
//! - CCITTFaxDecode - CCITT Fax compression (pass-through)
//! - JBIG2Decode - JBIG2 compression (pass-through)
//!
//! Decoders can be chained together in a filter pipeline.

use crate::error::{Error, Result};

mod ascii85;
mod ascii_hex;
mod brotli;
pub(crate) mod ccitt;
mod dct;
// ── pdf_manipulator patch: the engine proofs compare the reader with the committed recovery ──
pub(crate) mod flate;
// ── end pdf_manipulator patch ──
mod jbig2;
pub(crate) mod jpx;
// ── pdf_manipulator patch: the engine proofs build a decoder with a chosen switch ──
pub(crate) mod lzw;
// ── end pdf_manipulator patch ──
mod predictor;
mod runlength;
// ── pdf_manipulator patch: every filter is a reader ──
pub mod stream;
// ── end pdf_manipulator patch ──

pub use ascii85::Ascii85Decoder;
pub use ascii_hex::AsciiHexDecoder;
pub use brotli::BrotliDecoder;
pub use ccitt::CcittFaxDecoder;
pub use dct::DctDecoder;
pub use flate::FlateDecoder;
pub use jbig2::Jbig2Decoder;
pub use lzw::LzwDecoder;
pub use predictor::{decode_predictor, CcittParams, DecodeParams, PngPredictor};
pub use runlength::RunLengthDecoder;

/// PDF stream filter types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// FlateDecode (deflate/zlib compression)
    FlateDecode,
    /// ASCIIHexDecode (hexadecimal encoding)
    ASCIIHexDecode,
    /// ASCII85Decode (base-85 encoding)
    ASCII85Decode,
    /// LZWDecode (Lempel-Ziv-Welch compression)
    LZWDecode,
    /// RunLengthDecode (run-length encoding)
    RunLengthDecode,
    /// DCTDecode (JPEG compression)
    DCTDecode,
    /// CCITTFaxDecode (CCITT Fax compression)
    CCITTFaxDecode,
    /// JBIG2Decode (JBIG2 compression)
    JBIG2Decode,
    /// JPXDecode (JPEG 2000 compression)
    JPXDecode,
    /// BrotliDecode (Brotli compression, PDF 2.0)
    BrotliDecode,
}

/// Trait for PDF stream decoders.
///
/// Each decoder implements a specific PDF filter algorithm and can decode
/// compressed or encoded stream data.
pub trait StreamDecoder {
    /// Decode the input data.
    ///
    /// # Arguments
    ///
    /// * `input` - The encoded/compressed data
    ///
    /// # Returns
    ///
    /// The decoded data or an error if decoding fails.
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>>;

    /// Get the name of this decoder (e.g., "FlateDecode").
    fn name(&self) -> &str;
}

/// Normalize a PDF filter name, handling spec abbreviations and case variations.
///
/// PDF Spec: ISO 32000-1:2008, Table 6 — Standard filter abbreviations.
fn normalize_filter_name(name: &str) -> Result<&'static str> {
    // Fast path: exact match
    match name {
        "FlateDecode" => return Ok("FlateDecode"),
        "ASCIIHexDecode" => return Ok("ASCIIHexDecode"),
        "ASCII85Decode" => return Ok("ASCII85Decode"),
        "LZWDecode" => return Ok("LZWDecode"),
        "RunLengthDecode" => return Ok("RunLengthDecode"),
        "DCTDecode" => return Ok("DCTDecode"),
        "CCITTFaxDecode" => return Ok("CCITTFaxDecode"),
        "JBIG2Decode" => return Ok("JBIG2Decode"),
        "JPXDecode" => return Ok("JPXDecode"),
        "BrotliDecode" => return Ok("BrotliDecode"),
        _ => {},
    }

    // PDF spec abbreviations (Table 6)
    match name {
        "Fl" => return Ok("FlateDecode"),
        "AHx" => return Ok("ASCIIHexDecode"),
        "A85" => return Ok("ASCII85Decode"),
        "LZW" => return Ok("LZWDecode"),
        "RL" => return Ok("RunLengthDecode"),
        "DCT" => return Ok("DCTDecode"),
        "CCF" => return Ok("CCITTFaxDecode"),
        _ => {},
    }

    // Case-insensitive fallback
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "flatedecode" => Ok("FlateDecode"),
        "asciihexdecode" => Ok("ASCIIHexDecode"),
        "ascii85decode" => Ok("ASCII85Decode"),
        "lzwdecode" => Ok("LZWDecode"),
        "runlengthdecode" => Ok("RunLengthDecode"),
        "dctdecode" => Ok("DCTDecode"),
        "ccittfaxdecode" => Ok("CCITTFaxDecode"),
        "jbig2decode" => Ok("JBIG2Decode"),
        "jpxdecode" => Ok("JPXDecode"),
        "brotlidecode" => Ok("BrotliDecode"),
        _ => Err(Error::UnsupportedFilter(name.to_string())),
    }
}

/// Decode stream data using a filter pipeline.
///
/// PDF streams can have multiple filters applied in sequence. This function
/// applies each filter in order to decode the data.
///
/// # Arguments
///
/// * `data` - The raw stream data
/// * `filters` - List of filter names to apply in order
///
/// # Returns
///
/// The fully decoded data or an error if any filter fails.
///
/// # Examples
///
/// ```rust,no_run
/// use pdf_oxide::decoders::decode_stream;
///
/// let compressed_data = vec![/* compressed bytes */];
/// let filters = vec!["FlateDecode".to_string()];
/// let decoded = decode_stream(&compressed_data, &filters).unwrap();
/// ```
pub fn decode_stream(data: &[u8], filters: &[String]) -> Result<Vec<u8>> {
    decode_stream_with_params(data, filters, None)
}

/// Decode stream data using a filter pipeline with optional decode parameters.
///
/// This function extends `decode_stream` by supporting decode parameters
/// (e.g., PNG predictors) that are applied after the main filters.
///
/// # Arguments
///
/// * `data` - The raw stream data
/// * `filters` - List of filter names to apply in order
/// * `params` - Optional decode parameters (for predictors, etc.)
///
/// # Returns
///
/// The fully decoded data or an error if any filter fails.
pub fn decode_stream_with_params(
    data: &[u8],
    filters: &[String],
    params: Option<&DecodeParams>,
) -> Result<Vec<u8>> {
    // ── pdf_manipulator patch: every filter is a reader ──
    stream::decode_slice(data, filters, params)
    // ── end pdf_manipulator patch ──
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_stream_no_filters() {
        let data = b"Hello, World!";
        let result = decode_stream(data, &[]).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_decode_stream_unsupported_filter() {
        let data = b"test";
        let filters = vec!["UnsupportedFilter".to_string()];
        let result = decode_stream(data, &filters);
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::UnsupportedFilter(name)) => {
                assert_eq!(name, "UnsupportedFilter");
            },
            _ => panic!("Expected UnsupportedFilter error"),
        }
    }

    #[test]
    fn test_decode_stream_pipeline() {
        // Test with ASCIIHexDecode
        let data = b"48656C6C6F"; // "Hello" in hex
        let filters = vec!["ASCIIHexDecode".to_string()];
        let result = decode_stream(data, &filters).unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn test_normalize_filter_abbreviations() {
        assert_eq!(normalize_filter_name("A85").unwrap(), "ASCII85Decode");
        assert_eq!(normalize_filter_name("AHx").unwrap(), "ASCIIHexDecode");
        assert_eq!(normalize_filter_name("LZW").unwrap(), "LZWDecode");
        assert_eq!(normalize_filter_name("Fl").unwrap(), "FlateDecode");
        assert_eq!(normalize_filter_name("RL").unwrap(), "RunLengthDecode");
        assert_eq!(normalize_filter_name("CCF").unwrap(), "CCITTFaxDecode");
        assert_eq!(normalize_filter_name("DCT").unwrap(), "DCTDecode");
    }

    #[test]
    fn test_normalize_filter_case_insensitive() {
        assert_eq!(normalize_filter_name("Flatedecode").unwrap(), "FlateDecode");
        assert_eq!(normalize_filter_name("FLATEDECODE").unwrap(), "FlateDecode");
        assert_eq!(normalize_filter_name("flatedecode").unwrap(), "FlateDecode");
        assert_eq!(normalize_filter_name("ascii85decode").unwrap(), "ASCII85Decode");
        assert_eq!(normalize_filter_name("ASCIIHEXDECODE").unwrap(), "ASCIIHexDecode");
    }

    #[test]
    fn test_normalize_filter_unknown() {
        let result = normalize_filter_name("BogusFilter");
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::UnsupportedFilter(name)) => {
                assert_eq!(name, "BogusFilter");
            },
            _ => panic!("Expected UnsupportedFilter error"),
        }
    }

    #[test]
    fn test_decode_stream_with_abbreviation() {
        let data = b"48656C6C6F"; // "Hello" in hex
        let filters = vec!["AHx".to_string()];
        let result = decode_stream(data, &filters).unwrap();
        assert_eq!(result, b"Hello");
    }
}
