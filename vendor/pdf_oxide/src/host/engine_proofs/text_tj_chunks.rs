//! A `TJ` array longer than the content source's chunk size, read by text
//! extraction as the one array it is.

use super::support::{pdf_from_objects, stream_object};
use crate::content::{parse_content_stream, Operator};
use crate::document::PdfDocument;
use crate::error::Result;
use crate::extractors::text::OperatorSource;
use crate::extractors::TextExtractor;
use crate::host::content::OperatorStream;
use crate::object::Object;
use std::sync::Arc;

/// A page whose one text object shows a single `TJ` array of `elements`
/// elements: a word of two letters and a kerning offset in turn, with a wide
/// offset (a word break) after every fifth word.
fn page_with_long_tj(elements: usize) -> Vec<u8> {
    let mut content = String::from("BT /F1 10 Tf 10 700 Td [");
    for i in 0..elements / 2 {
        let word = [b'a' + (i % 26) as u8, b'a' + (i / 26 % 26) as u8];
        content.push_str(&format!("({}{})", word[0] as char, word[1] as char));
        content.push_str(if i % 5 == 4 { " -400 " } else { " -20 " });
    }
    content.push_str("] TJ ET\n");
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", content.as_bytes()),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

/// Operators already read, as a source that never chunks.
struct Whole(std::vec::IntoIter<Operator>);

impl Iterator for Whole {
    type Item = Result<Operator>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Ok)
    }
}

impl OperatorStream for Whole {
    fn tj_continued(&self) -> bool {
        false
    }

    fn tj_continues(&self) -> bool {
        false
    }
}

fn extractor_of(doc: &PdfDocument) -> TextExtractor<'_> {
    let page = doc.get_page_for_debug(0).unwrap();
    let resources: Object = page.as_dict().unwrap().get("Resources").unwrap().clone();
    let mut extractor = TextExtractor::new();
    extractor.set_resources(resources.clone());
    extractor.set_document(doc);
    doc.load_fonts_public(&resources, &mut extractor).unwrap();
    extractor
}

/// The array read whole by `parse_content_stream`, which has no chunk size.
fn whole_source<'a>(content: &[u8]) -> Arc<OperatorSource<'a>> {
    let operators = parse_content_stream(content).unwrap();
    Arc::new(move || Ok(Box::new(Whole(operators.clone().into_iter())) as Box<dyn OperatorStream>))
}

/// No Dart battery sees it: the wire returns a page's text, and no Dart API
/// can place the seams of a 10,000-element array, or build the same array
/// without them.
#[test]
fn a_tj_array_longer_than_a_chunk_extracts_as_the_array_read_whole() {
    let doc = PdfDocument::from_bytes(page_with_long_tj(10_000)).unwrap();
    let content = doc.get_page_content_data(0).unwrap();
    assert!(
        parse_content_stream(&content)
            .unwrap()
            .iter()
            .any(|op| matches!(op, Operator::TJ { array } if array.len() == 10_000)),
        "the reference reads one array of 10,000 elements"
    );

    let chunked = extractor_of(&doc).extract_text_spans(&content).unwrap();
    let whole = extractor_of(&doc).extract_text_spans_from(whole_source(&content)).unwrap();
    assert!(!whole.is_empty());
    assert_eq!(format!("{chunked:?}"), format!("{whole:?}"), "spans");

    let chunked = extractor_of(&doc).extract_owned(&content).unwrap();
    let whole = extractor_of(&doc).extract_owned_from(whole_source(&content)).unwrap();
    assert!(chunked.len() >= 10_000, "every letter, and the spaces between words");
    assert_eq!(format!("{chunked:?}"), format!("{whole:?}"), "characters");
}
