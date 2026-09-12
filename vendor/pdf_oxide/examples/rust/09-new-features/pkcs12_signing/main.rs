// PKCS#12 CMS signing
//
// Builds a simple PDF, signs it with a PKCS#12 certificate, and verifies
// the output contains the expected /ByteRange signature marker.
//
// Requires the `signatures` feature:
//   cargo run --example showcase_pkcs12_signing --features signatures
//
// The test certificate lives at tests/fixtures/test_signing.p12.
// The example skips gracefully if the file is absent.

#[cfg(not(feature = "signatures"))]
fn main() {
    println!("SKIP: build with --features signatures to run this example.");
}

#[cfg(feature = "signatures")]
fn main() -> pdf_oxide::error::Result<()> {
    use pdf_oxide::{
        signatures::{sign_pdf_bytes, SignOptions, SigningCredentials},
        writer::DocumentBuilder,
    };
    use std::path::PathBuf;

    let out_dir = PathBuf::from("target/examples_output/pkcs12_signing");
    std::fs::create_dir_all(&out_dir)?;

    let p12_path = "tests/fixtures/test_signing.p12";
    if !std::path::Path::new(p12_path).exists() {
        println!("SKIP: {} not found", p12_path);
        return Ok(());
    }

    let p12_data = std::fs::read(p12_path)?;
    let creds = SigningCredentials::from_pkcs12(&p12_data, "testpass")?;

    let mut builder = DocumentBuilder::new();
    builder
        .letter_page()
        .font("Helvetica", 12.0)
        .at(72.0, 720.0)
        .heading(1, "Signed Invoice")
        .at(72.0, 690.0)
        .paragraph("This document carries a CMS/PKCS#7 digital signature.")
        .done();
    let pdf_bytes = builder.build()?;

    let opts = SignOptions::default()
        .with_reason("Approved")
        .with_location("HQ");
    let signed = sign_pdf_bytes(&pdf_bytes, &creds, opts)?;

    let out = out_dir.join("signed_document.pdf");
    std::fs::write(&out, &signed)?;
    println!("Written: {} ({} bytes)", out.display(), signed.len());

    assert!(
        signed.windows(10).any(|w| w == b"/ByteRange"),
        "signature /ByteRange marker missing"
    );
    println!("Signature verified: /ByteRange present.");
    Ok(())
}
