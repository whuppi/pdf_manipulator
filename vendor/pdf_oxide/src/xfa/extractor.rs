//! XFA data extraction from PDF documents.
//!
//! This module provides utilities to detect and extract XFA form data
//! from PDF documents.

// ── pdf_manipulator patch: the whole-slice decoder is test-only now ──
#[cfg(test)]
use crate::decoders::decode_stream;
// ── end pdf_manipulator patch ──
use crate::document::PdfDocument;
use crate::error::{Error, Result};
use crate::object::Object;

/// XFA data extractor.
///
/// Provides static methods to detect and extract XFA form data from PDF documents.
pub struct XfaExtractor;

impl XfaExtractor {
    /// Check if a PDF document contains an XFA form.
    ///
    /// XFA forms are indicated by the presence of an /XFA entry in the /AcroForm
    /// dictionary of the document catalog.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::PdfDocument;
    /// use pdf_oxide::xfa::XfaExtractor;
    ///
    /// let mut doc = PdfDocument::open("form.pdf")?;
    /// if XfaExtractor::has_xfa(&mut doc)? {
    ///     println!("Document contains XFA form");
    /// }
    /// ```
    pub fn has_xfa(doc: &mut PdfDocument) -> Result<bool> {
        let catalog = doc.catalog()?;
        let catalog_dict = match catalog.as_dict() {
            Some(d) => d,
            None => return Ok(false),
        };

        let acroform_obj = match catalog_dict.get("AcroForm") {
            Some(obj) => Self::resolve_object(doc, obj)?,
            None => return Ok(false),
        };

        let acroform_dict = match acroform_obj.as_dict() {
            Some(d) => d,
            None => return Ok(false),
        };

        Ok(acroform_dict.contains_key("XFA"))
    }

    // ── pdf_manipulator patch: XFA packets are handed out as readers over their decoded streams, never as whole buffers ──
    /// The document's `/XFA` entry: a stream holding the complete XDP, or an
    /// array of alternating name and stream packets.
    fn xfa_entry(doc: &mut PdfDocument) -> Result<Object> {
        let catalog = doc.catalog()?;
        let catalog_dict = catalog
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Catalog is not a dictionary".to_string()))?;

        let acroform_obj = catalog_dict
            .get("AcroForm")
            .ok_or_else(|| Error::InvalidPdf("No AcroForm in document".to_string()))?;
        let acroform_obj = Self::resolve_object(doc, acroform_obj)?;

        let acroform_dict = acroform_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("AcroForm is not a dictionary".to_string()))?;

        let xfa_obj = acroform_dict
            .get("XFA")
            .ok_or_else(|| Error::InvalidPdf("No XFA entry in AcroForm".to_string()))?;
        Self::resolve_object(doc, xfa_obj)
    }

    /// Hands each XFA packet to `each` as a reader over its decoded stream:
    /// `("xdp", ..)` for a single stream, the packet's name for an array.
    /// The decoded data is read through a buffer and never held whole.
    pub fn read_packets(
        doc: &mut PdfDocument,
        mut each: impl FnMut(&str, &mut dyn std::io::BufRead) -> Result<()>,
    ) -> Result<()> {
        let mut packet = |name: &str, stream: &Object| -> Result<()> {
            let Object::Stream { dict, body } = stream else {
                return Ok(());
            };
            let decoded =
                crate::decoders::stream::reader_in(dict, body.origin(), || body.raw_reader())?;
            let mut reader = std::io::BufReader::with_capacity(64 * 1024, decoded);
            each(name, &mut reader)
        };
        match Self::xfa_entry(doc)? {
            stream @ Object::Stream { .. } => packet("xdp", &stream),
            Object::Array(arr) => {
                // Array is [name1, stream1, name2, stream2, ...]
                let mut i = 0;
                while i + 1 < arr.len() {
                    let name = match &arr[i] {
                        Object::Name(n) => n.clone(),
                        Object::String(s) => String::from_utf8_lossy(s).to_string(),
                        _ => format!("packet_{}", i / 2),
                    };
                    let stream = Self::resolve_object(doc, &arr[i + 1])?;
                    packet(&name, &stream)?;
                    i += 2;
                }
                Ok(())
            },
            _ => Err(Error::InvalidPdf("XFA entry is neither stream nor array".to_string())),
        }
    }

    /// The form the document's XFA packets describe, parsed as XML events
    /// over each packet's decoded stream.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::PdfDocument;
    /// use pdf_oxide::xfa::XfaExtractor;
    ///
    /// let mut doc = PdfDocument::open("form.pdf")?;
    /// let form = XfaExtractor::parse_form(&mut doc)?;
    /// println!("Found {} fields", form.field_count());
    /// ```
    pub fn parse_form(doc: &mut PdfDocument) -> Result<crate::xfa::XfaForm> {
        let mut parser = crate::xfa::XfaParser::new();
        Self::read_packets(doc, |_name, reader| parser.feed(reader))?;
        Ok(parser.finish())
    }
    // ── end pdf_manipulator patch ──

    /// Resolve an indirect reference to its object.
    fn resolve_object(doc: &mut PdfDocument, obj: &Object) -> Result<Object> {
        if let Some(ref_val) = obj.as_reference() {
            doc.load_object(ref_val)
        } else {
            Ok(obj.clone())
        }
    }

    // ── pdf_manipulator patch: only the tests decode a stream from a slice now ──
    #[cfg(test)]
    // ── end pdf_manipulator patch ──
    /// Decode a stream's data using its filter chain.
    fn decode_stream_data(
        dict: &std::collections::HashMap<String, Object>,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        // Get filters from dictionary
        let filters = Self::get_filters(dict);
        if filters.is_empty() {
            // No filters, return raw data
            Ok(data.to_vec())
        } else {
            decode_stream(data, &filters)
        }
    }

    // ── pdf_manipulator patch: only the tests read the filter names from a dictionary now ──
    #[cfg(test)]
    // ── end pdf_manipulator patch ──
    /// Extract filter names from stream dictionary.
    fn get_filters(dict: &std::collections::HashMap<String, Object>) -> Vec<String> {
        let filter_obj = dict.get("Filter");
        match filter_obj {
            Some(Object::Name(n)) => vec![n.clone()],
            Some(Object::Array(arr)) => arr
                .iter()
                .filter_map(|o| {
                    if let Object::Name(n) = o {
                        Some(n.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ---- Tests for get_filters ----

    #[test]
    fn test_get_filters_single_name() {
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
        let filters = XfaExtractor::get_filters(&dict);
        assert_eq!(filters, vec!["FlateDecode"]);
    }

    #[test]
    fn test_get_filters_array_of_names() {
        let mut dict = HashMap::new();
        dict.insert(
            "Filter".to_string(),
            Object::Array(vec![
                Object::Name("ASCII85Decode".to_string()),
                Object::Name("FlateDecode".to_string()),
            ]),
        );
        let filters = XfaExtractor::get_filters(&dict);
        assert_eq!(filters, vec!["ASCII85Decode", "FlateDecode"]);
    }

    #[test]
    fn test_get_filters_array_with_non_name_elements() {
        let mut dict = HashMap::new();
        dict.insert(
            "Filter".to_string(),
            Object::Array(vec![
                Object::Name("FlateDecode".to_string()),
                Object::Integer(42), // non-name element should be skipped
                Object::Name("LZWDecode".to_string()),
            ]),
        );
        let filters = XfaExtractor::get_filters(&dict);
        assert_eq!(filters, vec!["FlateDecode", "LZWDecode"]);
    }

    #[test]
    fn test_get_filters_no_filter_key() {
        let dict = HashMap::new();
        let filters = XfaExtractor::get_filters(&dict);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_get_filters_invalid_filter_type() {
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Integer(99));
        let filters = XfaExtractor::get_filters(&dict);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_get_filters_null_filter() {
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Null);
        let filters = XfaExtractor::get_filters(&dict);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_get_filters_empty_array() {
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Array(vec![]));
        let filters = XfaExtractor::get_filters(&dict);
        assert!(filters.is_empty());
    }

    // ---- Tests for decode_stream_data ----

    #[test]
    fn test_decode_stream_data_no_filters() {
        let dict = HashMap::new();
        let data = b"raw stream content";
        let result = XfaExtractor::decode_stream_data(&dict, data).unwrap();
        assert_eq!(result, b"raw stream content");
    }

    #[test]
    fn test_decode_stream_data_with_asciihex_filter() {
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Name("ASCIIHexDecode".to_string()));
        let data = b"48656C6C6F"; // "Hello" in hex
        let result = XfaExtractor::decode_stream_data(&dict, data).unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn test_decode_stream_data_empty_data_no_filters() {
        let dict = HashMap::new();
        let data = b"";
        let result = XfaExtractor::decode_stream_data(&dict, data).unwrap();
        assert!(result.is_empty());
    }

    // ---- Tests for resolve_object ----

    #[test]
    fn test_resolve_object_non_reference() {
        // When the object is not a reference, resolve_object should return a clone
        // We can't easily create a PdfDocument in tests, but we can test the non-reference branch
        // by checking the logic: if obj is not a reference, return Ok(obj.clone())
        let obj = Object::Name("TestName".to_string());
        assert!(obj.as_reference().is_none());
        // The non-reference branch just clones, so the result should equal the input
        // We verify the logic indirectly since we can't call resolve_object without a PdfDocument
    }

    // ---- Tests for extract_xfa_packets_from_array name handling ----

    #[test]
    fn test_packet_name_from_name_object() {
        // Verify Name variant produces the expected name string
        let name_obj = Object::Name("template".to_string());
        if let Object::Name(n) = &name_obj {
            assert_eq!(n, "template");
        }
    }

    #[test]
    fn test_packet_name_from_string_object() {
        // Verify String variant produces the expected name via from_utf8_lossy
        let string_obj = Object::String(b"config".to_vec());
        if let Object::String(s) = &string_obj {
            let name = String::from_utf8_lossy(s).to_string();
            assert_eq!(name, "config");
        }
    }

    #[test]
    fn test_packet_name_fallback() {
        // Verify fallback for non-Name, non-String objects
        let obj = Object::Integer(42);
        let i = 4_usize;
        let name = match &obj {
            Object::Name(n) => n.clone(),
            Object::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => format!("packet_{}", i / 2),
        };
        assert_eq!(name, "packet_2");
    }

    #[test]
    fn test_packet_name_from_string_non_utf8() {
        // Verify String with invalid UTF-8 is handled gracefully
        let string_obj = Object::String(vec![0xFF, 0xFE, 0xFD]);
        if let Object::String(s) = &string_obj {
            let name = String::from_utf8_lossy(s).to_string();
            // Should contain replacement characters
            assert!(name.contains('\u{FFFD}'));
        }
    }
}
