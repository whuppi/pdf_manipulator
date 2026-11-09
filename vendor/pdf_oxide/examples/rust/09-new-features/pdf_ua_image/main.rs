// PDF/UA accessible + decorative images
//
// Demonstrates two image embedding modes required for PDF/UA conformance:
//   • image_from_bytes_with_alt  — tagged Figure with alt text for screen readers
//   • image_from_bytes_as_artifact — /Artifact tag for purely decorative images
//
// Run: cargo run --example showcase_pdf_ua_image

use pdf_oxide::{
    error::Result,
    geometry::Rect,
    writer::{DocumentBuilder, DocumentMetadata},
};
use std::path::PathBuf;

// Minimal 1×1 white PNG (no external fixture needed).
const WHITE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xff, 0xff, 0x3f,
    0x00, 0x05, 0xfe, 0x02, 0xfe, 0x0d, 0xef, 0x46, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

fn main() -> Result<()> {
    let out_dir = PathBuf::from("target/examples_output/pdf_ua_image");
    std::fs::create_dir_all(&out_dir)?;

    let mut builder = DocumentBuilder::new().metadata(
        DocumentMetadata::new()
            .title("Accessible PDF Demo")
            .tagged_pdf_ua1()
            .language("en-US"),
    );
    builder
        .a4_page()
        .font("Helvetica", 12.0)
        .at(72.0, 750.0)
        .heading(1, "Accessible document with images")
        .at(72.0, 720.0)
        .paragraph("The image below has descriptive alt text for screen readers.")
        .image_from_bytes_with_alt(
            WHITE_PNG,
            Rect::new(72.0, 580.0, 100.0, 100.0),
            "A white placeholder image used for demonstration purposes",
        )?
        .at(72.0, 550.0)
        .paragraph("The logo below is purely decorative and marked as an artifact.")
        .image_from_bytes_as_artifact(WHITE_PNG, Rect::new(72.0, 450.0, 60.0, 60.0))?
        .done();

    let bytes = builder.build()?;
    let out = out_dir.join("pdf_ua_accessible_images.pdf");
    std::fs::write(&out, bytes)?;
    println!("Written: {}", out.display());
    Ok(())
}
