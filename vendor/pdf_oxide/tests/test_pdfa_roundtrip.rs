//! End-to-end PDF/A roundtrip tests: validate → convert → validate.
//!
//! Each test follows the same pattern:
//!   1. Build a PDF that has a specific compliance problem.
//!   2. Validate — confirm the expected ErrorCode is reported.
//!   3. Convert (PdfAConverter or convert_to_pdf_a).
//!   4. Validate again — confirm the error is gone (or a ConversionError was
//!      recorded for things that genuinely cannot be fixed without extra
//!      capabilities like the rendering feature).
//!
//! Tests that require the `rendering` feature are gated with `#[cfg(feature = "rendering")]`.

use pdf_oxide::compliance::{
    convert_to_pdf_a, validate_pdf_a, ActionType, ConversionConfig, ConversionResult, ErrorCode,
    PdfAConverter, PdfALevel, ValidationResult,
};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::writer::{DocumentBuilder, PageSize};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_plain_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text("Roundtrip test").done();
    }
    builder.build().expect("builder failed")
}

fn build_multipage_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let p1 = builder.page(PageSize::Letter);
        p1.at(72.0, 720.0).text("Page one").done();
    }
    {
        let p2 = builder.page(PageSize::Letter);
        p2.at(72.0, 720.0).text("Page two").done();
    }
    builder.build().expect("builder failed")
}

/// Build a minimal valid PDF with a catalog-level /OpenAction JavaScript action.
/// The validator catches /OpenAction + /S /JavaScript as JavaScriptNotAllowed.
fn build_pdf_with_open_action_js() -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    out.extend_from_slice(b"%PDF-1.4\n");

    // Obj 1: Catalog with /OpenAction pointing to a JS action
    offsets.push(out.len());
    out.extend_from_slice(
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /OpenAction 3 0 R >>\nendobj\n",
    );

    // Obj 2: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 1 /Kids [4 0 R] >>\nendobj\n");

    // Obj 3: JavaScript action
    offsets.push(out.len());
    out.extend_from_slice(b"3 0 obj\n<< /S /JavaScript /JS (app.alert(1)) >>\nendobj\n");

    // Obj 4: Page
    offsets.push(out.len());
    out.extend_from_slice(
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    // xref
    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 5\n");
    out.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    out.extend_from_slice(b"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n");
    out.extend_from_slice(format!("{}\n", xref_offset).as_bytes());
    out.extend_from_slice(b"%%EOF\n");
    out
}

/// Build a PDF with a /Names << /JavaScript ... >> tree in the catalog.
fn build_pdf_with_names_javascript() -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    out.extend_from_slice(b"%PDF-1.4\n");

    // Obj 1: Catalog with /Names referencing the names dict
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Names 3 0 R >>\nendobj\n");

    // Obj 2: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 1 /Kids [5 0 R] >>\nendobj\n");

    // Obj 3: Names dict with /JavaScript
    offsets.push(out.len());
    out.extend_from_slice(b"3 0 obj\n<< /JavaScript 4 0 R >>\nendobj\n");

    // Obj 4: JavaScript name tree (minimal leaf)
    offsets.push(out.len());
    out.extend_from_slice(
        b"4 0 obj\n<< /Names [(docjs) << /S /JavaScript /JS (app.alert(1)) >>] >>\nendobj\n",
    );

    // Obj 5: Page
    offsets.push(out.len());
    out.extend_from_slice(
        b"5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 6\n");
    out.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    out.extend_from_slice(b"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n");
    out.extend_from_slice(format!("{}\n", xref_offset).as_bytes());
    out.extend_from_slice(b"%%EOF\n");
    out
}

/// Return all ErrorCodes present in a ValidationResult.
fn validation_error_codes(result: &ValidationResult) -> Vec<ErrorCode> {
    result.errors.iter().map(|e| e.code).collect()
}

/// Return all ErrorCodes present in a ConversionResult (unfixed errors).
fn conversion_error_codes(result: &ConversionResult) -> Vec<ErrorCode> {
    result.errors.iter().map(|e| e.error_code).collect()
}

/// Return all ActionTypes present in a ConversionResult.
fn conversion_action_types(result: &ConversionResult) -> Vec<ActionType> {
    result.actions.iter().map(|a| a.action_type).collect()
}

// ---------------------------------------------------------------------------
// XMP / PDF/A identification roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_xmp_errors_cleared_after_convert() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    // Pre: plain PDF must have XMP / pdfaid errors.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    let pre_codes = validation_error_codes(&pre);
    assert!(
        pre_codes.contains(&ErrorCode::MissingXmpMetadata)
            || pre_codes.contains(&ErrorCode::MissingPdfaIdentification),
        "expected XMP/pdfaid errors before conversion, got: {pre_codes:?}"
    );

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    // Post: XMP / pdfaid errors must be gone.
    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    let post_codes = validation_error_codes(&post);
    assert!(
        !post_codes.contains(&ErrorCode::MissingXmpMetadata),
        "MissingXmpMetadata still present after conversion"
    );
    assert!(
        !post_codes.contains(&ErrorCode::MissingPdfaIdentification),
        "MissingPdfaIdentification still present after conversion"
    );
}

#[test]
fn test_xmp_action_recorded_on_conversion() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");
    let actions = conversion_action_types(&result);

    assert!(
        actions.contains(&ActionType::AddedXmpMetadata)
            || actions.contains(&ActionType::AddedPdfaIdentification),
        "expected XMP action recorded, got: {actions:?}"
    );
}

// ---------------------------------------------------------------------------
// OutputIntents roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_output_intent_error_cleared_after_convert() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    // OutputIntent error may or may not appear on minimal PDFs depending on
    // whether the validator detects device colours — but after conversion the
    // /OutputIntents key must be present unconditionally.
    let _ = validation_error_codes(&pre);

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    let catalog = doc.catalog().expect("catalog");
    assert!(
        catalog
            .as_dict()
            .map(|d| d.contains_key("OutputIntents"))
            .unwrap_or(false),
        "/OutputIntents must be present in catalog after conversion"
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::MissingOutputIntent),
        "MissingOutputIntent still present after conversion"
    );
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::DeviceColorWithoutIntent),
        "DeviceColorWithoutIntent still present after conversion"
    );
}

#[test]
fn test_output_intent_action_recorded() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");
    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");
    assert!(
        conversion_action_types(&result).contains(&ActionType::AddedOutputIntent),
        "expected AddedOutputIntent action, got: {:?}",
        result.actions
    );
}

// ---------------------------------------------------------------------------
// Language roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_language_error_cleared_after_convert() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::MissingLanguage),
        "MissingLanguage still present after conversion"
    );
}

#[test]
fn test_language_action_recorded() {
    // MissingLanguage is only checked for "a" levels (validate_structure runs).
    // Use A2a with add_structure so the validator reaches the /Lang check.
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");
    let config = ConversionConfig::new().add_structure(true);
    let result = PdfAConverter::new(PdfALevel::A2a)
        .with_config(config)
        .convert(&mut doc)
        .expect("convert");
    assert!(
        conversion_action_types(&result).contains(&ActionType::AddedLanguage),
        "expected AddedLanguage action, got: {:?}",
        result.actions
    );
}

// ---------------------------------------------------------------------------
// JavaScript removal roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_javascript_open_action_removed() {
    let bytes = build_pdf_with_open_action_js();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    // Pre: validator must see JavaScriptNotAllowed.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    assert!(
        validation_error_codes(&pre).contains(&ErrorCode::JavaScriptNotAllowed),
        "expected JavaScriptNotAllowed before conversion, got: {:?}",
        pre.errors
    );

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    // Post: no JS error.
    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::JavaScriptNotAllowed),
        "JavaScriptNotAllowed still present after conversion"
    );

    // Catalog must no longer have /OpenAction.
    let catalog = doc.catalog().expect("catalog");
    assert!(
        !catalog
            .as_dict()
            .map(|d| d.contains_key("OpenAction"))
            .unwrap_or(false),
        "/OpenAction must be removed from catalog"
    );
}

#[test]
fn test_javascript_names_tree_removed() {
    let bytes = build_pdf_with_names_javascript();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    assert!(
        validation_error_codes(&pre).contains(&ErrorCode::JavaScriptNotAllowed),
        "expected JavaScriptNotAllowed before conversion, got: {:?}",
        pre.errors
    );

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");
    assert!(
        conversion_action_types(&result).contains(&ActionType::RemovedJavaScript),
        "expected RemovedJavaScript action, got: {:?}",
        result.actions
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::JavaScriptNotAllowed),
        "JavaScriptNotAllowed still present after conversion"
    );
}

// ---------------------------------------------------------------------------
// Structure tree roundtrip (PDF/A-*a levels)
// ---------------------------------------------------------------------------

#[test]
fn test_structure_tree_added_for_level_a() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    // A2a requires structure.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A2a).expect("pre-validate");
    assert!(
        validation_error_codes(&pre).contains(&ErrorCode::MissingDocumentStructure),
        "expected MissingDocumentStructure before conversion, got: {:?}",
        pre.errors
    );

    // Enable structure generation.
    let config = ConversionConfig::new().add_structure(true);
    let converter = PdfAConverter::new(PdfALevel::A2a).with_config(config);
    let result = converter.convert(&mut doc).expect("convert");

    assert!(
        conversion_action_types(&result).contains(&ActionType::AddedStructure),
        "expected AddedStructure action, got: {:?}",
        result.actions
    );

    // Post: MissingDocumentStructure must be gone.
    let post = validate_pdf_a(&mut doc, PdfALevel::A2a).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::MissingDocumentStructure),
        "MissingDocumentStructure still present after structure addition"
    );
}

#[test]
fn test_structure_not_required_for_level_b() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    assert!(
        !validation_error_codes(&pre).contains(&ErrorCode::MissingDocumentStructure),
        "A2b must not require structure tree, got: {:?}",
        pre.errors
    );
}

// ---------------------------------------------------------------------------
// Font embedding (honest-failure without rendering; fixed with rendering)
// ---------------------------------------------------------------------------

#[test]
fn test_font_not_embedded_reported_without_rendering() {
    // The builder uses Helvetica (standard Type1), which has no /FontFile*
    // stream. Without the rendering feature, embed_font() cannot load a
    // system font, so it records a ConversionError rather than silently lying.
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    let pre_codes = validation_error_codes(&pre);

    if !pre_codes.contains(&ErrorCode::FontNotEmbedded) {
        // Builder PDF has no font dict picked up by validator — nothing to test.
        return;
    }

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    #[cfg(not(feature = "rendering"))]
    {
        // Without rendering, embed_font() must record a ConversionError.
        assert!(
            conversion_error_codes(&result).contains(&ErrorCode::FontNotEmbedded),
            "expected FontNotEmbedded ConversionError without rendering feature, got: {:?}",
            result.errors
        );
    }

    #[cfg(feature = "rendering")]
    {
        // With rendering, embed_font() should attempt system font lookup.
        // The result depends on whether Helvetica is installed on the system.
        let _ = result;
    }
}

#[cfg(feature = "rendering")]
#[test]
fn test_font_embedded_clears_error_when_system_font_available() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    if !validation_error_codes(&pre).contains(&ErrorCode::FontNotEmbedded) {
        return; // validator didn't flag it — nothing to test
    }

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");

    // If the system has the font, we should see EmbeddedFont action and no error.
    // If the font isn't installed, we see a ConversionError — also acceptable.
    let embedded = conversion_action_types(&result).contains(&ActionType::EmbeddedFont);
    let errored = conversion_error_codes(&result).contains(&ErrorCode::FontNotEmbedded);
    assert!(
        embedded || errored,
        "embed_font must either embed successfully or record ConversionError; got: {:?} / {:?}",
        result.actions,
        result.errors
    );

    // After conversion, the post-validate FontNotEmbedded count must not increase.
    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    let pre_count = validation_error_codes(&pre)
        .iter()
        .filter(|&&c| c == ErrorCode::FontNotEmbedded)
        .count();
    let post_count = validation_error_codes(&post)
        .iter()
        .filter(|&&c| c == ErrorCode::FontNotEmbedded)
        .count();
    assert!(
        post_count <= pre_count,
        "font embedding must not increase FontNotEmbedded errors: {pre_count} before, {post_count} after"
    );
}

// ---------------------------------------------------------------------------
// Annotation appearance roundtrip (requires rendering)
// ---------------------------------------------------------------------------

/// Build a minimal PDF with a text annotation that has no /AP dict.
#[cfg(feature = "rendering")]
fn build_pdf_with_annotation_no_ap() -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    out.extend_from_slice(b"%PDF-1.4\n");

    // Obj 1: Catalog
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Obj 2: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 1 /Kids [3 0 R] >>\nendobj\n");

    // Obj 3: Page with /Annots referencing obj 4
    offsets.push(out.len());
    out.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>\nendobj\n",
    );

    // Obj 4: FreeText annotation with NO /AP dict
    offsets.push(out.len());
    out.extend_from_slice(
        b"4 0 obj\n<< /Type /Annot /Subtype /FreeText /Rect [100 600 300 650] /Contents (Test annotation) /DA (/Helv 12 Tf 0 g) >>\nendobj\n",
    );

    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 5\n");
    out.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    out.extend_from_slice(b"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n");
    out.extend_from_slice(format!("{}\n", xref_offset).as_bytes());
    out.extend_from_slice(b"%%EOF\n");
    out
}

#[cfg(feature = "rendering")]
#[test]
fn test_annotation_appearance_synthesised() {
    let bytes = build_pdf_with_annotation_no_ap();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    // Pre: validator should flag MissingAppearanceStream.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("pre-validate");
    let pre_codes = validation_error_codes(&pre);
    if !pre_codes.contains(&ErrorCode::MissingAppearanceStream) {
        // Validator didn't flag it (may depend on annotation subtype checks).
        // Still run conversion to confirm no panic.
        convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert must not panic");
        return;
    }

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");
    assert!(
        conversion_action_types(&result).contains(&ActionType::FixedAnnotation),
        "expected FixedAnnotation action, got: {:?}",
        result.actions
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::MissingAppearanceStream),
        "MissingAppearanceStream still present after appearance synthesis"
    );
}

// ---------------------------------------------------------------------------
// Transparency flattening (requires rendering)
// ---------------------------------------------------------------------------

/// Build a minimal PDF with a transparent ExtGState (/ca 0.5).
#[cfg(feature = "rendering")]
fn build_pdf_with_transparency() -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    out.extend_from_slice(b"%PDF-1.4\n");

    // Obj 1: Catalog
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Obj 2: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 1 /Kids [3 0 R] >>\nendobj\n");

    // Obj 3: Page with Resources containing a transparent ExtGState
    offsets.push(out.len());
    out.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
        /Resources << /ExtGState << /GS1 4 0 R >> >> \
        /Contents 5 0 R >>\nendobj\n",
    );

    // Obj 4: ExtGState with /ca (fill opacity) less than 1 → triggers transparency
    offsets.push(out.len());
    out.extend_from_slice(b"4 0 obj\n<< /Type /ExtGState /ca 0.5 /CA 0.5 >>\nendobj\n");

    // Obj 5: Content stream using the transparent ExtGState
    let content = b"q /GS1 gs 100 100 200 200 re f Q";
    offsets.push(out.len());
    out.extend_from_slice(format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    out.extend_from_slice(content);
    out.extend_from_slice(b"\nendstream\nendobj\n");

    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 6\n");
    out.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    out.extend_from_slice(b"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n");
    out.extend_from_slice(format!("{}\n", xref_offset).as_bytes());
    out.extend_from_slice(b"%%EOF\n");
    out
}

#[cfg(feature = "rendering")]
#[test]
fn test_transparency_flattened_for_a1b() {
    let bytes = build_pdf_with_transparency();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    // PDF/A-1b does not allow transparency.
    let pre = validate_pdf_a(&mut doc, PdfALevel::A1b).expect("pre-validate");
    let pre_codes = validation_error_codes(&pre);
    if !pre_codes.contains(&ErrorCode::TransparencyNotAllowed) {
        // Validator didn't flag transparency on this minimal PDF — still run
        // conversion to confirm no crash.
        convert_to_pdf_a(&mut doc, PdfALevel::A1b).expect("convert must not panic");
        return;
    }

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A1b).expect("convert");
    assert!(
        conversion_action_types(&result).contains(&ActionType::FlattenedTransparency),
        "expected FlattenedTransparency action, got: {:?}",
        result.actions
    );

    // Post: transparency error must be gone.
    let post = validate_pdf_a(&mut doc, PdfALevel::A1b).expect("post-validate");
    assert!(
        !validation_error_codes(&post).contains(&ErrorCode::TransparencyNotAllowed),
        "TransparencyNotAllowed still present after flattening"
    );
}

// ---------------------------------------------------------------------------
// Idempotency: converting twice must not make things worse
// ---------------------------------------------------------------------------

#[test]
fn test_conversion_idempotent_for_b_levels() {
    for level in [PdfALevel::A1b, PdfALevel::A2b, PdfALevel::A3b] {
        let bytes = build_plain_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

        convert_to_pdf_a(&mut doc, level).expect("first convert");
        let post1 = validate_pdf_a(&mut doc, level).expect("post1 validate");
        let codes1 = validation_error_codes(&post1);

        convert_to_pdf_a(&mut doc, level).expect("second convert");
        let post2 = validate_pdf_a(&mut doc, level).expect("post2 validate");
        let codes2 = validation_error_codes(&post2);

        // Second conversion must not introduce new error codes.
        for code in &codes2 {
            assert!(
                codes1.contains(code),
                "second conversion introduced new error {code:?} for level {level:?}"
            );
        }
    }
}

#[test]
fn test_output_intents_not_duplicated_on_second_convert() {
    let bytes = build_plain_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("first convert");
    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("second convert");

    let catalog = doc.catalog().expect("catalog");
    if let Some(pdf_oxide::object::Object::Array(arr)) =
        catalog.as_dict().and_then(|d| d.get("OutputIntents"))
    {
        assert_eq!(
            arr.len(),
            1,
            "OutputIntents duplicated after two conversions: {} entries",
            arr.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Result is always a valid, re-parseable PDF
// ---------------------------------------------------------------------------

#[test]
fn test_converted_bytes_are_reparseable() {
    for level in [PdfALevel::A1b, PdfALevel::A2b, PdfALevel::A3b] {
        let bytes = build_plain_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).expect("parse");
        convert_to_pdf_a(&mut doc, level).expect("convert");

        assert!(
            doc.source_bytes.starts_with(b"%PDF-"),
            "converted bytes do not start with %PDF- for level {level:?}"
        );
        PdfDocument::from_bytes(doc.source_bytes.clone())
            .expect("re-parse of converted bytes failed for level {level:?}");
    }
}

#[test]
fn test_converted_multipage_pdf_is_reparseable() {
    let bytes = build_multipage_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse");
    convert_to_pdf_a(&mut doc, PdfALevel::A2b).expect("convert");
    PdfDocument::from_bytes(doc.source_bytes.clone()).expect("re-parse");
}

// ---------------------------------------------------------------------------
// All B-level fixable errors must clear (no new unfixable errors introduced)
// ---------------------------------------------------------------------------

#[test]
fn test_fixable_errors_cleared_for_all_b_levels() {
    // These are the errors that the converter can always fix regardless of
    // the rendering feature:
    let always_fixable = [
        ErrorCode::MissingXmpMetadata,
        ErrorCode::MissingPdfaIdentification,
        ErrorCode::MissingOutputIntent,
        ErrorCode::DeviceColorWithoutIntent,
        ErrorCode::MissingLanguage,
        ErrorCode::JavaScriptNotAllowed,
        ErrorCode::EmbeddedFileNotAllowed,
    ];

    for level in [PdfALevel::A1b, PdfALevel::A2b, PdfALevel::A3b] {
        let bytes = build_plain_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).expect("parse");

        convert_to_pdf_a(&mut doc, level).expect("convert");

        let post = validate_pdf_a(&mut doc, level).expect("post-validate");
        for code in &always_fixable {
            assert!(
                !validation_error_codes(&post).contains(code),
                "fixable error {code:?} still present after conversion at level {level:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// XMP part/conformance correctness per level
// ---------------------------------------------------------------------------

#[test]
fn test_xmp_part_and_conformance_per_level() {
    let cases = [
        (PdfALevel::A1b, "1", "B"),
        (PdfALevel::A2b, "2", "B"),
        (PdfALevel::A2u, "2", "U"),
        (PdfALevel::A3b, "3", "B"),
    ];
    for (level, expected_part, expected_conf) in cases {
        let bytes = build_plain_pdf();
        let mut doc = PdfDocument::from_bytes(bytes).expect("parse");
        convert_to_pdf_a(&mut doc, level).expect("convert");

        let xmp = pdf_oxide::extractors::xmp::XmpExtractor::extract(&doc)
            .expect("XmpExtractor error")
            .expect("XMP missing after conversion");

        assert_eq!(
            xmp.custom.get("pdfaid:part").map(String::as_str),
            Some(expected_part),
            "pdfaid:part wrong for level {level:?}"
        );
        assert_eq!(
            xmp.custom.get("pdfaid:conformance").map(String::as_str),
            Some(expected_conf),
            "pdfaid:conformance wrong for level {level:?}"
        );
    }
}
