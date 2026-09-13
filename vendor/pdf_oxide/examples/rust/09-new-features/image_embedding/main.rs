// Image embedding
//
// Demonstrates embedding PNG images into a PDF using raw bytes.
// No pixel dimensions needed — the library auto-detects them from the image header.
//
// Addresses issue #425: ImageContent::new() required explicit width/height;
// image_from_bytes() does not.
// Addresses issue #450: PNG images with an alpha channel previously displayed
// a diagonal stripe; fixed by adding DecodeParms to the soft-mask XObject.
//
// Run: cargo run --example showcase_image_embedding

use pdf_oxide::{error::Result, geometry::Rect, writer::DocumentBuilder};
use std::path::PathBuf;

// 1×1 white opaque PNG (68 bytes).
const WHITE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xff, 0xff, 0x3f,
    0x00, 0x05, 0xfe, 0x02, 0xfe, 0x0d, 0xef, 0x46, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

// 1×1 semi-transparent red PNG (RGBA, color type 6) — #450 regression check.
// Previously a diagonal stripe appeared due to missing DecodeParms in the SMask XObject.
const RGBA_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xd0,
    0x00, 0x00, 0x04, 0x81, 0x01, 0x80, 0x2c, 0x55, 0xce, 0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn main() -> Result<()> {
    let out_dir = PathBuf::from("target/examples_output/image_embedding");
    std::fs::create_dir_all(&out_dir)?;

    let mut builder = DocumentBuilder::new();
    builder
        .a4_page()
        .font("Helvetica", 12.0)
        .at(72.0, 720.0)
        .heading(1, "Image embedding with auto-detected dimensions")
        .at(72.0, 690.0)
        .paragraph("No pixel dims needed — the library reads them from the image header.")
        .image_from_bytes(WHITE_PNG, Rect::new(72.0, 480.0, 200.0, 200.0))?
        .at(72.0, 460.0)
        .paragraph("Image displayed 200×200 pt — pixel resolution is auto-detected.")
        .at(72.0, 420.0)
        .paragraph("Transparent PNG below — rendered without diagonal-line artifact (#450).")
        .image_from_bytes(RGBA_PNG, Rect::new(72.0, 200.0, 200.0, 200.0))?
        .done();

    let bytes = builder.build()?;
    assert!(!bytes.is_empty(), "output PDF must be non-empty");

    let out = out_dir.join("image_embedding.pdf");
    std::fs::write(&out, &bytes)?;
    println!("Written: {}", out.display());
    println!("All image embedding checks passed.");
    Ok(())
}
