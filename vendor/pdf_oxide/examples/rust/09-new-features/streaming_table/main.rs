// StreamingTable with rowspan
//
// Demonstrates creating a multi-page-safe streaming table where the first
// column spans two rows using rowspan support.
//
// Run: cargo run --example showcase_streaming_table

use pdf_oxide::{
    error::Result,
    writer::{CellAlign, DocumentBuilder, DocumentMetadata, StreamingColumn, StreamingTableConfig},
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let out_dir = PathBuf::from("target/examples_output/streaming_table");
    std::fs::create_dir_all(&out_dir)?;

    let cfg = StreamingTableConfig::new()
        .column(StreamingColumn::new("Category").width_pt(120.0))
        .column(StreamingColumn::new("Item").width_pt(160.0))
        .column(
            StreamingColumn::new("Notes")
                .width_pt(150.0)
                .align(CellAlign::Right),
        )
        .repeat_header(true)
        .max_rowspan(2);

    let mut builder =
        DocumentBuilder::new().metadata(DocumentMetadata::new().title("StreamingTable Demo"));
    {
        let mut tbl = builder
            .letter_page()
            .font("Helvetica", 10.0)
            .at(72.0, 700.0)
            .heading(1, "Product Catalogue")
            .at(72.0, 660.0)
            .streaming_table(cfg);

        tbl.push_row(|r| {
            r.span_cell("Fruits", 2); // spans row 1 + row 2
            r.cell("Apple");
            r.cell("crisp");
        })?;
        tbl.push_row(|r| {
            r.cell(""); // continuation cell for the span
            r.cell("Banana");
            r.cell("sweet");
        })?;
        tbl.push_row(|r| {
            r.cell("Vegetables");
            r.cell("Carrot");
            r.cell("earthy");
        })?;
        tbl.finish().done();
    }

    let bytes = builder.build()?;
    let out = out_dir.join("streaming_table_rowspan.pdf");
    std::fs::write(&out, bytes)?;
    println!("Written: {}", out.display());
    Ok(())
}
