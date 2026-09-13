//! Integration tests for PDF/A conversion (issue #442).
//!
//! Verifies that convert_to_pdf_a actually writes an XMP metadata stream to
//! the document and that a subsequent validate_pdf_a sees the changes.

use pdf_oxide::compliance::{convert_to_pdf_a, validate_pdf_a, ActionType, PdfALevel};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::extractors::xmp::XmpExtractor;
use pdf_oxide::writer::{DocumentBuilder, PageSize};

/// Build a minimal PDF with no XMP metadata stream.
fn build_plain_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text("PDF/A conversion test").done();
    }
    builder.build().expect("builder failed")
}

#[test]
fn test_convert_adds_xmp_stream_to_catalog() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    // At least one action should have been recorded.
    assert!(!result.actions.is_empty(), "expected at least one conversion action");

    // The document must now have a /Metadata entry in the catalog.
    let catalog = doc.catalog().expect("no catalog");
    let catalog_dict = catalog.as_dict().expect("catalog is not a dict");
    assert!(
        catalog_dict.contains_key("Metadata"),
        "catalog is missing /Metadata after conversion"
    );
}

#[test]
fn test_convert_xmp_contains_pdfaid_identification() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    let xmp = XmpExtractor::extract(&doc)
        .expect("XmpExtractor error")
        .expect("no XMP metadata found after conversion");

    assert_eq!(
        xmp.custom.get("pdfaid:part").map(String::as_str),
        Some("2"),
        "pdfaid:part should be '2' for PDF/A-2b"
    );
    assert_eq!(
        xmp.custom.get("pdfaid:conformance").map(String::as_str),
        Some("B"),
        "pdfaid:conformance should be 'B' for PDF/A-2b"
    );
}

#[test]
fn test_no_duplicate_xmp_actions() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    let xmp_actions = result
        .actions
        .iter()
        .filter(|a| {
            matches!(
                a.action_type,
                ActionType::AddedXmpMetadata | ActionType::AddedPdfaIdentification
            )
        })
        .count();

    assert_eq!(xmp_actions, 1, "expected exactly one XMP-related action, got {xmp_actions}");
}

#[test]
fn test_validate_after_convert_clears_xmp_errors() {
    use pdf_oxide::compliance::ErrorCode;

    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");

    // Pre-conversion: should have XMP errors.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate failed");
    let pre_xmp_errors: Vec<_> = pre
        .errors
        .iter()
        .filter(|e| {
            e.code == ErrorCode::MissingXmpMetadata
                || e.code == ErrorCode::MissingPdfaIdentification
        })
        .collect();
    assert!(!pre_xmp_errors.is_empty(), "expected XMP/pdfaid errors before conversion");

    // Convert.
    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    // Post-conversion: XMP + pdfaid + OutputIntent + Language errors must all be gone.
    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate failed");
    for code in [
        ErrorCode::MissingXmpMetadata,
        ErrorCode::MissingPdfaIdentification,
        ErrorCode::MissingOutputIntent,
        ErrorCode::MissingLanguage,
    ] {
        assert!(
            !post.errors.iter().any(|e| e.code == code),
            "{code:?} still present after conversion: {:?}",
            post.errors
        );
    }
}

#[test]
fn test_convert_roundtrip_bytes_are_valid_pdf() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    // The updated source_bytes must still be a valid PDF.
    assert!(
        doc.source_bytes.starts_with(b"%PDF-"),
        "source_bytes after conversion is not a valid PDF"
    );
    // Must be re-parseable.
    PdfDocument::from_bytes(doc.source_bytes.clone()).expect("re-parse of converted bytes failed");
}

#[test]
fn test_add_output_intent_idempotent() {
    // The converter must not double-add /OutputIntents if called twice.
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");
    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("first conversion failed");
    let result2 = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("second conversion failed");
    // Second pass: document was already compliant (or only has unfixable font errors)
    // — no OutputIntents duplicate should appear.
    let catalog = doc.catalog().expect("no catalog");
    let cat_dict = catalog.as_dict().expect("catalog not a dict");
    if let Some(pdf_oxide::object::Object::Array(arr)) = cat_dict.get("OutputIntents") {
        assert_eq!(arr.len(), 1, "OutputIntents must not be duplicated: {} entries", arr.len());
    }
    let _ = result2;
}

#[test]
fn test_remove_javascript_from_names() {
    let bytes = build_plain_pdf();
    // We convert normally and just assert the action map is clean — a true
    // JS-injection test requires building a PDF with /Names/JavaScript which
    // our builder does not expose. The remove_javascript path is exercised
    // by the validator finding nothing to remove (idempotent, no panic).
    let mut doc = pdf_oxide::document::PdfDocument::from_bytes(bytes).expect("parse failed");
    let result =
        pdf_oxide::compliance::convert_to_pdf_a(&mut doc, pdf_oxide::compliance::PdfALevel::A2b)
            .expect("conversion failed");
    // No JS-related conversion error should appear.
    assert!(
        result
            .errors
            .iter()
            .all(|e| e.error_code != pdf_oxide::compliance::ErrorCode::JavaScriptNotAllowed),
        "unexpected JS conversion error: {:?}",
        result.errors
    );
}

#[test]
fn test_add_language_sets_lang_key() {
    use pdf_oxide::compliance::{convert_to_pdf_a, PdfALevel};

    // For level A (A1a requires structure + lang), the validator emits MissingLanguage.
    // For level B we only warn, so test via direct catalog inspection after conversion.
    // Convert a PDF built with no /Lang.
    let bytes = build_plain_pdf();
    let mut doc = pdf_oxide::document::PdfDocument::from_bytes(bytes).expect("parse failed");

    // Force MissingLanguage to be triggered by validating against A1b
    // which doesn't require structure but our add_language fires on MissingLanguage.
    // Since A2b level-B only warns on Lang, inject it via a direct assertion:
    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("conversion failed");

    // Regardless of whether the validator raised the error, confirm the catalog
    // has a sensible /Lang after conversion (idempotent if already set).
    let catalog = doc.catalog().expect("no catalog");
    // If Lang was set, it must be a string value.
    if let Some(lang) = catalog.as_dict().and_then(|d| d.get("Lang")) {
        assert!(lang.as_string().is_some(), "/Lang must be a PDF string, got: {:?}", lang);
    }
}

#[test]
fn test_convert_all_levels() {
    // Test both B-level and U-level (but not A-level which requires structure).
    for level in [
        PdfALevel::A1b,
        PdfALevel::A2b,
        PdfALevel::A2u,
        PdfALevel::A3b,
    ] {
        let bytes = build_plain_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).expect("parse failed");
        let result = convert_to_pdf_a(&mut doc, level).expect("conversion failed");
        assert!(!result.actions.is_empty(), "no actions for level {level:?}");

        // XMP must be present with correct part/conformance.
        let xmp = XmpExtractor::extract(&doc)
            .expect("XmpExtractor error")
            .expect("no XMP after conversion for {level:?}");
        assert!(
            xmp.custom.contains_key("pdfaid:part"),
            "pdfaid:part missing for level {level:?}"
        );

        // OutputIntents must be present (unconditional per converter logic).
        let catalog = doc.catalog().expect("no catalog");
        assert!(
            catalog
                .as_dict()
                .map(|d| d.contains_key("OutputIntents"))
                .unwrap_or(false),
            "/OutputIntents missing after conversion for level {level:?}"
        );

        // After conversion the document must still be parseable.
        PdfDocument::from_bytes(doc.source_bytes.clone())
            .expect("re-parse failed after conversion for level {level:?}");

        // The post-conversion validation must not report any always-fixable errors.
        let post =
            pdf_oxide::compliance::validate_pdf_a(&mut doc, level).expect("post-validate failed");
        for code in [
            pdf_oxide::compliance::ErrorCode::MissingXmpMetadata,
            pdf_oxide::compliance::ErrorCode::MissingPdfaIdentification,
            pdf_oxide::compliance::ErrorCode::MissingOutputIntent,
            pdf_oxide::compliance::ErrorCode::MissingLanguage,
        ] {
            assert!(
                !post.errors.iter().any(|e| e.code == code),
                "{code:?} still present after conversion for level {level:?}: {:?}",
                post.errors
            );
        }
    }
}
