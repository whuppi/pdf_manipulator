//! Integration tests for outline handling.
//!
//! Does very basic testing of outline parsing.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::outline::Destination::PageIndex;
use pdf_oxide::outline::OutlineItem;

const SIMPLE_PDF_PATH: &str = "tests/fixtures/simple.pdf";
const OUTLINE_PDF_PATH: &str = "tests/fixtures/outline.pdf";

#[test]
fn test_outline_missing() {
    let pdf = PdfDocument::open(SIMPLE_PDF_PATH).expect("Failed to open simple.pdf");
    let outline = pdf.get_outline().expect("Failed to get outline");
    assert!(outline.is_none(), "Outline should should not be found");
}

#[test]
fn test_outline_present() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = PdfDocument::open(OUTLINE_PDF_PATH).expect("Failed to open outline.pdf");
    let outline = pdf
        .get_outline()
        .expect("Failed to get outline")
        .expect("Outline should have been found");

    let [embedded, indirect] = outline
        .try_into()
        .expect("Outline should have had exactly two items");

    match embedded {
        OutlineItem {
            title,
            dest: Some(PageIndex(0)),
            children,
        } => {
            assert_eq!(title, "Outline with Embedded Action");
            assert_eq!(children.len(), 0, "Outline items should have no children");
        },
        _ => return Err("Expected destination to be first page".into()),
    }

    match indirect {
        OutlineItem {
            title,
            dest: Some(PageIndex(0)),
            children,
        } => {
            assert_eq!(title, "Outline with Indirectly Referenced Action");
            assert_eq!(children.len(), 0, "Outline items should have no children");
        },
        _ => return Err("Expected destination to be first page".into()),
    }

    Ok(())
}
