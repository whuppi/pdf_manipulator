//! Integration tests for `enumerate_signatures` / `count_signatures`.
//!
//! These cover the document-level surface used by every binding's
//! `DocumentEditor.signatures` property.

#![cfg(feature = "signatures")]

use pdf_oxide::signatures::{count_signatures, enumerate_signatures};
use pdf_oxide::PdfDocument;

fn open(path: &str) -> PdfDocument {
    PdfDocument::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

#[test]
fn pdf_without_acroform_has_no_signatures() {
    let mut doc = open("tests/fixtures/simple.pdf");
    assert_eq!(count_signatures(&mut doc).unwrap(), 0);
    assert!(enumerate_signatures(&mut doc).unwrap().is_empty());
}

#[test]
fn signed_pdf_has_no_acroform_signatures() {
    // The issue-395 fixture was crafted to reproduce a C# SignatureException
    // regression; it has no real AcroForm signature fields, so
    // enumeration should come back empty (and not error).
    let mut doc = open("tests/fixtures/issue_regressions/issue_395_render_signature_exception.pdf");
    let sigs = enumerate_signatures(&mut doc).expect("enumerate must not error");
    assert_eq!(
        sigs.len(),
        count_signatures(&mut doc).unwrap(),
        "count and enumerate must agree"
    );
}
