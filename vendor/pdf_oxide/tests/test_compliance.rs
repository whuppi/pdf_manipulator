//! Integration tests for PDF/A compliance validation.

use pdf_oxide::compliance::{PdfALevel, PdfAValidator, ValidationResult};
use pdf_oxide::document::PdfDocument;
use std::path::PathBuf;

fn get_test_pdf_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

#[test]
fn test_pdf_a_levels() {
    // Test all PDF/A level properties
    assert!(PdfALevel::A1a.requires_structure());
    assert!(!PdfALevel::A1b.requires_structure());
    assert!(PdfALevel::A2a.requires_structure());
    assert!(!PdfALevel::A2b.requires_structure());

    assert!(!PdfALevel::A1a.allows_transparency());
    assert!(!PdfALevel::A1b.allows_transparency());
    assert!(PdfALevel::A2a.allows_transparency());
    assert!(PdfALevel::A2b.allows_transparency());

    assert!(!PdfALevel::A1b.allows_embedded_files());
    assert!(!PdfALevel::A2b.allows_embedded_files());
    assert!(PdfALevel::A3b.allows_embedded_files());
}

#[test]
fn test_validation_result_creation() {
    let result = ValidationResult::new(PdfALevel::A2b);
    assert!(!result.is_compliant);
    assert!(!result.has_errors());
    assert!(!result.has_warnings());
    assert_eq!(result.level, PdfALevel::A2b);
}

#[test]
fn test_validator_creation() {
    let validator = PdfAValidator::new();
    let _validator_stop = validator.stop_on_first_error(true);
    let _validator_no_warn = PdfAValidator::new().include_warnings(false);

    // These are just construction tests (no panic = success)
}

#[test]
fn test_validate_simple_pdf() {
    // Test validation of a simple PDF from test data
    let pdf_path = get_test_pdf_path("simple.pdf");
    if !pdf_path.exists() {
        // Skip if test file doesn't exist
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");
    let validator = PdfAValidator::new();
    let result = validator
        .validate(&mut doc, PdfALevel::A2b)
        .expect("Validation failed");

    // Simple PDFs typically won't be PDF/A compliant
    // We're just testing that validation runs without errors
    println!("Validation result: compliant={}", result.is_compliant);
    println!("Errors: {}", result.errors.len());
    println!("Warnings: {}", result.warnings.len());
}

#[test]
fn test_validate_encyclopedia_pdf() {
    // Test validation of a more complex PDF
    let pdf_path = get_test_pdf_path("encyclopedia.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");
    let validator = PdfAValidator::new();
    let result = validator
        .validate(&mut doc, PdfALevel::A2b)
        .expect("Validation failed");

    println!("Encyclopedia validation result: compliant={}", result.is_compliant);
    println!("Errors: {}", result.errors.len());
    for error in &result.errors {
        println!("  - {}", error);
    }
}

#[test]
fn test_individual_validators() {
    let pdf_path = get_test_pdf_path("simple.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");
    let validator = PdfAValidator::new();

    // Test individual check methods
    let metadata_result = validator
        .check_metadata(&mut doc, PdfALevel::A2b)
        .expect("Check failed");
    println!("Metadata check: compliant={}", metadata_result.is_compliant);

    let color_result = validator
        .check_colors(&mut doc, PdfALevel::A2b)
        .expect("Check failed");
    println!("Color check: compliant={}", color_result.is_compliant);

    // PDF/A-1 specific checks
    let transparency_result = validator
        .check_transparency(&mut doc, PdfALevel::A1b)
        .expect("Check failed");
    println!("Transparency check (A1b): warnings={}", transparency_result.warnings.len());
}

#[test]
fn test_level_a_structure_requirements() {
    let pdf_path = get_test_pdf_path("simple.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");
    let validator = PdfAValidator::new();

    // Level A requires structure tree
    let result_a = validator
        .check_structure(&mut doc, PdfALevel::A2a)
        .expect("Check failed");
    println!("Structure check (A2a): errors={}", result_a.errors.len());

    // Level B doesn't require structure tree
    let result_b = validator
        .check_structure(&mut doc, PdfALevel::A2b)
        .expect("Check failed");
    println!("Structure check (A2b): errors={}", result_b.errors.len());
}

#[test]
fn test_pdf_a_level_from_xmp() {
    // Test parsing PDF/A level from XMP values
    assert_eq!(PdfALevel::from_xmp("1", "A"), Some(PdfALevel::A1a));
    assert_eq!(PdfALevel::from_xmp("1", "B"), Some(PdfALevel::A1b));
    assert_eq!(PdfALevel::from_xmp("2", "A"), Some(PdfALevel::A2a));
    assert_eq!(PdfALevel::from_xmp("2", "B"), Some(PdfALevel::A2b));
    assert_eq!(PdfALevel::from_xmp("2", "U"), Some(PdfALevel::A2u));
    assert_eq!(PdfALevel::from_xmp("3", "A"), Some(PdfALevel::A3a));
    assert_eq!(PdfALevel::from_xmp("3", "B"), Some(PdfALevel::A3b));
    assert_eq!(PdfALevel::from_xmp("3", "U"), Some(PdfALevel::A3u));

    // Invalid combinations
    assert_eq!(PdfALevel::from_xmp("1", "U"), None);
    assert_eq!(PdfALevel::from_xmp("4", "A"), None);
    assert_eq!(PdfALevel::from_xmp("0", "B"), None);
}

#[test]
fn test_stop_on_first_error() {
    let pdf_path = get_test_pdf_path("simple.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");

    // With stop_on_first_error, we should get at most 1 error for some validators
    let validator = PdfAValidator::new().stop_on_first_error(true);
    let result = validator
        .validate(&mut doc, PdfALevel::A2b)
        .expect("Validation failed");

    println!("Stop-on-first-error result: errors={}", result.errors.len());
}

#[test]
fn test_include_warnings() {
    let pdf_path = get_test_pdf_path("simple.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping test: {} not found", pdf_path.display());
        return;
    }

    let mut doc = PdfDocument::open(&pdf_path).expect("Failed to open PDF");

    // With include_warnings(false), we should get no warnings
    let validator = PdfAValidator::new().include_warnings(false);
    let result = validator
        .validate(&mut doc, PdfALevel::A2b)
        .expect("Validation failed");

    assert!(
        result.warnings.is_empty(),
        "Expected no warnings when include_warnings is false"
    );
}
