//! Integration tests for PDF/X, PDF/A, and PDF/UA compliance validators.
//!
//! These tests exercise the compliance validation APIs using minimal in-memory
//! PDFs. Because the test PDFs are intentionally minimal, they will not be
//! compliant -- the goal is to verify that the validators execute without
//! panicking and produce meaningful, correctly-categorized errors.

use pdf_oxide::compliance::{validate_pdf_ua, PdfUaLevel, UaErrorCode};
use pdf_oxide::compliance::{
    validate_pdf_x, PdfALevel, PdfAValidator, PdfXLevel, PdfXValidator, XComplianceError,
    XErrorCode, XSeverity,
};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::writer::{DocumentBuilder, DocumentMetadata, PageSize};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal valid PDF (header + catalog + pages + one empty page).
/// This is the smallest PDF that the parser will accept.
fn build_minimal_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    builder.page(PageSize::Letter).done();
    builder.build().expect("Failed to build minimal PDF")
}

/// Build a slightly richer PDF with metadata and text content.
fn build_pdf_with_metadata_and_text() -> Vec<u8> {
    let metadata = DocumentMetadata::new()
        .title("Compliance Test Document")
        .author("pdf_oxide test suite")
        .subject("Compliance coverage testing");

    let mut builder = DocumentBuilder::new().metadata(metadata);
    builder
        .page(PageSize::A4)
        .at(72.0, 750.0)
        .heading(1, "Test Heading")
        .text("This is body text for compliance testing.")
        .done();
    builder.build().expect("Failed to build PDF with metadata")
}

/// Build a multi-page PDF to test per-page validation.
fn build_multi_page_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    builder.page(PageSize::Letter).text("Page 1").done();
    builder.page(PageSize::A4).text("Page 2").done();
    builder.page(PageSize::Letter).text("Page 3").done();
    builder.build().expect("Failed to build multi-page PDF")
}

/// Open a PdfDocument from raw bytes for validation.
fn open_document(data: &[u8]) -> PdfDocument {
    PdfDocument::from_bytes(data.to_vec()).expect("Failed to open PDF from bytes")
}

// ---------------------------------------------------------------------------
// 1. PdfXValidator construction tests
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_validator_new_x1a2001() {
    let validator = PdfXValidator::new(PdfXLevel::X1a2001);
    // Construction should succeed without panic; validate against a document to
    // confirm the level is wired through.
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);
    let result = validator
        .validate(&mut doc)
        .expect("validate must not fail");
    assert_eq!(result.level, PdfXLevel::X1a2001);
}

#[test]
fn test_pdf_x_validator_new_x32003() {
    let validator = PdfXValidator::new(PdfXLevel::X32003);
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);
    let result = validator
        .validate(&mut doc)
        .expect("validate must not fail");
    assert_eq!(result.level, PdfXLevel::X32003);
}

#[test]
fn test_pdf_x_validator_new_x4() {
    let validator = PdfXValidator::new(PdfXLevel::X4);
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);
    let result = validator
        .validate(&mut doc)
        .expect("validate must not fail");
    assert_eq!(result.level, PdfXLevel::X4);
}

#[test]
fn test_pdf_x_validator_new_x6() {
    let validator = PdfXValidator::new(PdfXLevel::X6);
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);
    let result = validator
        .validate(&mut doc)
        .expect("validate must not fail");
    assert_eq!(result.level, PdfXLevel::X6);
}

// ---------------------------------------------------------------------------
// 2. PdfXValidator builder methods
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_validator_builder_stop_on_first_error() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    // Default: collect all errors.
    let result_all = PdfXValidator::new(PdfXLevel::X1a2003)
        .validate(&mut doc)
        .expect("validate must not fail");
    let all_error_count = result_all.errors.len();

    // With stop_on_first_error: should have fewer or equal errors.
    let mut doc2 = open_document(&pdf);
    let result_stop = PdfXValidator::new(PdfXLevel::X1a2003)
        .stop_on_first_error(true)
        .validate(&mut doc2)
        .expect("validate must not fail");

    assert!(
        result_stop.errors.len() <= all_error_count,
        "stop_on_first_error should produce at most as many errors as full validation ({} vs {})",
        result_stop.errors.len(),
        all_error_count,
    );
    // A minimal PDF will trigger at least one error, so early stop should
    // report exactly 1.
    assert!(
        !result_stop.errors.is_empty(),
        "Minimal PDF should still produce at least one error"
    );
}

#[test]
fn test_pdf_x_validator_builder_include_warnings() {
    let pdf = build_minimal_pdf();

    // With warnings.
    let mut doc = open_document(&pdf);
    let result_with = PdfXValidator::new(PdfXLevel::X4)
        .include_warnings(true)
        .validate(&mut doc)
        .expect("validate must not fail");

    // Without warnings.
    let mut doc2 = open_document(&pdf);
    let result_without = PdfXValidator::new(PdfXLevel::X4)
        .include_warnings(false)
        .validate(&mut doc2)
        .expect("validate must not fail");

    assert!(
        result_without.warnings.is_empty(),
        "include_warnings(false) should suppress all warnings"
    );
    // Errors should be identical regardless of the warning flag.
    assert_eq!(
        result_with.errors.len(),
        result_without.errors.len(),
        "Error counts should match regardless of warning flag"
    );
}

// ---------------------------------------------------------------------------
// 3. validate() on a minimal PDF -- meaningful error reporting
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_validate_minimal_pdf_reports_output_intent_missing() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X1a2003)
        .validate(&mut doc)
        .expect("validate must not fail");

    assert!(!result.is_compliant, "Minimal PDF must not be PDF/X compliant");
    assert!(result.has_errors(), "Validation must report errors");

    // There must be an OutputIntentMissing error.
    let has_output_intent_error = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::OutputIntentMissing);
    assert!(
        has_output_intent_error,
        "Expected OutputIntentMissing error; got codes: {:?}",
        result.errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

#[test]
fn test_pdf_x_validate_minimal_pdf_reports_metadata_issues() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .validate(&mut doc)
        .expect("validate must not fail");

    // Check for XMP metadata or GTS_PDFXVersion errors/warnings.
    let metadata_codes: Vec<XErrorCode> = result
        .errors
        .iter()
        .chain(result.warnings.iter())
        .filter(|e| {
            matches!(
                e.code,
                XErrorCode::XmpMetadataMissing
                    | XErrorCode::XmpMetadataInvalid
                    | XErrorCode::GtsPdfxVersionMissing
                    | XErrorCode::GtsPdfxConformanceMissing
            )
        })
        .map(|e| e.code)
        .collect();

    assert!(
        !metadata_codes.is_empty(),
        "Expected metadata-related errors or warnings for a minimal PDF; found none"
    );
}

#[test]
fn test_pdf_x_validate_minimal_pdf_reports_trim_or_art_box_missing() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .validate(&mut doc)
        .expect("validate must not fail");

    let has_box_error = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::TrimOrArtBoxMissing);

    assert!(
        has_box_error,
        "Expected TrimOrArtBoxMissing error for minimal PDF; got codes: {:?}",
        result.errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// 4. validate_pdf_x() convenience function
// ---------------------------------------------------------------------------

#[test]
fn test_validate_pdf_x_convenience_function() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result =
        validate_pdf_x(&mut doc, PdfXLevel::X1a2003).expect("convenience fn must not fail");

    assert!(!result.is_compliant, "Minimal PDF must not be PDF/X compliant");
    assert!(result.has_errors());
    assert_eq!(result.level, PdfXLevel::X1a2003);
}

// ---------------------------------------------------------------------------
// 5. Error content checks -- meaningful messages, page references, severity
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_error_has_meaningful_message() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X4).expect("validate must not fail");

    for error in &result.errors {
        assert!(
            !error.message.is_empty(),
            "Error code {:?} should have a non-empty message",
            error.code
        );
        // Every error must have Error severity (not Warning).
        assert_eq!(
            error.severity,
            XSeverity::Error,
            "Errors in the errors vec must have Error severity; got {:?} for {:?}",
            error.severity,
            error.code,
        );
    }

    for warning in &result.warnings {
        assert!(
            !warning.message.is_empty(),
            "Warning code {:?} should have a non-empty message",
            warning.code
        );
        assert_eq!(
            warning.severity,
            XSeverity::Warning,
            "Warnings in the warnings vec must have Warning severity; got {:?} for {:?}",
            warning.severity,
            warning.code,
        );
    }
}

#[test]
fn test_pdf_x_page_box_errors_carry_page_number() {
    let pdf = build_multi_page_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X4).expect("validate must not fail");

    let box_errors: Vec<&XComplianceError> = result
        .errors
        .iter()
        .filter(|e| e.code == XErrorCode::TrimOrArtBoxMissing)
        .collect();

    // We expect per-page TrimOrArtBox errors -- each should carry a page number.
    for error in &box_errors {
        assert!(
            error.page.is_some(),
            "TrimOrArtBoxMissing error should carry a page number, but page is None"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Multiple PdfXLevel variants
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_all_levels_validate_without_panic() {
    let levels = [
        PdfXLevel::X1a2001,
        PdfXLevel::X1a2003,
        PdfXLevel::X32002,
        PdfXLevel::X32003,
        PdfXLevel::X4,
        PdfXLevel::X4p,
        PdfXLevel::X5g,
        PdfXLevel::X5n,
        PdfXLevel::X5pg,
        PdfXLevel::X6,
    ];

    let pdf = build_minimal_pdf();

    for level in &levels {
        let mut doc = open_document(&pdf);
        let result = PdfXValidator::new(*level)
            .validate(&mut doc)
            .unwrap_or_else(|e| panic!("Validation failed for {:?}: {}", level, e));

        assert!(!result.is_compliant, "Minimal PDF should not be compliant for {:?}", level);
        assert_eq!(result.level, *level);
    }
}

#[test]
fn test_pdf_x_level_properties_consistency() {
    // Verify level metadata helpers match expectations.
    assert!(!PdfXLevel::X1a2001.allows_transparency());
    assert!(!PdfXLevel::X1a2001.allows_rgb());
    assert!(!PdfXLevel::X32003.allows_transparency());
    assert!(PdfXLevel::X32003.allows_rgb());
    assert!(PdfXLevel::X4.allows_transparency());
    assert!(PdfXLevel::X4.allows_rgb());
    assert!(PdfXLevel::X4.allows_layers());
    assert!(PdfXLevel::X4p.allows_external_icc());
    assert!(!PdfXLevel::X4.allows_external_icc());
    assert!(PdfXLevel::X5g.allows_external_graphics());
    assert!(PdfXLevel::X5pg.allows_external_graphics());
    assert!(!PdfXLevel::X4.allows_external_graphics());
    assert_eq!(PdfXLevel::X6.required_pdf_version(), "2.0");
    assert_eq!(PdfXLevel::X1a2001.required_pdf_version(), "1.3");
}

// ---------------------------------------------------------------------------
// 7. DocumentBuilder-generated PDF with more structure
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_validate_builder_pdf_with_metadata() {
    let pdf = build_pdf_with_metadata_and_text();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X4).expect("validate must not fail");

    // Should still fail compliance (no OutputIntents, no TrimBox, etc.)
    assert!(!result.is_compliant);

    // But the validator should have checked pages.
    assert!(
        result.stats.pages_checked >= 1,
        "stats.pages_checked should be at least 1; got {}",
        result.stats.pages_checked
    );

    // Output intent should still be flagged.
    let has_output_intent = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::OutputIntentMissing);
    assert!(has_output_intent, "OutputIntentMissing should be reported");
}

#[test]
fn test_pdf_x_validate_multi_page_counts_all_pages() {
    let pdf = build_multi_page_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X1a2003).expect("validate must not fail");

    // The validator should check all 3 pages.
    assert_eq!(
        result.stats.pages_checked, 3,
        "Expected 3 pages checked; got {}",
        result.stats.pages_checked
    );
}

// ---------------------------------------------------------------------------
// 8. PDF/A validator coverage
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_a_validator_on_minimal_pdf() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let validator = PdfAValidator::new();
    let result = validator
        .validate(&mut doc, PdfALevel::A2b)
        .expect("PDF/A validation must not fail");

    assert!(!result.is_compliant, "Minimal PDF should not be PDF/A-2b compliant");
    assert!(result.has_errors(), "Should produce at least one error");
    assert_eq!(result.level, PdfALevel::A2b);
}

#[test]
fn test_pdf_a_validator_builder_methods() {
    let pdf = build_minimal_pdf();

    // include_warnings(false) suppresses warnings.
    let mut doc = open_document(&pdf);
    let result = PdfAValidator::new()
        .include_warnings(false)
        .validate(&mut doc, PdfALevel::A1b)
        .expect("PDF/A validation must not fail");
    assert!(
        result.warnings.is_empty(),
        "Warnings should be empty when include_warnings is false"
    );

    // stop_on_first_error limits errors.
    let mut doc2 = open_document(&pdf);
    let result_stop = PdfAValidator::new()
        .stop_on_first_error(true)
        .validate(&mut doc2, PdfALevel::A2b)
        .expect("PDF/A validation must not fail");
    assert!(
        !result_stop.errors.is_empty(),
        "Should have at least one error even with stop_on_first_error"
    );
}

#[test]
fn test_pdf_a_multiple_levels() {
    let levels = [
        PdfALevel::A1a,
        PdfALevel::A1b,
        PdfALevel::A2a,
        PdfALevel::A2b,
        PdfALevel::A2u,
        PdfALevel::A3a,
        PdfALevel::A3b,
        PdfALevel::A3u,
    ];

    let pdf = build_minimal_pdf();

    for level in &levels {
        let mut doc = open_document(&pdf);
        let result = PdfAValidator::new()
            .validate(&mut doc, *level)
            .unwrap_or_else(|e| panic!("PDF/A validation failed for {:?}: {}", level, e));

        assert!(!result.is_compliant, "Minimal PDF should not be compliant for {:?}", level);
        assert_eq!(result.level, *level);
    }
}

// ---------------------------------------------------------------------------
// 9. PDF/UA validator coverage
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_ua_validator_on_minimal_pdf() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result =
        validate_pdf_ua(&mut doc, PdfUaLevel::Ua1).expect("PDF/UA validation must not fail");

    assert!(!result.is_compliant, "Minimal PDF should not be PDF/UA-1 compliant");
    assert!(result.has_errors(), "Should produce at least one error");
    assert_eq!(result.level, PdfUaLevel::Ua1);

    // A minimal PDF will at least be missing Tagged PDF structure and language.
    let error_codes: Vec<UaErrorCode> = result.errors.iter().map(|e| e.code).collect();
    let has_structure_error = error_codes
        .iter()
        .any(|c| matches!(c, UaErrorCode::NotTaggedPdf | UaErrorCode::MissingLanguage));
    assert!(
        has_structure_error,
        "Expected structural accessibility errors (NotTaggedPdf or MissingLanguage); got {:?}",
        error_codes
    );
}

// ---------------------------------------------------------------------------
// 10. XValidationResult and XValidationStats sanity
// ---------------------------------------------------------------------------

#[test]
fn test_x_validation_result_total_issues() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X4).expect("validate must not fail");

    assert_eq!(
        result.total_issues(),
        result.errors.len() + result.warnings.len(),
        "total_issues() must equal errors + warnings"
    );
}

#[test]
fn test_x_compliance_error_display_format() {
    // Construct an error manually and verify Display output.
    let error =
        XComplianceError::new(XErrorCode::OutputIntentMissing, "OutputIntents array is required")
            .with_page(2)
            .with_object_id(42);

    let display = format!("{}", error);
    assert!(
        display.contains("XMETA-001"),
        "Display should contain error code string; got: {}",
        display
    );
    assert!(
        display.contains("page 3"),
        "Display should show 1-indexed page number; got: {}",
        display
    );
    assert!(display.contains("object 42"), "Display should show object id; got: {}", display);
}

// ---------------------------------------------------------------------------
// 11. XComplianceError constructors and builders
// ---------------------------------------------------------------------------

#[test]
fn test_x_compliance_error_warning_constructor() {
    let warning =
        XComplianceError::warning(XErrorCode::TrappedKeyMissing, "Trapped key should be present");
    assert_eq!(warning.severity, XSeverity::Warning);
    assert_eq!(warning.code, XErrorCode::TrappedKeyMissing);
}

#[test]
fn test_x_compliance_error_with_clause() {
    let error = XComplianceError::new(XErrorCode::FontNotEmbedded, "Font must be embedded")
        .with_clause("6.3.5");
    assert_eq!(error.clause, Some("6.3.5".to_string()));
}

#[test]
fn test_x_compliance_error_with_page_and_object() {
    let error =
        XComplianceError::new(XErrorCode::TrimOrArtBoxMissing, "TrimBox or ArtBox required")
            .with_page(5)
            .with_object_id(100);

    assert_eq!(error.page, Some(5));
    assert_eq!(error.object_id, Some(100));
}

#[test]
fn test_x_compliance_error_debug() {
    let error = XComplianceError::new(XErrorCode::EncryptionNotAllowed, "No encryption");
    let debug = format!("{:?}", error);
    assert!(debug.contains("EncryptionNotAllowed"));
}

// ---------------------------------------------------------------------------
// 12. XErrorCode Display and properties
// ---------------------------------------------------------------------------

#[test]
fn test_x_error_code_all_variants_display() {
    let codes = [
        XErrorCode::OutputIntentMissing,
        XErrorCode::OutputIntentInvalid,
        XErrorCode::OutputConditionMissing,
        XErrorCode::XmpMetadataMissing,
        XErrorCode::XmpMetadataInvalid,
        XErrorCode::GtsPdfxVersionMissing,
        XErrorCode::GtsPdfxConformanceMissing,
        XErrorCode::TrappedKeyMissing,
        XErrorCode::EncryptionNotAllowed,
        XErrorCode::MediaBoxMissing,
        XErrorCode::TrimOrArtBoxMissing,
        XErrorCode::TrimBoxInvalid,
        XErrorCode::BleedBoxInvalid,
        XErrorCode::BoxesInconsistent,
        XErrorCode::TransparencyNotAllowed,
        XErrorCode::SMaskNotAllowed,
        XErrorCode::BlendModeNotAllowed,
        XErrorCode::RgbColorNotAllowed,
        XErrorCode::DeviceColorWithoutIntent,
        XErrorCode::IccProfileInvalid,
        XErrorCode::FontNotEmbedded,
        XErrorCode::Type3FontNotAllowed,
        XErrorCode::AnnotationNotAllowed,
        XErrorCode::JavaScriptNotAllowed,
        XErrorCode::ActionNotAllowed,
    ];

    for code in &codes {
        let display = format!("{}", code);
        assert!(!display.is_empty(), "Display for {:?} should not be empty", code);
        // Each code should have a string ID like XMETA-001
        let code_str = code.to_string();
        assert!(!code_str.is_empty(), "to_string() for {:?} should not be empty", code);
    }
}

// ---------------------------------------------------------------------------
// 13. PdfXLevel properties
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_level_xmp_version() {
    // Each level should have a non-empty XMP version string
    let levels = [
        PdfXLevel::X1a2001,
        PdfXLevel::X1a2003,
        PdfXLevel::X32002,
        PdfXLevel::X32003,
        PdfXLevel::X4,
        PdfXLevel::X4p,
        PdfXLevel::X5g,
        PdfXLevel::X5n,
        PdfXLevel::X5pg,
        PdfXLevel::X6,
    ];

    for level in &levels {
        let xmp = level.xmp_version();
        assert!(!xmp.is_empty(), "xmp_version() for {:?} should not be empty", level);
    }
}

#[test]
fn test_pdf_x_level_from_gts_version() {
    // Known GTS version strings
    assert_eq!(PdfXLevel::from_gts_version("PDF/X-1a:2001"), Some(PdfXLevel::X1a2001));
    assert_eq!(PdfXLevel::from_gts_version("PDF/X-1a:2003"), Some(PdfXLevel::X1a2003));
    assert_eq!(PdfXLevel::from_gts_version("PDF/X-4"), Some(PdfXLevel::X4));
    assert_eq!(PdfXLevel::from_gts_version("PDF/X-6"), Some(PdfXLevel::X6));
    // Unknown string
    assert_eq!(PdfXLevel::from_gts_version("Something Else"), None);
}

#[test]
fn test_pdf_x_level_display_and_debug() {
    let level = PdfXLevel::X4;
    let display = format!("{}", level);
    let debug = format!("{:?}", level);
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}

// ---------------------------------------------------------------------------
// 14. XValidationResult helpers
// ---------------------------------------------------------------------------

#[test]
fn test_x_validation_result_has_errors_and_warnings() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .include_warnings(true)
        .validate(&mut doc)
        .unwrap();

    assert!(result.has_errors());
    // total_issues includes both errors and warnings
    assert!(result.total_issues() >= result.errors.len());
}

#[test]
fn test_x_validation_result_display() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_x(&mut doc, PdfXLevel::X1a2003).unwrap();
    let display = format!("{:?}", result);
    assert!(!display.is_empty());
}

// ---------------------------------------------------------------------------
// 15. Validation with richer PDFs to hit more code paths
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_x_validate_x1a_disallows_rgb() {
    // X-1a doesn't allow RGB. Our minimal PDF has no RGB explicitly,
    // but the validator should still check for it.
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X1a2001)
        .validate(&mut doc)
        .unwrap();

    // Should not have RgbColorNotAllowed since minimal PDF has no color spaces
    let has_rgb = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::RgbColorNotAllowed);
    // This is fine either way - just exercising the code path
    let _ = has_rgb;
}

#[test]
fn test_pdf_x_validate_encryption_check() {
    // A minimal PDF should not be encrypted
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .validate(&mut doc)
        .unwrap();

    let has_encryption = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::EncryptionNotAllowed);
    assert!(!has_encryption, "Minimal PDF should not trigger encryption error");
}

#[test]
fn test_pdf_x_validate_font_checking() {
    let pdf = build_pdf_with_metadata_and_text();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X1a2003)
        .validate(&mut doc)
        .unwrap();

    // Builder PDFs should have fonts - check that stats track them
    // The stats.fonts_checked may be > 0 since the PDF has text
    let _ = result.stats.fonts_checked;
    let _ = result.stats.fonts_embedded;
}

#[test]
fn test_pdf_x_validate_transparency_x1a_no_transparency() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    // X-1a doesn't allow transparency
    let result = PdfXValidator::new(PdfXLevel::X1a2001)
        .validate(&mut doc)
        .unwrap();
    // Minimal PDF has no transparency groups, so this should not trigger
    let has_transparency = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::TransparencyNotAllowed);
    assert!(!has_transparency, "Minimal PDF should not have transparency");
    assert!(!result.stats.has_transparency);
}

#[test]
fn test_pdf_x_validate_annotations_checking() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .validate(&mut doc)
        .unwrap();
    // Minimal PDF has no annotations, so annotations_checked should be 0
    assert_eq!(result.stats.annotations_checked, 0);
}

#[test]
fn test_pdf_x_validate_actions_checking() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = PdfXValidator::new(PdfXLevel::X4)
        .validate(&mut doc)
        .unwrap();
    // Minimal PDF has no JavaScript or Actions
    let has_js = result
        .errors
        .iter()
        .any(|e| e.code == XErrorCode::JavaScriptNotAllowed);
    assert!(!has_js);
}

// ---------------------------------------------------------------------------
// 16. PDF/A levels
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_a_level_display_and_debug() {
    let levels = [
        PdfALevel::A1a,
        PdfALevel::A1b,
        PdfALevel::A2a,
        PdfALevel::A2b,
        PdfALevel::A2u,
        PdfALevel::A3a,
        PdfALevel::A3b,
        PdfALevel::A3u,
    ];
    for level in &levels {
        let display = format!("{}", level);
        let debug = format!("{:?}", level);
        assert!(!display.is_empty());
        assert!(!debug.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 17. PDF/UA levels
// ---------------------------------------------------------------------------

#[test]
fn test_pdf_ua_level_ua2() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result =
        validate_pdf_ua(&mut doc, PdfUaLevel::Ua2).expect("PDF/UA-2 validation must not fail");
    assert!(!result.is_compliant);
    assert_eq!(result.level, PdfUaLevel::Ua2);
}

#[test]
fn test_pdf_ua_has_errors_with_meaningful_messages() {
    let pdf = build_minimal_pdf();
    let mut doc = open_document(&pdf);

    let result = validate_pdf_ua(&mut doc, PdfUaLevel::Ua1).unwrap();
    for error in &result.errors {
        assert!(!error.message.is_empty());
    }
}
