// PDF/A conversion: validate → convert → validate
//
// Demonstrates and verifies the full archival pipeline across all fixable
// compliance scenarios. Each section:
//   1. Builds a PDF that has a known compliance problem.
//   2. Validates (confirms the expected error is present).
//   3. Converts.
//   4. Validates again (asserts the error is gone).
//
// Font: DejaVu Sans (Bitstream Vera derivative, permissive licence — freely
// embeddable). Bundled via include_bytes! so the example is self-contained.
//
// Run in CI: cargo run --example showcase_pdfa_conversion
// Any panic fails the CI job.

use pdf_oxide::{
    compliance::{
        convert_to_pdf_a, validate_pdf_a, ActionType, ConversionConfig, ErrorCode, PdfAConverter,
        PdfALevel,
    },
    document::PdfDocument,
    error::Result,
    extractors::xmp::XmpExtractor,
    writer::{DocumentBuilder, EmbeddedFont, PageSize},
};

// DejaVu Sans — SIL OFL / Bitstream Vera permissive licence.
const DEJAVU_SANS: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/DejaVuSans.ttf");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_plain_pdf() -> Vec<u8> {
    let font = EmbeddedFont::from_data(Some("DejaVuSans".into()), DEJAVU_SANS.to_vec())
        .expect("DejaVuSans embedded font");
    let mut builder = DocumentBuilder::new().register_embedded_font("DejaVu", font);
    {
        let page = builder.page(PageSize::Letter);
        page.font("DejaVu", 12.0)
            .at(72.0, 720.0)
            .text("PDF/A example")
            .done();
    }
    builder.build().expect("builder failed")
}

/// Build a minimal PDF with a catalog-level /OpenAction JavaScript action.
fn build_pdf_with_open_action_js() -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");
    offsets.push(out.len());
    out.extend_from_slice(
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /OpenAction 3 0 R >>\nendobj\n",
    );
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Count 1 /Kids [4 0 R] >>\nendobj\n");
    offsets.push(out.len());
    out.extend_from_slice(b"3 0 obj\n<< /S /JavaScript /JS (app.alert(1)) >>\nendobj\n");
    offsets.push(out.len());
    out.extend_from_slice(
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
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

fn error_codes(result: &pdf_oxide::compliance::ValidationResult) -> Vec<ErrorCode> {
    result.errors.iter().map(|e| e.code).collect()
}

fn action_types(result: &pdf_oxide::compliance::ConversionResult) -> Vec<ActionType> {
    result.actions.iter().map(|a| a.action_type).collect()
}

// ---------------------------------------------------------------------------
// Scenario 1: XMP metadata
// ---------------------------------------------------------------------------

fn scenario_xmp_metadata() -> Result<()> {
    println!("\n[1] XMP metadata (MissingXmpMetadata + MissingPdfaIdentification)");

    let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b)?;
    let pre_codes = error_codes(&pre);
    assert!(
        pre_codes.contains(&ErrorCode::MissingXmpMetadata)
            || pre_codes.contains(&ErrorCode::MissingPdfaIdentification),
        "expected XMP/pdfaid errors before conversion, got: {pre_codes:?}"
    );
    println!("  pre-conversion errors: {}", pre.errors.len());

    let result = convert_to_pdf_a(&mut doc, PdfALevel::A2b)?;
    assert!(
        action_types(&result).contains(&ActionType::AddedXmpMetadata)
            || action_types(&result).contains(&ActionType::AddedPdfaIdentification),
        "expected XMP action, got: {:?}",
        result.actions
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b)?;
    let post_codes = error_codes(&post);
    assert!(
        !post_codes.contains(&ErrorCode::MissingXmpMetadata),
        "MissingXmpMetadata not cleared"
    );
    assert!(
        !post_codes.contains(&ErrorCode::MissingPdfaIdentification),
        "MissingPdfaIdentification not cleared"
    );
    println!("  post-conversion errors: {} — PASS", post.errors.len());

    // Verify XMP part/conformance values per level.
    for (level, part, conformance) in [
        (PdfALevel::A1b, "1", "B"),
        (PdfALevel::A2b, "2", "B"),
        (PdfALevel::A2u, "2", "U"),
        (PdfALevel::A3b, "3", "B"),
    ] {
        let mut d = PdfDocument::from_bytes(build_plain_pdf())?;
        convert_to_pdf_a(&mut d, level)?;
        let xmp = XmpExtractor::extract(&d)?.expect("XMP missing after conversion");
        assert_eq!(
            xmp.custom.get("pdfaid:part").map(String::as_str),
            Some(part),
            "pdfaid:part wrong for {level:?}"
        );
        assert_eq!(
            xmp.custom.get("pdfaid:conformance").map(String::as_str),
            Some(conformance),
            "pdfaid:conformance wrong for {level:?}"
        );
    }
    println!("  XMP part/conformance correct for all 4 B/U levels — PASS");

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 2: OutputIntents
// ---------------------------------------------------------------------------

fn scenario_output_intents() -> Result<()> {
    println!("\n[2] OutputIntents (MissingOutputIntent / DeviceColorWithoutIntent)");

    let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;
    convert_to_pdf_a(&mut doc, PdfALevel::A2b)?;

    let catalog = doc.catalog()?;
    assert!(
        catalog
            .as_dict()
            .map(|d| d.contains_key("OutputIntents"))
            .unwrap_or(false),
        "/OutputIntents must be present in catalog after conversion"
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b)?;
    assert!(
        !error_codes(&post).contains(&ErrorCode::MissingOutputIntent),
        "MissingOutputIntent not cleared"
    );
    assert!(
        !error_codes(&post).contains(&ErrorCode::DeviceColorWithoutIntent),
        "DeviceColorWithoutIntent not cleared"
    );
    println!("  /OutputIntents present, errors cleared — PASS");

    // Idempotency: two conversions must not duplicate OutputIntents.
    let mut doc2 = PdfDocument::from_bytes(build_plain_pdf())?;
    convert_to_pdf_a(&mut doc2, PdfALevel::A2b)?;
    convert_to_pdf_a(&mut doc2, PdfALevel::A2b)?;
    let catalog2 = doc2.catalog()?;
    if let Some(pdf_oxide::object::Object::Array(arr)) =
        catalog2.as_dict().and_then(|d| d.get("OutputIntents"))
    {
        assert_eq!(
            arr.len(),
            1,
            "OutputIntents duplicated after two conversions: {} entries",
            arr.len()
        );
    }
    println!("  OutputIntents not duplicated on second conversion — PASS");

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 3: Language
// ---------------------------------------------------------------------------

fn scenario_language() -> Result<()> {
    println!("\n[3] Language (/Lang via MissingLanguage on A-level)");

    let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;
    let config = ConversionConfig::new().add_structure(true);
    let result = PdfAConverter::new(PdfALevel::A2a)
        .with_config(config)
        .convert(&mut doc)?;

    assert!(
        action_types(&result).contains(&ActionType::AddedLanguage),
        "expected AddedLanguage action, got: {:?}",
        result.actions
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2a)?;
    assert!(
        !error_codes(&post).contains(&ErrorCode::MissingLanguage),
        "MissingLanguage not cleared"
    );
    println!("  /Lang set and MissingLanguage cleared — PASS");

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 4: JavaScript removal
// ---------------------------------------------------------------------------

fn scenario_javascript_removal() -> Result<()> {
    println!("\n[4] JavaScript removal (JavaScriptNotAllowed)");

    let mut doc = PdfDocument::from_bytes(build_pdf_with_open_action_js())?;

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2b)?;
    assert!(
        error_codes(&pre).contains(&ErrorCode::JavaScriptNotAllowed),
        "expected JavaScriptNotAllowed before conversion, got: {:?}",
        pre.errors
    );
    println!("  pre-conversion: JavaScriptNotAllowed present");

    convert_to_pdf_a(&mut doc, PdfALevel::A2b)?;

    let post = validate_pdf_a(&mut doc, PdfALevel::A2b)?;
    assert!(
        !error_codes(&post).contains(&ErrorCode::JavaScriptNotAllowed),
        "JavaScriptNotAllowed still present after conversion"
    );

    let catalog = doc.catalog()?;
    assert!(
        !catalog
            .as_dict()
            .map(|d| d.contains_key("OpenAction"))
            .unwrap_or(false),
        "/OpenAction must be removed from catalog"
    );
    println!("  /OpenAction removed, JavaScriptNotAllowed cleared — PASS");

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 5: Structure tree (PDF/A-*a levels)
// ---------------------------------------------------------------------------

fn scenario_structure_tree() -> Result<()> {
    println!("\n[5] Structure tree (MissingDocumentStructure for A-levels)");

    let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;

    let pre = validate_pdf_a(&mut doc, PdfALevel::A2a)?;
    assert!(
        error_codes(&pre).contains(&ErrorCode::MissingDocumentStructure),
        "expected MissingDocumentStructure before conversion, got: {:?}",
        pre.errors
    );
    println!("  pre-conversion: MissingDocumentStructure present");

    let config = ConversionConfig::new().add_structure(true);
    let result = PdfAConverter::new(PdfALevel::A2a)
        .with_config(config)
        .convert(&mut doc)?;
    assert!(
        action_types(&result).contains(&ActionType::AddedStructure),
        "expected AddedStructure action, got: {:?}",
        result.actions
    );

    let post = validate_pdf_a(&mut doc, PdfALevel::A2a)?;
    assert!(
        !error_codes(&post).contains(&ErrorCode::MissingDocumentStructure),
        "MissingDocumentStructure still present after structure addition"
    );
    println!("  /StructTreeRoot + /MarkInfo added, error cleared — PASS");

    // B-levels must NOT require structure.
    let mut doc2 = PdfDocument::from_bytes(build_plain_pdf())?;
    let pre2 = validate_pdf_a(&mut doc2, PdfALevel::A2b)?;
    assert!(
        !error_codes(&pre2).contains(&ErrorCode::MissingDocumentStructure),
        "A2b must not require structure tree"
    );
    println!("  A2b correctly does not require structure — PASS");

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 6: Full compliance for all B-levels (0 fixable errors remain)
// ---------------------------------------------------------------------------

fn scenario_full_compliance_b_levels() -> Result<()> {
    println!("\n[6] Full compliance: all fixable errors cleared for all B-levels");

    // Always fixable regardless of feature flags.
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
        let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;
        convert_to_pdf_a(&mut doc, level)?;
        let post = validate_pdf_a(&mut doc, level)?;

        for code in &always_fixable {
            assert!(
                !error_codes(&post).contains(code),
                "fixable error {code:?} still present after conversion at {level:?}: {:?}",
                post.errors
            );
        }

        // With the `rendering` feature, font embedding via URW/Nimbus system fonts
        // also runs — standard PDF Type1 fonts (Helvetica, Courier, Times) are
        // mapped to Nimbus Sans / Nimbus Mono PS / Nimbus Roman equivalents.
        #[cfg(feature = "rendering")]
        assert!(
            !error_codes(&post).contains(&ErrorCode::FontNotEmbedded),
            "FontNotEmbedded still present after conversion at {level:?}: {:?}",
            post.errors
        );

        let remaining = post.errors.len();
        println!("  {level:?}: {remaining} errors remain after conversion — PASS");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 7: Idempotency and re-parseability
// ---------------------------------------------------------------------------

fn scenario_idempotency_and_reparseable() -> Result<()> {
    println!("\n[7] Idempotency and re-parseability");

    for level in [PdfALevel::A1b, PdfALevel::A2b, PdfALevel::A3b] {
        let mut doc = PdfDocument::from_bytes(build_plain_pdf())?;
        convert_to_pdf_a(&mut doc, level)?;

        assert!(
            doc.source_bytes.starts_with(b"%PDF-"),
            "converted bytes do not start with %PDF- for {level:?}"
        );
        PdfDocument::from_bytes(doc.source_bytes.clone())
            .unwrap_or_else(|_| panic!("re-parse failed for {level:?}"));

        // Second conversion must not introduce new errors.
        let post1 = validate_pdf_a(&mut doc, level)?;
        convert_to_pdf_a(&mut doc, level)?;
        let post2 = validate_pdf_a(&mut doc, level)?;
        for code in error_codes(&post2) {
            assert!(
                error_codes(&post1).contains(&code),
                "second conversion introduced new error {code:?} for {level:?}"
            );
        }
        println!("  {level:?}: re-parseable and idempotent — PASS");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("=== PDF/A conversion validate→convert→validate pipeline ===");
    println!("Font: DejaVu Sans (Bitstream Vera permissive licence, freely embeddable)");

    scenario_xmp_metadata()?;
    scenario_output_intents()?;
    scenario_language()?;
    scenario_javascript_removal()?;
    scenario_structure_tree()?;
    scenario_full_compliance_b_levels()?;
    scenario_idempotency_and_reparseable()?;

    println!("\n=== All scenarios passed ===");
    Ok(())
}
